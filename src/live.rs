use freya::prelude::*;
use futures_lite::StreamExt;
use rig::Model;
use rig::completion::CompletionRequest;
use rig::providers::anthropic::{self, Anthropic, Messages};
use rig::streaming::{Item, StreamEvent};
use serde_json::json;

use crate::scene::{Command, Names, Scene};
use crate::stt::{Event, Status, use_stt};
use crate::typesafeai::{self, Noul, Request};

const MODEL: &str = anthropic::completion::CLAUDE_SONNET_5_5;

/// The TypeSafe AI question asking whether new text needs the agent at all.
const NEW: &str = "new";
/// The TypeSafe AI question asking whether new text corrects what the agent is doing.
const CORRECTION: &str = "correction";
/// How likely a yes from TypeSafe AI has to be to count.
const THRESHOLD: f32 = 0.5;
/// Ends the response of a run that was cancelled. Only a correction cancels one.
const CANCELLED: &str = "> cancelled: new speech corrected this\n";

/// Transcribes the microphone for as long as the component is mounted, and has an
/// agent turn what is said into changes to `scene`.
pub fn use_live(scene: State<Scene>, names: State<Names>) -> UseLive {
    let live = UseLive {
        scene,
        names,
        status: use_state(|| Status::Loading),
        transcript: use_state(String::new),
        live_text: use_state(String::new),
        new_text: use_state(String::new),
        acting_on: use_state(|| 0),
        received: use_state(|| 0),
        given: use_state(|| 0),
        wanted: use_state(|| 0),
        responses: use_state(Vec::new),
        writing: use_state(String::new),
        thinking: use_state(|| false),
        checking: use_state(|| 0),
        queued: use_state(|| 0),
        model: use_state(|| match Anthropic::from_env() {
            Ok(provider) => Some(provider.completion(MODEL)),
            Err(err) => {
                eprintln!("[agent] {err}");
                None
            }
        }),
        agent: use_state(|| None),
    };
    let mut on_event = live;
    use_stt(move |event| on_event.on_event(event));
    live
}

/// Returned by [`use_live`]. Turns transcription events into scene changes, on the UI
/// thread.
#[derive(Clone, Copy, PartialEq)]
pub struct UseLive {
    scene: State<Scene>,
    names: State<Names>,
    status: State<Status>,
    transcript: State<String>,
    /// The open section's text so far, not in the transcript yet.
    live_text: State<String>,
    /// Text added to the transcript since the agent last finished a run.
    new_text: State<String>,
    /// How much of `new_text` the current or latest run was given; the rest came in
    /// while it was running.
    acting_on: State<usize>,
    /// How many texts have been added to `new_text`, so also the next one's number.
    received: State<u64>,
    /// How many texts the current or latest run was given: those numbered below it.
    given: State<u64>,
    /// One more than the number of the latest text that needs the agent, or 0. The
    /// agent runs again when it's done if this is more than `given`.
    wanted: State<u64>,
    /// Every response of the agent, one per run, as it wrote them, with a `> ` line
    /// after each command saying how it went. A cancelled run's ends early.
    responses: State<Vec<String>>,
    /// The line the agent is writing, until it's complete and added to its response.
    writing: State<String>,
    /// Whether the agent is running.
    thinking: State<bool>,
    /// How many texts are waiting on TypeSafe AI to say whether they need the agent.
    checking: State<usize>,
    /// How many texts that need the agent came in while the current run was going. It
    /// runs again for them when it's done.
    queued: State<usize>,
    /// None if the API key isn't set.
    model: State<Option<Model<Messages>>>,
    /// The running agent; at most one at a time.
    agent: State<Option<TaskHandle>>,
}

impl UseLive {
    /// Whether the models are loading, listening, or failed.
    pub fn status(&self) -> State<Status> {
        self.status
    }

    /// Everything said so far that won't change anymore, with a space between sections.
    pub fn transcript(&self) -> State<String> {
        self.transcript
    }

    /// What's being said right now; it may still change.
    pub fn live_text(&self) -> State<String> {
        self.live_text
    }

