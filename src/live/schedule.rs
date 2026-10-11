//! When the agent runs: one run at a time, for the segments that need it. A correction
//! restarts the run it corrects; anything else waits for the run to finish, which then
//! runs again.

use std::fmt;

/// What was done about a checked segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// It doesn't need the agent.
    Ignored,
    /// A run that started after it came in was given it already.
    AlreadyGiven,
    /// Nothing was running, so a run started for it.
    Started,
    /// It corrects the running agent, which was restarted.
    Restarted,
    /// The running agent finishes first, then runs again for it.
    Queued,
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Decision::Ignored => "ignored",
            Decision::AlreadyGiven => "already given",
            Decision::Started => "started",
            Decision::Restarted => "restarted",
            Decision::Queued => "queued",
        })
    }
}

/// Which segments runs were given, and which still need one.
#[derive(Debug, Default)]
pub struct Schedule {
    /// How many segments the current or latest run was given: those numbered below it.
    given: usize,
    /// One more than the number of the latest segment that needs the agent, or 0.
    wanted: usize,
    /// How many segments that need the agent came in while the current run was going.
    queued: usize,
}

impl Schedule {
    /// Decides what to do about segment `number`, now that it's checked. `needed` says
    /// whether it needs the agent, `restart` whether it corrects the run that was going
    /// when it came in, which still is, and `running` whether any run is going.
    pub fn decide(
        &mut self,
        number: usize,
        needed: bool,
        restart: bool,
        running: bool,
    ) -> Decision {
        if !needed {
            return Decision::Ignored;
        }
        if number < self.given {
            return Decision::AlreadyGiven;
        }
        self.wanted = self.wanted.max(number + 1);
        if !running {
            Decision::Started
        } else if restart {
            Decision::Restarted
        } else {
            self.queued += 1;
            Decision::Queued
        }
    }

    /// A run starts, given every segment numbered below `end`.
    pub fn start(&mut self, end: usize) {
        self.given = end;
        self.queued = 0;
    }

    /// How many segments the current or latest run was given: those numbered below it.
    pub fn given(&self) -> usize {
        self.given
    }

    /// Whether a segment the latest run wasn't given needs the agent, so it should run
    /// again when it finishes.
    pub fn wants_another(&self) -> bool {
        self.wanted > self.given
    }

    /// How many segments that need the agent came in while the current run was going.
    pub fn queued(&self) -> usize {
        self.queued
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_when_idle_and_ignores_what_needs_nothing() {
        let mut schedule = Schedule::default();
        assert_eq!(schedule.decide(0, false, false, false), Decision::Ignored);
        assert!(!schedule.wants_another());
        assert_eq!(schedule.decide(1, true, false, false), Decision::Started);
        schedule.start(2);
        assert!(!schedule.wants_another());
    }

    #[test]
    fn queues_during_a_run_then_runs_again() {
        let mut schedule = Schedule::default();
        schedule.decide(0, true, false, false);
        schedule.start(1);
        assert_eq!(schedule.decide(1, true, false, true), Decision::Queued);
        assert_eq!(schedule.decide(2, true, false, true), Decision::Queued);
        assert_eq!(schedule.queued(), 2);
        assert!(schedule.wants_another());

        schedule.start(3);
        assert_eq!(schedule.queued(), 0);
        assert!(!schedule.wants_another());
    }

    #[test]
    fn a_run_that_started_later_already_has_it() {
        let mut schedule = Schedule::default();
        schedule.start(3);
        assert_eq!(
            schedule.decide(1, true, false, true),
            Decision::AlreadyGiven
        );
        assert_eq!(schedule.decide(1, true, true, true), Decision::AlreadyGiven);
        assert!(!schedule.wants_another());
    }

    #[test]
    fn restarts_only_for_a_correction_of_the_running_agent() {
        let mut schedule = Schedule::default();
        schedule.start(1);
        assert_eq!(schedule.decide(1, true, true, true), Decision::Restarted);
        assert_eq!(schedule.queued(), 0);
        // The correction came in during a run that has since ended.
        assert_eq!(schedule.decide(2, true, false, false), Decision::Started);
    }
}
