mod mic;
mod ui;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mic::{Input, Mic};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError,
    WhisperState, WhisperVadContext, WhisperVadContextParams, WhisperVadParams,
};

pub use mic::DeviceInfo;
pub use ui::*;

const WHISPER_RATE: u32 = 16_000;
// Whisper rejects clips shorter than 1 s, so wait for this much before the first pass.
const MIN_SECS: u32 = 1;
// The VAD resets on every call, so each loop it looks at this much of the newest audio.
const VAD_CONTEXT_SECS: u32 = 3;
// Audio kept before the detected start of speech, so the first word isn't clipped.
const PRE_ROLL_MS: u32 = 200;
// Silence after the speech that's included in a section's final pass.
const TRAILING_SILENCE_MS: u32 = 300;
// Past this length, a long section is cut where two passes agree on a segment end.
const FORCE_CUT_SECS: u32 = 20;
// How far a segment end may move between passes and still count as agreeing.
const CUT_TOLERANCE_MS: i64 = 200;
// Whisper's limit: a section this long is finalized even without a pause or agreed cut.
const MAX_SECTION_SECS: u32 = 30;
// The VAD threshold: 0.0 = always speech, 1.0 = always silence.
const VAD_THRESHOLD: f32 = 0.8;
// New audio needed before the next VAD run and pass. Each VAD run looks at
// VAD_CONTEXT_SECS of audio, so running it for every small chunk the device delivers
// keeps the CPU busy once passes are fast.
const VAD_STEP_MS: u32 = 100;
// How long the worker waits for audio before checking the stop flag anyway.
const STOP_POLL: Duration = Duration::from_millis(100);

pub struct Config {
    pub model_path: PathBuf,
    pub vad_path: PathBuf,
    pub language: Option<String>, // None = auto-detect
    // Silence this long ends a section (also the VAD's min_silence_duration_ms).
    pub pause_ms: u32,
}

#[derive(Debug)]
pub enum Event {
    /// Setup is done and recording has started from this device.
    Started(DeviceInfo),
    /// Text of the open section so far; replaces the previous Live text.
    Live {
        text: String,
        pass_ms: u32,
        timing: Timing,
    },
    /// Final text: append it to the transcript, never revised. The live text
    /// is empty after this until the next Live.
    Final {
        text: String,
        #[expect(dead_code, reason = "the GUI doesn't need it")]
        reason: End,
        pass_ms: u32,
        timing: Timing,
    },
    /// An error from the audio stream; recording goes on.
    Error(String),
    /// Stopped because of this error, during setup or after. Nothing comes after it.
    Failed(Error),
}

#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Time spent waiting for audio before the iteration. Near 0 means the worker is
    /// busy all the time and falls behind.
    pub wait_ms: u32,
    pub new_audio_ms: u32,
    pub vad_ms: u32,
    pub audio_ms: u32,
    /// When the event was sent, to measure how long it takes to arrive.
    pub sent_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Pause,
    /// A long section was cut at a segment end two passes agreed on.
    Cut,
    MaxLength,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to load model: {0}")]
    LoadModel(WhisperError),
    #[error("failed to create state: {0}")]
    CreateState(WhisperError),
    #[error("failed to load VAD model: {0}")]
    LoadVad(WhisperError),
    #[error(transparent)]
    Mic(#[from] mic::Error),
    #[error("transcription failed: {0}")]
    Transcribe(WhisperError),
}

