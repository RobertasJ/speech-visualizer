use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use freya::prelude::*;
use futures_channel::mpsc;
use futures_lite::StreamExt;

use super::diagnostics::{Diagnostics, ms_f64};
use super::transcriber::{Status, start_transcriber, use_stop_in_background, wait_for_transcriber};
use crate::Screen;
use crate::options::Options;
use crate::stt::{Event, Transcriber};

#[derive(PartialEq)]
pub struct Session {
    pub options: Options,
    pub screen: State<Screen>,
    pub transcriber: State<Option<Transcriber>>,
}

impl Component for Session {
    fn render(&self) -> impl IntoElement {
        let mut screen = self.screen;
        let transcriber = self.transcriber;
        let mut status = use_state(|| Status::Loading);
        let mut transcript = use_state(String::new);
        let mut live = use_state(String::new);
        let mut diagnostics = use_state(|| Diagnostics::new(self.options.diag_window));
        // When the latest update arrived, until the render that shows it takes it.
        let arrived = use_hook(|| Rc::new(Cell::new(None::<Instant>)));
        let render_lag = use_hook(|| Rc::new(Cell::new(None::<Duration>)));
        let mut errors = use_state(Vec::<String>::new);
        let mut scroll = use_scroll_controller(|| ScrollConfig {
            default_vertical_position: ScrollPosition::End,
            ..Default::default()
        });

        use_stop_in_background(transcriber);
        use_hook(|| {
            let (event_tx, mut event_rx) = mpsc::unbounded();
            let setup = start_transcriber(&self.options, move |event| {
                let _ = event_tx.unbounded_send(event);
            });

            // Cancelled when the session unmounts.
            let arrived = arrived.clone();
            spawn(async move {
                if !wait_for_transcriber(setup, status, transcriber).await {
                    return;
                }

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

        let status_text = status.read().to_string();

        let top_bar = rect()
            .horizontal()
            .width(Size::fill())
            .padding(10.)
            .spacing(12.)
            .cross_align(Alignment::center())
            .background(colors.surface_primary)
            .child(
                Button::new()
                    .on_press(move |_| screen.set(Screen::Selection))
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
            .span(
                Span::new(format!("{sep}{}", live.read()))
                    .color(colors.text_secondary)
                    .font_slant(FontSlant::Italic),
            );

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
