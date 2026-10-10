//! Names for scene elements, so a command can say "box" where it would say an id.

use std::collections::HashMap;

use super::ElementId;

/// Lowercase names and the elements they're on. Starts out as [`number_names`], so a
/// name can be on an element that doesn't exist yet. Kept apart from the scene, so it
/// isn't told about removed elements: [`forget`](Self::forget) their ids.
#[derive(Debug, Clone, PartialEq)]
pub struct Names(HashMap<String, ElementId>);

impl Default for Names {
    fn default() -> Self {
        Self(number_names())
    }
}

/// Ids up to ten by their word, since speech often spells them out: "three" is element
/// 3. Names start with these, and get them back on [`reset`](Names::reset).
fn number_names() -> HashMap<String, ElementId> {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    WORDS
        .iter()
        .zip(0..)
        .map(|(&word, id)| (word.to_owned(), ElementId(id)))
        .collect()
}

impl Names {
    /// The element `name` is on, ignoring case, whether or not that element exists.
    pub fn get(&self, name: &str) -> Option<ElementId> {
        self.0.get(&name.to_lowercase()).copied()
    }

    /// Puts `name` on `id`. A name that's in use already moves to it.
    pub fn insert(&mut self, name: &str, id: ElementId) {
        self.0.insert(name.to_lowercase(), id);
    }

    /// Removes one name, ignoring case; false if there was no such name.
    pub fn remove(&mut self, name: &str) -> bool {
        self.0.remove(&name.to_lowercase()).is_some()
    }

    /// Removes every name on `ids`, number words included.
    pub fn forget(&mut self, ids: &[ElementId]) {
        self.0.retain(|_, named| !ids.contains(named));
    }

    /// Removes every name but the number words.
    pub fn reset(&mut self) {
        self.0 = number_names();
    }

    /// The names on `id`, sorted.
    pub fn of(&self, id: ElementId) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .0
            .iter()
            .filter(|&(_, &named)| named == id)
            .map(|(name, _)| name.as_str())
            .collect();
        names.sort_unstable();
        names
    }
}
