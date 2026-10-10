use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::stt::Timing;

pub struct Diagnostics {
    pub last: Option<Update>,
    window: usize,
    intervals: VecDeque<u32>,
    passes: VecDeque<u32>,
}

pub struct Update {
    pub pass_ms: u32,
    timing: Timing,
    arrived: Instant,
    to_ui: Duration,
    interval_ms: Option<u32>,
}

impl Diagnostics {
    pub fn new(window: usize) -> Self {
        Self {
            last: None,
            window,
            intervals: VecDeque::new(),
            passes: VecDeque::new(),
        }
    }

    pub fn record(&mut self, kind: &str, pass_ms: u32, timing: Timing) {
        let now = Instant::now();
        let interval_ms = self
            .last
            .as_ref()
            .map(|last| now.duration_since(last.arrived).as_millis() as u32);
        let update = Update {
            pass_ms,
            timing,
            arrived: now,
            to_ui: now.duration_since(timing.sent_at),
            interval_ms,
        };
        if let Some(interval) = interval_ms {
            push_window(&mut self.intervals, self.window, interval);
        }
        push_window(&mut self.passes, self.window, pass_ms);
        eprintln!("[diag] {kind:5} {}", update.describe());
        self.last = Some(update);
    }

    pub fn summary(&self, render_lag: Option<Duration>) -> String {
        let Some(last) = &self.last else {
            return "No updates yet".into();
        };
        let interval = match last.interval_ms {
            Some(ms) => format!("{ms} ms (avg {} ms)", average(&self.intervals)),
            None => "-".into(),
        };
        let render = render_lag.map_or("-".into(), |lag| format!("{:.1} ms", ms_f64(lag)));
        format!(
            "update every {interval} · {} · avg whisper {} ms · to render {render}",
            last.describe(),
            average(&self.passes),
        )
    }
}

impl Update {
    fn describe(&self) -> String {
        let t = &self.timing;
        format!(
            "whisper {} ms for {:.1} s audio · VAD {} ms · waited {} ms for {} ms new audio · to UI {:.1} ms",
            self.pass_ms,
            t.audio_ms as f64 / 1000.,
            t.vad_ms,
            t.wait_ms,
            t.new_audio_ms,
            ms_f64(self.to_ui),
        )
    }
}

fn push_window(window: &mut VecDeque<u32>, size: usize, value: u32) {
    if window.len() == size {
        window.pop_front();
    }
    window.push_back(value);
}

fn average(window: &VecDeque<u32>) -> u32 {
    let sum: u64 = window.iter().map(|&v| v as u64).sum();
    (sum / window.len().max(1) as u64) as u32
}

pub fn ms_f64(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.
}
