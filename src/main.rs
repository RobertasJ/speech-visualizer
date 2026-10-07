use std::io::Write;
use std::sync::mpsc;
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError,
    WhisperState, WhisperVadContext, WhisperVadContextParams, WhisperVadParams,
};

const WHISPER_RATE: u32 = 16_000;
// Whisper rejects clips shorter than 1 s, so wait for this much before the first pass.
const MIN_SECS: u32 = 1;
// The VAD resets on every call, so each loop it looks at this much of the newest audio.
const VAD_CONTEXT_SECS: u32 = 3;
// Silence this long ends a section (also the VAD's min_silence_duration_ms).
const PAUSE_MS: u32 = 300;
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

fn main() {
    let usage = "usage: just run <path/to/ggml-model.bin> <path/to/ggml-silero.bin>";
    let model_path = std::env::args().nth(1).expect(usage);
    let vad_path = std::env::args().nth(2).expect(usage);

    // Route whisper.cpp's log output into whisper-rs, which drops it without a log backend.
    whisper_rs::install_logging_hooks();

    // --- Whisper: load the model once, reuse the state for every chunk ---
    let ctx = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
        .expect("failed to load model");
    let mut state = ctx.create_state().expect("failed to create state");

    // --- VAD: silero, only used to find where speech starts and pauses ---
    let mut vad = WhisperVadContext::new(&vad_path, WhisperVadContextParams::default())
        .expect("failed to load VAD model");

    // --- Mic: default input device at its native rate/format ---
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    let supported = device
        .default_input_config()
        .expect("no default input config");

    let device_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let sample_format = supported.sample_format();
    let stream_config: cpal::StreamConfig = supported.into();

    println!(
        "{DIM}Recording from '{}' at {} Hz, {} channel(s). Ctrl+C to stop.{RESET}",
        device
            .description()
            .map(|d| d.name().to_owned())
            .unwrap_or_default(),
        device_rate,
        channels
    );

    // The audio callback runs on cpal's thread and sends mono chunks here.
    let (tx, rx) = mpsc::channel::<Vec<f32>>();

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, stream_config, channels, tx),
        SampleFormat::I16 => build_stream::<i16>(&device, stream_config, channels, tx),
        SampleFormat::U16 => build_stream::<u16>(&device, stream_config, channels, tx),
        other => panic!("unsupported sample format: {other:?}"),
    };
    stream.play().expect("failed to start input stream");

    // --- Main loop: sections of speech cut at pauses, re-transcribed until final ---
    let samples = |ms: u32| (device_rate as u64 * ms as u64 / 1000) as usize;
    let min_len = samples(MIN_SECS * 1000);
    let vad_len = samples(VAD_CONTEXT_SECS * 1000);
    let pause_len = samples(PAUSE_MS);
    let pre_roll = samples(PRE_ROLL_MS);
    let trailing = samples(TRAILING_SILENCE_MS);
    let force_len = samples(FORCE_CUT_SECS * 1000);
    let max_len = samples(MAX_SECTION_SECS * 1000);

    // Device-rate audio: the open section, plus whatever came before it that the VAD
    // and pre-roll still need. While idle only the newest VAD_CONTEXT_SECS are kept.
    let mut buf: Vec<f32> = Vec::new();
    // Start of the open section in `buf`; None while idle.
    let mut section: Option<usize> = None;
    // Where the VAD last saw speech end in `buf`, while a section is open.
    let mut speech_end = 0usize;
    // Segments of the previous pass over the open section, to agree on a cut point.
    let mut prev: Vec<Segment> = Vec::new();
    // Text of the open section, shown as the dimmed live tail.
    let mut live = String::new();
    let mut elapsed_ms = 0u128;
    // Transcript words on the current terminal line (earlier lines are printed for good).
    let mut line = String::new();
    // Last (line, live) drawn, so unchanged passes don't redraw (and flicker).
    let mut last_drawn = (String::new(), String::new());

    loop {
        // Block until there's new audio, then take everything that queued up while
        // the previous pass was running so we always work on the latest audio.
        buf.extend(rx.recv().expect("audio channel closed"));
        while let Ok(more) = rx.try_recv() {
            buf.extend(more);
        }

        // VAD over the newest few seconds. On error (e.g. too little audio yet) just
        // wait for more.
        let ctx_start = buf.len().saturating_sub(vad_len);
        let Ok(speech) = detect_speech(&mut vad, &buf[ctx_start..], device_rate) else {
            continue;
        };
        // A pause: no speech at all, or the last speech ended at least PAUSE_MS ago.
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

        let cols = terminal_size::terminal_size().map_or(80, |(w, _)| w.0 as usize);
        let mut out = std::io::stdout().lock();
        let len = buf.len() - start;

        if paused || len >= max_len {
            // The section is over: one final pass over all of it (after a pause, with a
            // little of the trailing silence). Its text is never revised.
            let end = if paused {
                (speech_end + trailing).clamp(start, buf.len())
            } else {
                start + max_len
            };
            let segments;
            (segments, elapsed_ms) = transcribe(&mut state, &buf[start..end], device_rate);
            append_final(&mut out, &mut line, &joined(&segments), cols);
            buf.drain(..end);
            section = None;
            prev.clear();
            live.clear();
        } else if len >= min_len {
            let (mut segments, ms) = transcribe(&mut state, &buf[start..], device_rate);
            elapsed_ms = ms;

            // A long section is cut at a segment end both passes agree on: the text up
            // to there is final, and the rest of the audio starts a new open section.
            if len >= force_len
                && let Some(k) = agreed_cut(&prev, &segments)
            {
                let cut_cs = segments[k].end_cs;
                let cut = (start + cs_to_samples(cut_cs, device_rate)).min(buf.len());
                append_final(&mut out, &mut line, &joined(&segments[..=k]), cols);
                buf.drain(..cut);
                section = Some(0);
                speech_end = speech_end.saturating_sub(cut);
                // Keep the rest relative to the new section start for the next comparison.
                segments.drain(..=k);
                for seg in &mut segments {
                    seg.end_cs -= cut_cs;
                }
            }

            live = joined(&segments);
            prev = segments;
        }

        if (line.as_str(), live.as_str()) != (last_drawn.0.as_str(), last_drawn.1.as_str()) {
            draw_live(&mut out, &line, &live, elapsed_ms, cols);
            out.flush().ok();
            last_drawn = (line.clone(), live.clone());
        }
    }
}

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";
/// Width of the "12345 ms │ " column in front of every line.
const GUTTER: usize = 11;

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
) -> Result<Option<(usize, usize)>, WhisperError> {
    let samples = resample_linear(audio, rate, WHISPER_RATE);
    let mut params = WhisperVadParams::new();
    params.set_min_silence_duration(PAUSE_MS as i32);
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
fn transcribe(state: &mut WhisperState, audio: &[f32], rate: u32) -> (Vec<Segment>, u128) {
    let mut audio = resample_linear(audio, rate, WHISPER_RATE);
    // A final pass can be shorter than whisper's 1 s minimum; pad it with silence.
    audio.resize(audio.len().max((WHISPER_RATE * MIN_SECS) as usize), 0.0);

    // FullParams is consumed by full(), so build a fresh one per pass.
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en")); // "auto" = detect; e.g. "en" to fix it
    // Every pass re-transcribes the section, so conditioning on earlier text would repeat it.
    params.set_no_context(true);
    params.set_single_segment(false); // segment ends are the candidate cut points
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    params.set_suppress_nst(true); // drop non-speech tokens like "[Music]"

    let started = Instant::now();
    state.full(params, &audio).expect("transcription failed");
    let elapsed_ms = started.elapsed().as_millis();

    let segments = state
        .as_iter()
        .map(|seg| Segment {
            text: seg.to_string(),
            end_cs: seg.end_timestamp(),
        })
        .collect();
    (segments, elapsed_ms)
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

fn cs_to_samples(cs: i64, rate: u32) -> usize {
    (cs.max(0) as u64 * rate as u64 / 100) as usize
}

/// Appends final text to the transcript line, printing the line for good whenever
/// the next word wouldn't fit.
fn append_final(out: &mut impl Write, line: &mut String, text: &str, cols: usize) {
    for word in text.split_whitespace() {
        // Wrap the transcript ourselves; a wrapped line can't be redrawn with `\r`.
        if !line.is_empty() && GUTTER + line.chars().count() + 1 + word.chars().count() >= cols {
            writeln!(out, "\r{DIM}{:>GUTTER$}{RESET}{line}\x1b[K", "│ ").ok();
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
}

/// Redraws the current line in place: the settled transcript, then the live tail
/// dimmed. If the tail doesn't fit, only its end is shown.
fn draw_live(out: &mut impl Write, line: &str, live: &str, elapsed_ms: u128, cols: usize) {
    let used = GUTTER + line.chars().count() + usize::from(!line.is_empty());
    let room = cols.saturating_sub(used + 1);

    let len = live.chars().count();
    let shown: String = if len > room {
        let tail: String = live.chars().skip(len - room + 1).collect();
        format!("…{tail}")
    } else {
        live.to_owned()
    };
    let sep = if line.is_empty() { "" } else { " " };
    // Overwrite in place and clear only what's left after the text (\x1b[K), instead
    // of blanking the line first. \x1b[?2026h/l wraps it in a synchronized update so
    // terminals that support it paint the frame at once (others ignore it).
    write!(
        out,
        "\x1b[?2026h\r{DIM}{elapsed_ms:>5} ms │ {RESET}{line}{sep}{DIM}{shown}{RESET}\x1b[K\x1b[?2026l"
    )
    .ok();
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

/// Builds an input stream for sample type `T`, converts samples to f32,
/// and downmixes interleaved channels to mono before sending them on.
fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    tx: mpsc::Sender<Vec<f32>>,
) -> cpal::Stream
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Interleaved: [L, R, L, R, ...] for stereo. Average each frame.
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| {
                        frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
                    })
                    .collect();
                let _ = tx.send(mono);
            },
            |err| eprintln!("stream error: {err}"),
            None,
        )
        .expect("failed to build input stream")
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
