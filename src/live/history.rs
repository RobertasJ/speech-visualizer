//! What the agent did in each run: the text it wrote, the scripts it ran and how they
//! went. Shown on screen, and given back to it as text on its next runs.

use std::fmt::Write;
use std::time::{Duration, Instant};

use super::usage::Usage;
use crate::scene::Outcome;

/// The agent's runs, oldest first.
#[derive(Debug, Default)]
pub struct History {
    runs: Vec<Run>,
}

/// One run of the agent.
#[derive(Debug)]
pub struct Run {
    /// The new text it was started for.
    pub new_text: String,
    pub entries: Vec<Entry>,
    started: Instant,
    /// How long it took to write or call anything.
    pub first_output: Option<Duration>,
    /// How long it took in all, once it's over.
    pub took: Option<Duration>,
    /// What its requests used, and cost.
    pub usage: Usage,
}

/// Something the agent did in a run.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// Text it wrote.
    Text(String),
    /// A script: the code written so far, and how it went once it ran.
    Script {
        code: String,
        outcome: Option<Outcome>,
    },
    /// New speech corrected what it was doing, so it was stopped.
    Cancelled,
    /// A request to the model failed, which ended the run.
    Failed(String),
}

impl History {
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// Starts a run, for `new_text`.
    pub fn begin(&mut self, new_text: String) {
        self.runs.push(Run {
            new_text,
            entries: Vec::new(),
            started: Instant::now(),
            first_output: None,
            took: None,
            usage: Usage::default(),
        });
    }

    /// The latest run is over.
    pub fn end(&mut self) {
        if let Some(run) = self.runs.last_mut() {
            run.took.get_or_insert_with(|| run.started.elapsed());
        }
    }

    /// The latest run was cancelled, which ends it.
    pub fn cancelled(&mut self) {
        self.push(Entry::Cancelled);
        self.end();
    }

    /// A request in the latest run finished, having used `usage`.
    pub fn used(&mut self, usage: Usage) {
        if let Some(run) = self.runs.last_mut() {
            run.usage += usage;
        }
    }

    /// What every run used, and cost.
    pub fn total(&self) -> Usage {
        let mut total = Usage::default();
        for run in &self.runs {
            total += run.usage;
        }
        total
    }

    /// A request in the latest run failed.
    pub fn failed(&mut self, error: String) {
        self.push(Entry::Failed(error));
    }

    /// The agent started a new part of its text.
    pub fn paragraph(&mut self) {
        self.output();
        self.push(Entry::Text(String::new()));
    }

    /// The agent wrote `fragment`.
    pub fn text(&mut self, fragment: &str) {
        self.output();
        match self.last_entry() {
            Some(Entry::Text(text)) => text.push_str(fragment),
            _ => self.push(Entry::Text(fragment.to_owned())),
        }
    }

    /// The agent started calling `run_script`.
    pub fn script(&mut self) {
        self.output();
        self.push(Entry::Script {
            code: String::new(),
            outcome: None,
        });
    }

    /// The code of the script being written, so far.
    pub fn code(&mut self, so_far: String) {
        if let Some(Entry::Script {
            code,
            outcome: None,
        }) = self.last_entry()
        {
            *code = so_far;
        }
    }

    /// The script being written was run, as `code`, with `outcome`.
    pub fn ran(&mut self, code: String, outcome: Outcome) {
        let ran = Entry::Script {
            code,
            outcome: Some(outcome),
        };
        match self.last_entry() {
            Some(entry @ Entry::Script { outcome: None, .. }) => *entry = ran,
            _ => self.push(ran),
        }
    }

    /// Each run as text, oldest first, for the agent to read back.
    pub fn rendered(&self) -> Vec<String> {
        self.runs.iter().map(Run::rendered).collect()
    }

    fn push(&mut self, entry: Entry) {
        if let Some(run) = self.runs.last_mut() {
            run.entries.push(entry);
        }
    }

    /// The latest run wrote or called something, which times its first output.
    fn output(&mut self) {
        if let Some(run) = self.runs.last_mut() {
            run.first_output
                .get_or_insert_with(|| run.started.elapsed());
        }
    }

    fn last_entry(&mut self) -> Option<&mut Entry> {
        self.runs.last_mut()?.entries.last_mut()
    }
}

impl Run {
    /// Whether it's still going.
    pub fn is_running(&self) -> bool {
        self.took.is_none()
    }

