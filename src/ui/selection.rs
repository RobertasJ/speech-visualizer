use std::path::{Path, PathBuf};

use freya::prelude::*;

use crate::Screen;
use crate::options::Options;

#[derive(PartialEq)]
pub struct Selection {
    pub options: State<Options>,
    pub screen: State<Screen>,
}

impl Component for Selection {
    fn render(&self) -> impl IntoElement {
        let mut options = self.options;
        let mut screen = self.screen;

        // Scanned on every visit, so models added in the meantime show up.
        let (models, vads, scan_error) = use_hook(|| {
            let models_dir = options.peek().models_dir.clone();
            match bin_files(&models_dir) {
                Ok(files) => {
                    let (vads, models) = files.into_iter().partition(|path| is_vad(path));
                    (models, vads, None)
                }
                Err(err) => (
                    Vec::new(),
                    Vec::new(),
                    Some(format!("Can't read {}: {err}", models_dir.display())),
                ),
            }
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
            .selected_item(language_name(current.languages, current.language))
            .children(current.languages.iter().map(|&(code, name)| {
                MenuItem::new()
                    .selected(current.language == code)
                    .on_press(move |_| options.write().language = code)
                    .child(name)
            }));

        let (pause_min, pause_max) = (current.pause_min_ms, current.pause_max_ms);
        let pause_value =
            (current.pause_ms - pause_min) as f64 * 100. / (pause_max - pause_min) as f64;
        let pause = Slider::new(move |value: f64| {
            let ms = pause_min as f64 + value / 100. * (pause_max - pause_min) as f64;
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
                        current.models_dir.display()
                    ))
                    .color(colors.text_secondary),
            );
        }

        let buttons = rect()
            .horizontal()
            .spacing(8.)
            .child(
                Button::new()
                    .filled()
                    .enabled(can_start)
                    .on_press(move |_| screen.set(Screen::Transcript))
                    .child("Start"),
            )
            .child(
                Button::new()
                    .enabled(can_start)
                    .on_press(move |_| screen.set(Screen::Live))
                    .child("Live"),
            )
            .child(
                Button::new()
                    .on_press(move |_| screen.set(Screen::Console))
                    .child("Open display without transcript"),
            );

        rect().expanded().center().child(form.child(buttons))
    }
}

fn field(title: impl Into<String>, input: impl IntoElement) -> impl IntoElement {
    rect()
        .spacing(6.)
        .child(label().text(title.into()).font_size(14.))
        .child(input)
}

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

fn language_name(languages: &[(&str, &'static str)], code: &str) -> &'static str {
    languages
        .iter()
        .find(|&&(c, _)| c == code)
        .map_or("Unknown", |&(_, name)| name)
}