/// Loads the models, then records from the default input device and transcribes until
/// `stop` is set. Events are passed to `on_event` as they come, starting with
/// [`Event::Started`] once setup is done.
pub fn run(
    config: &Config,
    stop: &AtomicBool,
    on_event: &mut impl FnMut(Event),
) -> Result<(), Error> {
    // Route whisper.cpp's log output into whisper-rs, which drops it without a log backend.
    whisper_rs::install_logging_hooks();

    // --- Whisper: load the model once, reuse the state for every chunk ---
    let ctx =
        WhisperContext::new_with_params(&config.model_path, WhisperContextParameters::default())
            .map_err(Error::LoadModel)?;
    let mut state = ctx.create_state().map_err(Error::CreateState)?;

    // --- VAD: silero, only used to find where speech starts and pauses ---
    let mut vad = WhisperVadContext::new(
        &config.vad_path.to_string_lossy(),
        WhisperVadContextParams::default(),
    )
    .map_err(Error::LoadVad)?;

    // --- Mic: recording stops (and the mic is released) when this is dropped, on return ---
    let mic = Mic::start()?;
    let rate = mic.info.sample_rate;
    on_event(Event::Started(mic.info.clone()));

    // --- Main loop: sections of speech cut at pauses, re-transcribed until final ---
    let samples = |ms: u32| (rate as u64 * ms as u64 / 1000) as usize;
    let min_len = samples(MIN_SECS * 1000);
    let vad_len = samples(VAD_CONTEXT_SECS * 1000);
    let pre_roll = samples(PRE_ROLL_MS);
    let trailing = samples(TRAILING_SILENCE_MS);
    let force_len = samples(FORCE_CUT_SECS * 1000);
    let max_len = samples(MAX_SECTION_SECS * 1000);
    let vad_step = samples(VAD_STEP_MS);
    let language = config.language.as_deref().unwrap_or("auto");

    // Device-rate audio: the open section, plus whatever came before it that the VAD
    // and pre-roll still need. While idle only the newest VAD_CONTEXT_SECS are kept.
    let mut buf: Vec<f32> = Vec::new();
    // Start of the open section in `buf`; None while idle.
    let mut section: Option<usize> = None;
    // Where the VAD last saw speech end in `buf`, while a section is open.
    let mut speech_end = 0usize;
    // Segments of the previous pass over the open section, to agree on a cut point.
    let mut prev: Vec<Segment> = Vec::new();
    // Text of the last Live event, so unchanged passes aren't reported again.
    let mut live = String::new();
    let mut new_samples = 0;
    let mut waited = Duration::ZERO;

    loop {
        // Block until there's new audio, then take everything that queued up while
        // the previous pass was running so we always work on the latest audio.
        let recv_start = Instant::now();
        let received = mic.recv(STOP_POLL)?;
        waited += recv_start.elapsed();
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        for input in received {
            match input {
                Input::Audio(chunk) => {
                    new_samples += chunk.len();
                    buf.extend(chunk);
                }
                Input::Error(err) => on_event(Event::Error(format!("stream error: {err}"))),
            }
        }
        if new_samples < vad_step {
            continue;
        }
        let new_audio_ms = samples_to_ms(std::mem::take(&mut new_samples), rate);
        let wait_ms = millis(std::mem::take(&mut waited));

        let pause_ms = config.pause_ms;
        let pause_len = samples(pause_ms);

        // VAD over the newest few seconds. On error (e.g. too little audio yet) just
        // wait for more.
        let ctx_start = buf.len().saturating_sub(vad_len);
        let vad_start = Instant::now();
        let Ok(speech) = detect_speech(&mut vad, &buf[ctx_start..], rate, pause_ms) else {
            continue;
        };
        let vad_ms = millis(vad_start.elapsed());
        let timing = |audio_len: usize| Timing {
            wait_ms,
            new_audio_ms,
            vad_ms,
            audio_ms: samples_to_ms(audio_len, rate),
            sent_at: Instant::now(),
        };
        // A pause: no speech at all, or the last speech ended at least pause_ms ago.
        let paused = speech.is_none_or(|(_, end)| buf.len() - (ctx_start + end) >= pause_len);

        let start = match (section, speech) {
            (Some(start), _) => {
                if let Some((_, end)) = speech {
                    speech_end = ctx_start + end;
                }
                start
            }
            // Speech while idle opens a section, starting a little before it.
            (None, Some((first, end))) => {
                let start = (ctx_start + first).saturating_sub(pre_roll);
                section = Some(start);
                speech_end = ctx_start + end;
                prev.clear();
                start
            }
            // Still idle: audio outside a section is never transcribed.
            (None, None) => {
                buf.drain(..buf.len().saturating_sub(vad_len));
                continue;
            }
        };

        let len = buf.len() - start;

        if paused || len >= max_len {
            // The section is over: one final pass over all of it (after a pause, with a
            // little of the trailing silence). Its text is never revised.
            let end = if paused {
                (speech_end + trailing).clamp(start, buf.len())
            } else {
                start + max_len
            };
            let (segments, pass_ms) = transcribe(&mut state, &buf[start..end], rate, language)?;
            on_event(Event::Final {
                text: joined(&segments),
                reason: if paused { End::Pause } else { End::MaxLength },
                pass_ms,
                timing: timing(end - start),
            });
            buf.drain(..end);
            section = None;
            prev.clear();
            live.clear();
        } else if len >= min_len {
            let (mut segments, pass_ms) = transcribe(&mut state, &buf[start..], rate, language)?;

            // A long section is cut at a segment end both passes agree on: the text up
            // to there is final, and the rest of the audio starts a new open section.
            let mut cut_text = None;
            if len >= force_len
                && let Some(k) = agreed_cut(&prev, &segments)
            {
                let cut_cs = segments[k].end_cs;
                let cut = (start + cs_to_samples(cut_cs, rate)).min(buf.len());
                cut_text = Some(joined(&segments[..=k]));
                buf.drain(..cut);
                section = Some(0);
                speech_end = speech_end.saturating_sub(cut);
                // Keep the rest relative to the new section start for the next comparison.
                segments.drain(..=k);
                for seg in &mut segments {
                    seg.end_cs -= cut_cs;
                }
            }

            // Report a cut and the text after it back to back, so they show up together.
            let text = joined(&segments);
            if let Some(cut_text) = cut_text {
                on_event(Event::Final {
                    text: cut_text,
                    reason: End::Cut,
                    pass_ms,
                    timing: timing(len),
                });
                live.clear();
            }
            if text != live {
                live = text;
                on_event(Event::Live {
                    text: live.clone(),
                    pass_ms,
                    timing: timing(len),
                });
            }
            prev = segments;
        }
    }
}