    /// The run as text: what it wrote, each script in a ```rhai block followed by `>`
    /// lines saying how it went, and how it ended if it didn't finish.
    pub fn rendered(&self) -> String {
        let mut text = String::new();
        for entry in &self.entries {
            match entry {
                Entry::Text(written) if written.trim().is_empty() => {}
                Entry::Text(written) => {
                    writeln!(text, "{}", written.trim()).expect("writing to a String");
                }
                Entry::Script { code, outcome } => {
                    writeln!(text, "```rhai\n{}\n```", code.trim()).expect("writing to a String");
                    let lines = match outcome {
                        Some(outcome) => outcome.lines(),
                        None => vec!["not run".to_owned()],
                    };
                    for line in lines {
                        writeln!(text, "> {line}").expect("writing to a String");
                    }
                }
                Entry::Cancelled => text.push_str("> cancelled: new speech corrected this\n"),
                Entry::Failed(error) => {
                    writeln!(text, "> failed: {error}").expect("writing to a String");
                }
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(output: &[&str], error: Option<&str>) -> Outcome {
        Outcome {
            output: output.iter().map(|&line| line.to_owned()).collect(),
            error: error.map(Into::into),
        }
    }

    #[test]
    fn text_fragments_join_until_a_new_paragraph() {
        let mut history = History::default();
        history.begin("add a box".into());
        history.paragraph();
        history.text("Adding ");
        history.text("a box.");
        history.paragraph();
        history.text("Done.");
        assert_eq!(
            history.runs()[0].entries,
            [
                Entry::Text("Adding a box.".into()),
                Entry::Text("Done.".into())
            ]
        );
    }

    #[test]
    fn a_script_streams_in_then_records_how_it_went() {
        let mut history = History::default();
        history.begin("add a box".into());
        history.script();
        history.code("let b = re".into());
        history.code("let b = rect();".into());
        history.ran("let b = rect();".into(), outcome(&[], None));
        assert_eq!(
            history.runs()[0].entries,
            [Entry::Script {
                code: "let b = rect();".into(),
                outcome: Some(outcome(&[], None)),
            }]
        );
        // Code that arrives after it ran doesn't change it.
        history.code("something else".into());
        assert!(
            matches!(&history.runs()[0].entries[0], Entry::Script { code, .. } if code == "let b = rect();")
        );
    }

    #[test]
    fn renders_runs_for_the_agent() {
        let mut history = History::default();
        history.begin("add a box".into());
        history.text("A box.");
        history.script();
        history.ran("let b = rect();".into(), outcome(&[], None));
        history.script();
        history.ran(
            "print(b); el(9)".into(),
            outcome(&["#1 rect"], Some("no element 9")),
        );
        history.script();
        history.code("b.desc = ".into());
        history.cancelled();

        history.begin("make it red".into());
        history.failed("overloaded".into());
        history.end();

        assert_eq!(
            history.rendered(),
            [
                "A box.\n\
                 ```rhai\nlet b = rect();\n```\n> ok\n\
                 ```rhai\nprint(b); el(9)\n```\n> #1 rect\n> error: no element 9\n\
                 ```rhai\nb.desc =\n```\n> not run\n\
                 > cancelled: new speech corrected this\n",
                "> failed: overloaded\n",
            ]
        );
    }

    #[test]
    fn usage_adds_up_per_run_and_in_total() {
        let used = |input, output| Usage {
            requests: 1,
            input,
            output,
            dollars: Some(0.5),
            ..Usage::default()
        };
        let mut history = History::default();
        history.begin("add a box".into());
        history.used(used(100, 10));
        history.used(used(200, 20));
        history.begin("make it red".into());
        history.used(used(50, 5));

        let first = history.runs()[0].usage;
        assert_eq!((first.requests, first.input, first.output), (2, 300, 30));
        assert_eq!(first.dollars, Some(1.));
        let total = history.total();
        assert_eq!((total.requests, total.input, total.output), (3, 350, 35));
        assert_eq!(total.dollars, Some(1.5));
    }

    #[test]
    fn timings() {
        let mut history = History::default();
        history.begin("add a box".into());
        history.failed("timeout".into());
        assert!(history.runs()[0].is_running());
        assert!(history.runs()[0].first_output.is_none());
        history.text("A box.");
        assert!(history.runs()[0].first_output.is_some());
        history.cancelled();
        let run = &history.runs()[0];
        assert!(!run.is_running());
        let took = run.took.unwrap();
        // Ending again keeps the first time.
        history.end();
        assert_eq!(history.runs()[0].took, Some(took));
    }
}
