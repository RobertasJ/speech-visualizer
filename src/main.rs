mod stt;

use std::cell::Cell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use freya::prelude::*;
use futures_channel::{mpsc, oneshot};
use futures_lite::StreamExt;

use stt::{Config, DeviceInfo, Event, Timing, Transcriber};

/// Language codes offered in the selection, with "auto" for whisper's detection.
const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("auto", "Auto-detect"),
    ("lt", "Lithuanian"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("pl", "Polish"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
];
// Range of the pause slider; the default is what the terminal version used.
const PAUSE_MIN_MS: u32 = 100;
const PAUSE_MAX_MS: u32 = 1000;
const PAUSE_DEFAULT_MS: u32 = 300;
// How many of the latest updates the diagnostics average over.
const DIAG_WINDOW: usize = 20;

fn main() {
    // Model files (.bin) are picked from this directory; VAD models have "silero" in the name.
    let models_dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "models".into()));

    launch(
        LaunchConfig::new().with_window(
            WindowConfig::new_app(SpeechApp { models_dir })
                .with_size(900., 600.)
                .with_title("Speech visualizer"),
        ),
    )
}

/// What the user picked on the selection screen.
#[derive(Clone, PartialEq)]
struct Options {
    model: Option<PathBuf>,
    vad: Option<PathBuf>,
    language: &'static str,
    pause_ms: u32,
}

struct SpeechApp {
    models_dir: PathBuf,
}

impl App for SpeechApp {
    fn render(&self) -> impl IntoElement {
        // Follow the OS light/dark preference.
        let mut theme = use_init_theme(|| Platform::get().preferred_theme.read().to_theme());
        use_side_effect(move || theme.set(Platform::get().preferred_theme.read().to_theme()));

        // Kept here, so going back to the selection shows the last choices.
        let options = use_state(|| Options {
            model: None,
            vad: None,
            language: LANGUAGES[0].0,
            pause_ms: PAUSE_DEFAULT_MS,
        });
        let running = use_state(|| false);

        let screen = if running() {
            Session {
                options: options.read().clone(),
                running,
            }
            .into_element()
        } else {
            Selection {
                models_dir: self.models_dir.clone(),
                options,
                running,
            }
            .into_element()
        };

        rect()
            .expanded()
            .theme_background()
            .theme_color()
            .child(screen)
    }
}

/// The options form; Start switches to a [`Session`].
#[derive(PartialEq)]
struct Selection {
    models_dir: PathBuf,
    options: State<Options>,
    running: State<bool>,
}

impl Component for Selection {
    fn render(&self) -> impl IntoElement {
        let mut options = self.options;
        let mut running = self.running;

        // Scanned on every visit, so models added in the meantime show up.
        let (models, vads, scan_error) = use_hook(|| match bin_files(&self.models_dir) {
            Ok(files) => {
                let (vads, models) = files.into_iter().partition(|path| is_vad(path));
                (models, vads, None)
            }
            Err(err) => (
                Vec::new(),
                Vec::new(),
                Some(format!("Can't read {}: {err}", self.models_dir.display())),
            ),
        });
        // Keep earlier picks that still exist, otherwise default to the first file.
        use_hook({
            let (models, vads) = (models.clone(), vads.clone());
            move || {
                let mut options = options.write();
                if !options.model.as_ref().is_some_and(|m| models.contains(m)) {
                    options.model = models.first().cloned();
                }
                if !options.vad.as_ref().is_some_and(|v| vads.contains(v)) {
                    options.vad = vads.first().cloned();
                }
            }
        });

        let current = options.read().clone();
        let colors = use_theme().read().colors.clone();
        let can_start = current.model.is_some() && current.vad.is_some();

        let language = Select::new()
            .selected_item(language_name(current.language))
            .children(LANGUAGES.iter().map(|&(code, name)| {
                MenuItem::new()
                    .selected(current.language == code)
                    .on_press(move |_| options.write().language = code)
                    .child(name)
            }));

        let pause_value =
            (current.pause_ms - PAUSE_MIN_MS) as f64 * 100. / (PAUSE_MAX_MS - PAUSE_MIN_MS) as f64;
        let pause = Slider::new(move |value: f64| {
            let ms = PAUSE_MIN_MS as f64 + value / 100. * (PAUSE_MAX_MS - PAUSE_MIN_MS) as f64;
            // Snap to 10 ms steps.
            options.write().pause_ms = (ms / 10.).round() as u32 * 10;
        })
        .value(pause_value)
        .size(Size::px(250.));

        let mut form = rect()
            .width(Size::px(420.))
            .spacing(16.)
            .child(label().text("Speech visualizer").font_size(26.))
            .child(field(
                "Whisper model",
                file_select(&models, &current.model, move |path| {
                    options.write().model = Some(path)
                }),
            ))
            .child(field(
                "VAD model",
                file_select(&vads, &current.vad, move |path| {
                    options.write().vad = Some(path)
                }),
            ))
            .child(field("Language", language))
            .child(field(
                format!("Pause that ends a section: {} ms", current.pause_ms),
                pause,
            ));

        if let Some(scan_error) = scan_error {
            form = form.child(label().text(scan_error).color(colors.error));
        } else if !can_start {
            form = form.child(
                label()
                    .text(format!(
                        "Put a whisper model and a silero VAD model (.bin) in {}.",
                        self.models_dir.display()
                    ))
                    .color(colors.text_secondary),
            );
        }

        rect().expanded().center().child(
            form.child(
                Button::new()
                    .filled()
                    .enabled(can_start)
                    .on_press(move |_| running.set(true))
                    .child("Start"),
            ),
        )
    }
}