/// A whisper output segment: its text and where it ends, in centiseconds from the
/// start of the audio it was transcribed from.
struct Segment {
    text: String,
    end_cs: i64,
}

/// Runs the VAD over `audio` (device rate) and returns where speech first starts and
/// last ends, in device samples from the start of `audio`, or None without speech.
fn detect_speech(
    vad: &mut WhisperVadContext,
    audio: &[f32],
    rate: u32,
    pause_ms: u32,
) -> Result<Option<(usize, usize)>, WhisperError> {
    let samples = resample_linear(audio, rate, WHISPER_RATE);
    let mut params = WhisperVadParams::new();
    params.set_min_silence_duration(pause_ms as i32);
    params.set_threshold(VAD_THRESHOLD);
    let found: Vec<_> = vad.segments_from_samples(params, &samples)?.collect();
    let (Some(first), Some(last)) = (found.first(), found.last()) else {
        return Ok(None);
    };
    // VAD times are in centiseconds.
    let at = |cs: f32| cs_to_samples(cs as i64, rate).min(audio.len());
    Ok(Some((at(first.start), at(last.end))))
}

/// Transcribes `audio` (device rate) in one pass and returns its segments and how
/// long the pass took in ms.
fn transcribe(
    state: &mut WhisperState,
    audio: &[f32],
    rate: u32,
    language: &str,
) -> Result<(Vec<Segment>, u32), Error> {
    let mut audio = resample_linear(audio, rate, WHISPER_RATE);
    // A final pass can be shorter than whisper's 1 s minimum; pad it with silence.
    audio.resize(audio.len().max((WHISPER_RATE * MIN_SECS) as usize), 0.0);

    // FullParams is consumed by full(), so build a fresh one per pass.
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(language)); // "auto" = detect; e.g. "en" to fix it
    // Every pass re-transcribes the section, so conditioning on earlier text would repeat it.
    params.set_no_context(true);
    params.set_single_segment(false); // segment ends are the candidate cut points
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    params.set_suppress_nst(true); // drop non-speech tokens like "[Music]"

    let started = Instant::now();
    state.full(params, &audio).map_err(Error::Transcribe)?;
    let pass_ms = millis(started.elapsed());

    let segments = state
        .as_iter()
        .map(|seg| Segment {
            text: seg.to_string(),
            end_cs: seg.end_timestamp(),
        })
        .collect();
    Ok((segments, pass_ms))
}

/// The latest segment end (never the last segment of `cur`) where both passes agree:
/// the same text up to there, ignoring case and punctuation, and an end time within
/// CUT_TOLERANCE_MS.
fn agreed_cut(prev: &[Segment], cur: &[Segment]) -> Option<usize> {
    (0..cur.len().saturating_sub(1)).rev().find(|&k| {
        k < prev.len()
            && (prev[k].end_cs - cur[k].end_cs).abs() * 10 <= CUT_TOLERANCE_MS
            && normalize(&joined(&prev[..=k])) == normalize(&joined(&cur[..=k]))
    })
}

/// The segments' text with annotations removed and whitespace collapsed.
fn joined(segments: &[Segment]) -> String {
    let text: Vec<&str> = segments.iter().map(|s| s.text.as_str()).collect();
    strip_annotations(&text.join(" "))
}

/// Lowercases and drops punctuation, so "Hello, world." and "hello world" compare equal.
fn normalize(text: &str) -> String {
    let kept: String = text
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn millis(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

fn samples_to_ms(samples: usize, rate: u32) -> u32 {
    (samples as u64 * 1000 / rate as u64) as u32
}

fn cs_to_samples(cs: i64, rate: u32) -> usize {
    (cs.max(0) as u64 * rate as u64 / 100) as usize
}

/// Removes Whisper's bracketed annotations ("[BLANK_AUDIO]", "(music)", ...)
/// and collapses whitespace.
fn strip_annotations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Simple linear-interpolation resampler (no low-pass filter, so a little
/// aliasing when downsampling, which is fine for a test).
fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (input.len() as f64 / ratio) as usize;

    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = input[idx];
            let b = *input.get(idx + 1).unwrap_or(&a);
            a + (b - a) * frac
        })
        .collect()
}