    /// The agent's responses, one per run, oldest first. The last one is the current or
    /// latest run's.
    pub fn responses(&self) -> State<Vec<String>> {
        self.responses
    }

    /// The line the agent is writing, not in its response yet.
    pub fn writing(&self) -> State<String> {
        self.writing
    }

    /// Whether the agent is running.
    pub fn thinking(&self) -> State<bool> {
        self.thinking
    }

    /// How many texts are being checked for whether they need the agent.
    pub fn checking(&self) -> State<usize> {
        self.checking
    }

    /// How many texts that need the agent came in while it was running, for its next
    /// run.
    pub fn queued(&self) -> State<usize> {
        self.queued
    }

    /// Called for every transcription event, in order: work out what the display should
    /// show and send commands for it.
    fn on_event(&mut self, event: Event) {
        match event {
            Event::Started(device) => self.status.set(Status::Listening(device)),
            Event::Failed(err) => self.status.set(Status::Failed(err)),
            // The open section's text so far; it may still change.
            Event::Live { text, .. } => self.live_text.set(text),
            // Text that won't change anymore: have the agent act on it if it needs to.
            Event::Final { text, .. } => {
                self.live_text.set(String::new());
                if text.is_empty() {
                    return;
                }
                for mut joined in [self.transcript, self.new_text] {
                    let mut joined = joined.write();
                    if !joined.is_empty() {
                        joined.push(' ');
                    }
                    joined.push_str(&text);
                }
                let number = *self.received.peek();
                self.received.set(number + 1);
                self.check_text(text, number);
            }
            Event::Error(_) => {}
        }
    }

    /// Asks TypeSafe AI whether text number `number` asks for anything new, and if the
    /// agent is running, whether it corrects what that run is acting on. Then has the
    /// agent act on it if it needs to.
    fn check_text(&mut self, text: String, number: u64) {
        let running = *self.agent.peek();
        let new_text = self.new_text.peek();
        let acting_on = running.map(|_| &new_text[..*self.acting_on.peek()]);
        // Without `text`: given the whole transcript, it reads `text` as already asked for.
        let transcript = self.transcript.peek();
        let earlier = transcript[..transcript.len() - text.len()].trim_end();
        let mut request = Request::new(json!({
            "earlier": earlier,
            "acting_on": acting_on,
            "new_text": text,
        }))
        .with_question(
            NEW,
            Noul::new(
                "Someone is speaking to an assistant that changes what a display shows. \
                 `earlier` is what they said before `new_text`, and `acting_on` is what \
                 the assistant is carrying out right now, if anything. Does `new_text` ask for anything to be done to the display that hasn't \
                 been asked for already?",
            )
            .with_criteria(
                "It asks for something to be added, changed, named, removed or cleared, \
                 or corrects or takes back something asked for, even in passing.",
                "It's filler, thinking out loud, talk not meant for the assistant, or \
                 only repeats what was already asked for.",
            ),
        );
        if running.is_some() {
            request = request.with_question(
                CORRECTION,
                Noul::new(
                    "Someone is speaking commands that change what a display shows, and \
                     an assistant is carrying out `acting_on`. `earlier` is what they \
                     said before `new_text`. Does `new_text` correct it, rather than \
                     asking for something more?",
                )
                .with_criteria(
                    "It changes, fixes or takes back what `acting_on` asks for, like \
                     \"no, the other box\", \"I meant blue\" or \"wait, not that\", \
                     or fixes a word in it that was misheard.",
                    "It asks for something new, adds to what `acting_on` asks for, or \
                     isn't a request at all.",
                ),
            );
        }
        drop((new_text, transcript));

        *self.checking.write() += 1;
        let mut live = *self;
        spawn(async move {
            let response = typesafeai::send(&request).await;
            *live.checking.write() -= 1;
            let scores = match response {
                Ok(response) => {
                    let correction = running.map(|_| response.get_noul(CORRECTION));
                    Some((response.get_noul(NEW), correction))
                }
                Err(err) => {
                    eprintln!("[check] {err}");
                    None
                }
            };
            // Better to run the agent for nothing than to miss a request.
            let (new, correction) = scores.unwrap_or((1., None));
            let correction = correction.is_some_and(|correction| correction > THRESHOLD);
            let outcome = if new > THRESHOLD || correction {
                live.act_on(number, running, correction)
            } else {
                "ignored"
            };

            let scores = match scores {
                Some((new, Some(correction))) => {
                    format!("new {new:.2}, correction {correction:.2}")
                }
                Some((new, None)) => format!("new {new:.2}"),
                None => "check failed".to_owned(),
            };
            eprintln!("[check] {text:?}: {scores} -> {outcome}");
        });
    }

