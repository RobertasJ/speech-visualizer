//! Live mode: what's said is transcribed and checked with TypeSafe AI, and an agent acts
//! on what needs it, changing the scene with scripts.
//!
//! - [`transcript`]: what's been said, in segments.
//! - [`check`]: asks TypeSafe AI whether a segment needs the agent, or corrects it.
//! - [`schedule`]: decides when the agent runs.
//! - [`agent`]: one run of it, and [`history`]: what it did in each run.
//! - [`ui`]: the panels the live screen shows all this in.

mod agent;
mod check;
mod history;
mod schedule;
mod transcript;
mod ui;

use std::time::Instant;

use freya::prelude::*;
use rig::Model;
use rig::providers::anthropic::Messages;

use crate::scene::{Scene, Script};
use crate::stt::{Event, Status, use_stt};
use agent::Input;
use check::{Check, Verdict};
use history::History;
use schedule::{Decision, Schedule};
use transcript::{Checked, Transcript};

pub use ui::{ActivityPanel, TranscriptPanel};

/// Transcribes the microphone for as long as the component is mounted, and has an
/// agent turn what is said into changes to `scene`.
pub fn use_live(scene: State<Scene>) -> UseLive {
    let live = UseLive {
        scene,
        script: use_state(|| Script::new(scene)),
        status: use_state(|| Status::Loading),
        transcript: use_state(Transcript::default),
        schedule: use_state(Schedule::default),
        history: use_state(History::default),
        model: use_state(agent::model),
        agent: use_state(|| None),
    };
    let mut on_event = live;
    use_stt(move |event| on_event.on_event(event));
    live
}

/// Returned by [`use_live`]. Turns transcription events into scene changes, on the UI
/// thread, and has the state the live screen shows.
#[derive(Clone, Copy, PartialEq)]
pub struct UseLive {
    scene: State<Scene>,
    /// Runs the agent's scripts. Its variables last as long as the screen.
    script: State<Script>,
    status: State<Status>,
    transcript: State<Transcript>,
    schedule: State<Schedule>,
    history: State<History>,
    /// The model the agent runs on, or why there's none.
    model: State<Result<Model<Messages>, String>>,
    /// The running agent; at most one at a time.
    agent: State<Option<TaskHandle>>,
}

impl UseLive {
    /// Whether the models are loading, listening, or failed.
    pub fn status(&self) -> State<Status> {
        self.status
    }

    pub fn transcript(&self) -> State<Transcript> {
        self.transcript
    }

    pub fn history(&self) -> State<History> {
        self.history
    }

    pub fn schedule(&self) -> State<Schedule> {
        self.schedule
    }

    /// Called for every transcription event, in order.
    fn on_event(&mut self, event: Event) {
        match event {
            Event::Started(device) => self.status.set(Status::Listening(device)),
            Event::Failed(err) => self.status.set(Status::Failed(err)),
            // The open section's text so far; it may still change.
            Event::Live { text, .. } => self.transcript.write().set_live(text),
            // Text that won't change anymore: check whether the agent should act on it.
            Event::Final { text, .. } => {
                let mut transcript = self.transcript.write();
                transcript.set_live(String::new());
                if text.is_empty() {
                    return;
                }
                let number = transcript.push(text);
                drop(transcript);
                self.check(number);
            }
            Event::Error(_) => {}
        }
    }

    /// Asks TypeSafe AI about segment `number`, then does what the schedule decides.
    fn check(&self, number: usize) {
        let running = *self.agent.peek();
        let transcript = self.transcript.peek();
        // What the running agent was given, if one is running.
        let given = self.schedule.peek().given();
        let acting_on = running.map(|_| transcript.text(transcript.pending()..given));
        let check = Check::new(
            &transcript.text(0..number),
            acting_on.as_deref(),
            &transcript.segments()[number].text,
        );
        drop(transcript);

        let mut live = *self;
        spawn(async move {
            let started = Instant::now();
            let verdict = check.send().await.map_err(|err| err.to_string());
            let took = started.elapsed();

            let needed = check::needs_agent(&verdict);
            let now_running = *live.agent.peek();
            let corrects = verdict.as_ref().is_ok_and(Verdict::corrects);
            // Only the run it was checked against is restarted.
            let restart = corrects && running.is_some() && now_running == running;
            let decision =
                live.schedule
                    .write()
                    .decide(number, needed, restart, now_running.is_some());
            let checked = Checked {
                verdict,
                decision,
                took,
            };
            live.transcript.write().checked(number, checked);
            if matches!(decision, Decision::Started | Decision::Restarted) {
                live.start();
            }
        });
    }

    /// Starts the agent on everything said so far, cancelling the running one, if any.
    fn start(&mut self) {
        if let Some(agent) = self.agent.write().take() {
            agent.cancel();
            self.history.write().cancelled();
        }

        let transcript = self.transcript.peek();
        let end = transcript.count();
        let new_text = transcript.text(transcript.pending()..end);
        let model = match &*self.model.peek() {
            Ok(model) => model.clone(),
            Err(error) => {
                let mut history = self.history.write();
                history.begin(new_text);
                history.failed(error.clone());
                history.end();
                return;
            }
        };
        // Listing the variables goes through the scene's store, which borrows the scene
        // mutably, so it comes before peeking at the scene.
        let variables = self.script.peek().variables().into_iter().collect();
        let input = Input {
            transcript: transcript.text(0..end),
            new_text: new_text.clone(),
            responses: self.history.peek().rendered(),
            scene: &self.scene.peek(),
            variables,
        }
        .to_json();
        drop(transcript);

        self.schedule.write().start(end);
        self.history.write().begin(new_text);
        let mut live = *self;
        let agent = spawn(async move {
            let finished = agent::run(model, input, live.history, live.script, live.scene).await;
            live.agent.set(None);
            live.history.write().end();
            if finished {
                live.finished(end);
            }
        });
        self.agent.set(Some(agent));
    }

    /// A run finished: the segments numbered below `end` are handled, and it runs again
    /// if any that came in meanwhile need it.
    fn finished(&mut self, end: usize) {
        self.transcript.write().handled(end);
        if self.schedule.peek().wants_another() {
            self.start();
        }
    }
}