/// A labelled row of the selection form.
fn field(title: impl Into<String>, input: impl IntoElement) -> impl IntoElement {
    rect()
        .spacing(6.)
        .child(label().text(title.into()).font_size(14.))
        .child(input)
}

/// A dropdown of `files`, showing their file names.
fn file_select(
    files: &[PathBuf],
    selected: &Option<PathBuf>,
    on_pick: impl FnMut(PathBuf) + Clone + 'static,
) -> impl IntoElement {
    let shown = selected.as_deref().map_or("None found".into(), file_name);
    Select::new()
        .selected_item(shown)
        .children(files.iter().map(|path| {
            let mut on_pick = on_pick.clone();
            let picked = path.clone();
            MenuItem::new()
                .selected(selected.as_ref() == Some(path))
                .on_press(move |_| on_pick(picked.clone()))
                .child(file_name(path))
        }))
}

/// Where the transcriber is, as shown in the top bar.
enum Status {
    Loading,
    Listening(DeviceInfo),
    Stopped,
    Failed(String),
}

/// A running transcription. Its transcriber is started on mount and stopped when it
/// unmounts, i.e. when going back to the selection.
#[derive(PartialEq)]
struct Session {
    options: Options,
    running: State<bool>,
}

impl Component for Session {
    fn render(&self) -> impl IntoElement {
        let mut running = self.running;
        let mut status = use_state(|| Status::Loading);
        let mut transcript = use_state(String::new);
        let mut live = use_state(String::new);
        let mut diagnostics = use_state(Diagnostics::default);
        // When the latest update arrived, until the render that shows it takes it.
        let arrived = use_hook(|| Rc::new(Cell::new(None::<Instant>)));
        // How long after arriving the latest update was rendered.
        let render_lag = use_hook(|| Rc::new(Cell::new(None::<Duration>)));
        let mut errors = use_state(Vec::<String>::new);
        let mut scroll = use_scroll_controller(|| ScrollConfig {
            default_vertical_position: ScrollPosition::End,
            ..Default::default()
        });

        use_hook(|| {
            let options = &self.options;
            let config = Config {
                model_path: options.model.clone().unwrap_or_default(),
                vad_path: options.vad.clone().unwrap_or_default(),
                language: (options.language != "auto").then(|| options.language.to_owned()),
                pause_ms: options.pause_ms,
            };

            // start() blocks while the models load, so it runs on its own thread. If the
            // session is gone by the time it's done, the transcriber is dropped right there.
            let (setup_tx, setup_rx) = oneshot::channel();
            let (event_tx, mut event_rx) = mpsc::unbounded();
            std::thread::spawn(move || {
                let started = Transcriber::start(config, move |event| {
                    let _ = event_tx.unbounded_send(event);
                });
                let _ = setup_tx.send(started);
            });

            // Cancelled when the session unmounts, which drops the transcriber.
            let arrived = arrived.clone();
            spawn(async move {
                let transcriber = match setup_rx.await {
                    Ok(Ok(transcriber)) => transcriber,
                    Ok(Err(err)) => {
                        status.set(Status::Failed(err.to_string()));
                        return;
                    }
                    Err(_) => {
                        status.set(Status::Failed("the setup thread panicked".into()));
                        return;
                    }
                };
                status.set(Status::Listening(transcriber.device().clone()));
                let _transcriber = StopInBackground(Some(transcriber));

                // The event channel closes when the transcription thread ends.
                while let Some(event) = event_rx.next().await {
                    match event {
                        Event::Live {
                            text,
                            pass_ms,
                            timing,
                        } => {
                            diagnostics.write().record("live", pass_ms, timing);
                            arrived.set(Some(Instant::now()));
                            live.set(text);
                        }
                        Event::Final {
                            text,
                            pass_ms,
                            timing,
                            ..
                        } => {
                            diagnostics.write().record("final", pass_ms, timing);
                            arrived.set(Some(Instant::now()));
                            if !text.is_empty() {
                                let mut transcript = transcript.write();
                                if !transcript.is_empty() {
                                    transcript.push(' ');
                                }
                                transcript.push_str(&text);
                            }
                            live.set(String::new());
                        }
                        Event::Error(err) => errors.write().push(err),
                    }
                    scroll.scroll_to(ScrollPosition::End, Direction::Vertical);
                }
                status.set(Status::Stopped);
            });
        });

        if let Some(at) = arrived.take() {
            let lag = at.elapsed();
            render_lag.set(Some(lag));
            eprintln!("[diag] render {:.1} ms after arriving", ms_f64(lag));
        }

        let colors = use_theme().read().colors.clone();

        let status_text = match &*status.read() {
            Status::Loading => "Loading models…".to_owned(),
            Status::Listening(device) => format!(
                "Recording from '{}' at {} Hz, {} channel(s)",
                device.name, device.sample_rate, device.channels
            ),
            Status::Stopped => "Stopped".to_owned(),
            Status::Failed(err) => format!("Failed to start: {err}"),
        };

        let top_bar = rect()
            .horizontal()
            .width(Size::fill())
            .padding(10.)
            .spacing(12.)
            .cross_align(Alignment::center())
            .background(colors.surface_primary)
            .child(
                Button::new()
                    .on_press(move |_| running.set(false))
                    .child("← Back to selection"),
            )
            .child(
                label()
                    .width(Size::flex(1.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .text(status_text)
                    .color(colors.text_secondary),
            )
            .child(
                label()
                    .text(format!(
                        "{} ms",
                        diagnostics.read().last.as_ref().map_or(0, |u| u.pass_ms)
                    ))
                    .font_tabular()
                    .color(colors.text_secondary),
            );
        let diagnostics_bar = label()
            .width(Size::fill())
            .padding((4., 12.))
            .font_size(12.)
            .font_tabular()
            .text(diagnostics.read().summary(render_lag.get()))
            .color(colors.text_secondary);

        let sep = if transcript.read().is_empty() {
            ""
        } else {
            " "
        };
        let text = paragraph()
            .width(Size::fill())
            .font_size(22.)
            .line_height(1.4)
            .span(Span::new(transcript.read().clone()))
            .span(Span::new(format!("{sep}{}", live.read())).color(colors.text_secondary));

        rect()
            .expanded()
            .content(Content::Flex)
            .child(top_bar)
            .child(diagnostics_bar)
            .children(errors.read().iter().map(|err| {
                label()
                    .padding((4., 12.))
                    .text(err.clone())
                    .color(colors.error)
                    .into_element()
            }))
            .child(
                ScrollView::new_controlled(scroll)
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(rect().padding(20.).child(text)),
            )
    }
}

/// Timings of the latest text updates (Live and Final events), shown under the top bar
/// and logged to stderr.
#[derive(Default)]
struct Diagnostics {
    last: Option<Update>,
    /// Gaps between the latest updates arriving, in ms.
    intervals: VecDeque<u32>,
    /// Passes behind the latest updates, in ms.
    passes: VecDeque<u32>,
}

struct Update {
    pass_ms: u32,
    timing: Timing,
    arrived: Instant,
    /// From the worker sending the event to it arriving here.
    to_ui: Duration,
    /// Since the update before it arrived.
    interval_ms: Option<u32>,
}

impl Diagnostics {
    fn record(&mut self, kind: &str, pass_ms: u32, timing: Timing) {
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
            push_window(&mut self.intervals, interval);
        }
        push_window(&mut self.passes, pass_ms);
        eprintln!("[diag] {kind:5} {}", update.describe());
        self.last = Some(update);
    }

    fn summary(&self, render_lag: Option<Duration>) -> String {
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

fn push_window(window: &mut VecDeque<u32>, value: u32) {
    if window.len() == DIAG_WINDOW {
        window.pop_front();
    }
    window.push_back(value);
}

fn average(window: &VecDeque<u32>) -> u32 {
    let sum: u64 = window.iter().map(|&v| v as u64).sum();
    (sum / window.len().max(1) as u64) as u32
}

fn ms_f64(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.
}

/// Owns the transcriber and drops it on a separate thread: dropping waits for the pass in
/// progress, which would otherwise freeze the window.
struct StopInBackground(Option<Transcriber>);

impl Drop for StopInBackground {
    fn drop(&mut self) {
        if let Some(transcriber) = self.0.take() {
            std::thread::spawn(move || drop(transcriber));
        }
    }
}

/// The .bin files in `dir`, sorted by name.
fn bin_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "bin"))
        .collect();
    files.sort();
    Ok(files)
}

fn is_vad(path: &Path) -> bool {
    file_name(path).contains("silero")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn language_name(code: &str) -> &'static str {
    LANGUAGES
        .iter()
        .find(|&&(c, _)| c == code)
        .map_or("Unknown", |&(_, name)| name)
}