    /// Has the agent act on text number `number`, which needs it. A correction of the
    /// run that was going when it came in restarts that run, if it still is. Otherwise
    /// a running agent finishes first, then runs again for it.
    /// Returns what it did, for the log.
    fn act_on(
        &mut self,
        number: u64,
        during: Option<TaskHandle>,
        correction: bool,
    ) -> &'static str {
        // A run that started after it came in was given it already.
        if number < *self.given.peek() {
            return "already given";
        }
        let wanted = (*self.wanted.peek()).max(number + 1);
        self.wanted.set(wanted);

        let running = *self.agent.peek();
        match running {
            None => {
                self.start_agent();
                "started"
            }
            Some(_) if correction && running == during => {
                self.start_agent();
                "restarted"
            }
            Some(_) => {
                *self.queued.write() += 1;
                "queued"
            }
        }
    }

    /// Cancels the running agent, if any, and starts one on the current transcript,
    /// scene and names.
    fn start_agent(&mut self) {
        if let Some(agent) = self.agent.write().take() {
            agent.cancel();
            if let Some(response) = self.responses.write().last_mut() {
                response.push_str(CANCELLED);
            }
        }
        let Some(model) = self.model.peek().clone() else {
            return;
        };

        let input = json!({
            "transcript": *self.transcript.peek(),
            "new_text": *self.new_text.peek(),
            "responses": *self.responses.peek(),
            "scene": *self.scene.peek(),
            "names": *self.names.peek(),
        });
        let input = serde_json::to_string_pretty(&input).expect("JSON values always serialize");

        let acting_on = self.new_text.peek().len();
        self.acting_on.set(acting_on);
        let received = *self.received.peek();
        self.given.set(received);
        self.queued.set(0);
        self.responses.write().push(String::new());
        self.writing.set(String::new());
        self.thinking.set(true);
        let mut live = *self;
        let agent = spawn(async move {
            let finished = live.run_agent(model, input).await;
            live.agent.set(None);
            live.writing.set(String::new());
            live.thinking.set(false);
            if finished {
                live.finish_run();
            }
        });
        self.agent.set(Some(agent));
    }

    /// Drops the text the finished run acted on, and runs again if any that came in
    /// while it was running needs it. The rest stays, as context for the next run.
    fn finish_run(&mut self) {
        let acting_on = *self.acting_on.peek();
        let mut new_text = self.new_text.write();
        new_text.replace_range(..acting_on, "");
        // The space it was joined to the next text with.
        if new_text.starts_with(' ') {
            new_text.remove(0);
        }
        drop(new_text);
        if *self.wanted.peek() > *self.given.peek() {
            self.start_agent();
        }
    }

    /// Streams the model's reply, running each command line as soon as it's complete.
    /// If the run is cancelled, a partial last line isn't run. False if it failed.
    async fn run_agent(&mut self, model: Model<Messages>, input: String) -> bool {
        let request = CompletionRequest::new(input).preamble(PREAMBLE);
        let mut stream = match model.stream(request) {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("[agent] {err}");
                return false;
            }
        };

        // A copy of the handle, so the line can be borrowed while `self` runs it.
        let mut writing = self.writing;
        while let Some(item) = stream.next().await {
            match item {
                Ok(Item::Event(StreamEvent::Text { text, .. })) => {
                    let mut line = writing.write();
                    line.push_str(&text);
                    while let Some(end) = line.find('\n') {
                        self.run_line(line[..end].trim_end());
                        line.replace_range(..=end, "");
                    }
                }
                Ok(_) => {}
                // The text stays new, so the next run gets it again.
                Err(err) => {
                    eprintln!("[agent] {err}");
                    return false;
                }
            }
        }
        let line = std::mem::take(&mut *writing.write());
        if !line.is_empty() {
            self.run_line(line.trim_end());
        }
        true
    }

    /// Adds one line of the agent's output to its response. If the line is a command
    /// (it starts with `#`), runs it and adds how it went after it.
    fn run_line(&mut self, line: &str) {
        let mut responses = self.responses.write();
        let response = responses.last_mut().expect("each run starts a response");
        response.push_str(line);
        response.push('\n');

        let Some(command) = line.trim_start().strip_prefix('#') else {
            return;
        };
        let outcome = match command.parse::<Command>() {
            Ok(command) => match command.apply(&mut self.scene.write(), &mut self.names.write()) {
                Ok(Some(id)) => format!("added {id}"),
                Ok(None) => "ok".into(),
                Err(err) => format!("failed: {err}"),
            },
            Err(err) => format!("failed: {err}"),
        };
        response.push_str(&format!("> {outcome}\n"));
    }
}

