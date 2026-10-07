use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc;
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

const WHISPER_RATE: u32 = 16_000;
const WINDOW_SECS: u32 = 5;
// Whisper rejects clips shorter than 1 s, so wait for this much before the first pass.
const MIN_SECS: u32 = 1;
// Words older than this (from the newest audio) are considered settled and are moved
// from the live tail into the transcript. Must stay below WINDOW_SECS.
const STABLE_SECS: f64 = 2.0;

fn main() {
    let model_path = std::env::args()
        .nth(1)
        .expect("usage: just run <path/to/ggml-model.bin>");

    // Route whisper.cpp's log output into whisper-rs, which drops it without a log backend.
    whisper_rs::install_logging_hooks();

    // --- Whisper: load the model once, reuse the state for every chunk ---
    let ctx = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
        .expect("failed to load model");
    let mut state = ctx.create_state().expect("failed to create state");

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

    // --- Main loop: sliding window over the newest 5 s, transcribed back to back ---
    let window_len = (device_rate * WINDOW_SECS) as usize;
    let min_len = (device_rate * MIN_SECS) as usize;
    let mut window: VecDeque<f32> = VecDeque::with_capacity(window_len * 2);
    // Samples received so far; gives every window an absolute position in time.
    let mut total_samples: u64 = 0;
    // Words ending before this absolute time (secs) are already in the transcript.
    let mut committed_until = 0.0f64;
    // Transcript words on the current terminal line (earlier lines are printed for good).
    let mut line = String::new();
    // Last (line, live) drawn, so unchanged passes don't redraw (and flicker).
    let mut last_drawn = (String::new(), String::new());

    loop {
        // Block until there's new audio, then take everything that queued up while
        // the previous pass was running so we always transcribe the latest window.
        let first = rx.recv().expect("audio channel closed");
        total_samples += first.len() as u64;
        window.extend(first);
        while let Ok(more) = rx.try_recv() {
            total_samples += more.len() as u64;
            window.extend(more);
        }
        if window.len() > window_len {
            window.drain(..window.len() - window_len);
        }
        if window.len() < min_len {
            continue;
        }

        let now = total_samples as f64 / device_rate as f64;
        let window_start = now - window.len() as f64 / device_rate as f64;
        let audio = resample_linear(window.make_contiguous(), device_rate, WHISPER_RATE);

        // FullParams is consumed by full(), so build a fresh one per pass.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("auto")); // "auto" = detect; e.g. "en" to fix it
        // Windows overlap, so conditioning on the previous pass would repeat text.
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_token_timestamps(true); // per-token times, to know which words are settled
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_suppress_nst(true); // drop non-speech tokens like "[Music]"

        let started = Instant::now();
        state.full(params, &audio).expect("transcription failed");
        let elapsed_ms = started.elapsed().as_millis();

        // Only words not yet in the transcript. Using the midpoint tolerates the
        // timestamps of the same word jittering a little between passes.
        let words: Vec<Word> = window_words(&state, ctx.token_eot(), window_start)
            .into_iter()
            .filter(|w| (w.start + w.end) / 2.0 > committed_until)
            .collect();

        // Settled words (a leading run, so order is kept) go to the transcript;
        // the rest is the live tail that may still change.
        let settled = words
            .iter()
            .take_while(|w| w.end <= now - STABLE_SECS)
            .count();
        let (done, pending) = words.split_at(settled);
        let live = pending
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let cols = terminal_size::terminal_size().map_or(80, |(w, _)| w.0 as usize);
        let mut out = std::io::stdout().lock();
        for word in done {
            committed_until = word.end;
            // Wrap the transcript ourselves; a wrapped line can't be redrawn with `\r`.
            if !line.is_empty()
                && GUTTER + line.chars().count() + 1 + word.text.chars().count() >= cols
            {
                writeln!(out, "\r{DIM}{:>GUTTER$}{RESET}{line}\x1b[K", "│ ").ok();
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&word.text);
        }

        if (line.as_str(), live.as_str()) != (last_drawn.0.as_str(), last_drawn.1.as_str()) {
            draw_live(&mut out, &line, &live, elapsed_ms, cols);
            out.flush().ok();
            last_drawn = (line.clone(), live);
        }
    }
}

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";
/// Width of the "12345 ms │ " column in front of every line.
const GUTTER: usize = 11;

/// A transcribed word with absolute start/end times in seconds.
struct Word {
    text: String,
    start: f64,
    end: f64,
}

/// Groups the tokens of the last pass into words (a token starting with a space
/// begins a new word) and places them in absolute time.
fn window_words(state: &WhisperState, eot: i32, window_start: f64) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    for seg in state.as_iter() {
        for i in 0..seg.n_tokens() {
            let Some(token) = seg.get_token(i) else {
                continue;
            };
            if token.token_id() >= eot {
                continue; // special tokens: [_BEG_], timestamps, ...
            }
            let Ok(piece) = token.to_str_lossy() else {
                continue;
            };
            let data = token.token_data();
            // Token times are in centiseconds from the start of the window.
            let (t0, t1) = (
                window_start + data.t0 as f64 / 100.0,
                window_start + data.t1 as f64 / 100.0,
            );
            match words.last_mut() {
                Some(w) if !piece.starts_with(' ') => {
                    w.text.push_str(&piece);
                    w.end = t1;
                }
                _ => words.push(Word {
                    text: piece.trim().to_owned(),
                    start: t0,
                    end: t1,
                }),
            }
        }
    }
    // Drop annotation "words" such as "[BLANK_AUDIO]".
    words.retain_mut(|w| {
        w.text = strip_annotations(&w.text);
        !w.text.is_empty()
    });
    words
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