/// The agent's system prompt. Lines it writes that start with `#` are run as commands;
/// the rest is its reasoning.
const PREAMBLE: &str = r##"You listen to someone speaking, through speech recognition, and change what a display window shows to match what they ask for. The display holds a tree of elements: rectangles, which can hold other elements, and text.

## Input

Each message is a JSON object:

- `transcript`: everything said so far, as recognized. It ends with `new_text`.
- `new_text`: what was said since you last finished acting on the speech. Act on this; use the rest of the transcript only to understand it, such as what "it" or "that box" refers to.
- `responses`: your previous replies, oldest first, as you wrote them. After each command is a line starting with `>` that says how it went: `> added <id>`, `> ok`, or `> failed: <reason>`. A reply is cut off when new speech corrects what it was doing, and then ends with `> cancelled: new speech corrected this`, so it may be only part of what you meant to do. Carry out whatever is still missing, and don't repeat what already succeeded.
- `scene`: the elements as they are now. `elements` is the top level, in order. Each element has its `id`, and either `text` with its text, or `rect` with the elements inside it, in order. `next_id` is the id the next element you add will get.
- `names`: each name and the id it's on. The number words "zero" to "ten" start out on ids 0 to 10.

The scene and names already reflect everything done so far, so earlier requests in the transcript don't need doing again.

Speech recognition makes mistakes: words get misheard, split, merged or dropped. Go by what the person most plausibly meant. Not everything said is a request; if the new text asks for no change, do nothing.

## Output

Write plain lines. A line starting with `#` is a command, and it runs the moment the line ends, in order. Every other line is ignored, so you can think out loud between commands. Keep that to a few short lines, since the person is waiting to see the change. Never start a line of reasoning with `#`, don't write the `>` lines yourself, and don't use code blocks.

The commands:

# rect [in <id>] [as <name>]          Add an empty rectangle at the end of <id>'s children, or of the top level, and name it <name>.
# text [in <id>] [as <name>] <text>   Add text the same way. The text is the rest of the line.
# set <id> <text>                     Replace the text of a text element.
# remove <id>                         Remove an element and everything in it.
# name <id> <name>                    Put a name on an element. A name already in use moves to it.
# unname <name>                       Remove one name. The element keeps any others.
# unname all                          Remove every name except the number words.
# clear                               Remove every element, and every name except the number words.

- `<id>` is an element's number, or any of its names. Names ignore case.
- A name is a single word. It can't be a number, or `in`, `as`, `number` or `all`.
- New elements get ids counting up from `next_id`, in the order you add them, and ids are never reused. That lets you refer to something you added earlier in the same reply.
- Text you add can't start with the word "in" or "as", since that would be read as a parent or a name.
- A command that fails changes nothing. You'll see why in its `>` line in `responses` next time.

## Example

With `new_text` "add a header box with welcome in it":

A box named header, with the text inside.
# rect as header
# text in header Welcome
"##;
