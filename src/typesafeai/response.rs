use serde::{Deserialize, Deserializer, de};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Response {
    pub model: String,
    pub usage: Usage,
    answers: HashMap<String, Answer>,
}

impl Response {
    pub fn get_noul(&self, key: &str) -> f32 {
        match self.answers.get(key) {
            Some(Answer::Noul { noul }) => *noul,
            _ => panic!("No Noul answer found for key: {}", key),
        }
    }

    pub fn get_choice(&self, key: &str) -> &ChoiceAnswer {
        match self.answers.get(key) {
            Some(Answer::Choice(answer)) => answer,
            _ => panic!("No Choice answer found for key: {}", key),
        }
    }

    pub fn get_score(&self, key: &str) -> &ScoreAnswer {
        match self.answers.get(key) {
            Some(Answer::Score(answer)) => answer,
            _ => panic!("No Score answer found for key: {}", key),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    Noul { noul: f32 },
    Choice(ChoiceAnswer),
    Score(ScoreAnswer),
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChoiceAnswer {
    choice: String,
    probabilities: HashMap<String, f32>,
    confidence: f32,
}

impl ChoiceAnswer {
    pub fn choice(&self) -> &str {
        &self.choice
    }

    pub fn probabilities(&self) -> &HashMap<String, f32> {
        &self.probabilities
    }

    pub fn probability(&self, choice: &str) -> Option<f32> {
        self.probabilities.get(choice).copied()
    }

    pub fn confidence(&self) -> f32 {
        self.confidence
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScoreAnswer {
    score: f32,
    #[serde(rename = "legend", deserialize_with = "by_level")]
    legends: Vec<String>,
    #[serde(deserialize_with = "by_level")]
    probabilities: Vec<f32>,
    confidence: f32,
}

impl ScoreAnswer {
    pub fn score(&self) -> f32 {
        self.score
    }

    /// Indexed by level.
    pub fn legends(&self) -> &[String] {
        &self.legends
    }

    pub fn legend(&self, level: usize) -> Option<&str> {
        self.legends.get(level).map(String::as_str)
    }

    /// Indexed by level.
    pub fn probabilities(&self) -> &[f32] {
        &self.probabilities
    }

    pub fn probability(&self, level: usize) -> Option<f32> {
        self.probabilities.get(level).copied()
    }

    pub fn confidence(&self) -> f32 {
        self.confidence
    }
}

/// Reads a map keyed by level numbers as strings ("0", "1", ...) into its values,
/// indexed by level. The levels must run from 0 without gaps.
fn by_level<'de, D, V>(deserializer: D) -> Result<Vec<V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    let map = HashMap::<String, V>::deserialize(deserializer)?;
    let mut levels = map
        .into_iter()
        .map(|(key, value)| match key.parse::<usize>() {
            Ok(level) => Ok((level, value)),
            Err(_) => Err(de::Error::custom(format!("level `{key}` is not a number"))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    levels.sort_by_key(|&(level, _)| level);
    levels
        .into_iter()
        .enumerate()
        .map(|(i, (level, value))| {
            if level == i {
                Ok(value)
            } else {
                Err(de::Error::custom(format!("level {i} is missing")))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn response(answers: Value) -> Result<Response, serde_json::Error> {
        serde_json::from_value(json!({
            "model": "jev-latest",
            "usage": {"input_tokens": 10, "output_tokens": 2},
            "answers": answers,
        }))
    }

    #[test]
    fn parses_each_kind_of_answer() {
        let response = response(json!({
            "ready": {"type": "noul", "noul": 0.75},
            "route": {
                "type": "choice",
                "choice": "billing",
                "probabilities": {"billing": 0.75, "general": 0.25},
                "confidence": 0.5,
            },
            "urgency": {
                "type": "score",
                "score": 1.5,
                "legend": {"1": "very", "0": "not"},
                "probabilities": {"0": 0.25, "1": 0.75},
                "confidence": 0.5,
            },
        }))
        .unwrap();
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.get_noul("ready"), 0.75);

        let route = response.get_choice("route");
        assert_eq!(route.choice(), "billing");
        assert_eq!(route.probability("general"), Some(0.25));
        assert_eq!(route.probability("other"), None);

        // Levels come back in order, whatever order the map had them in.
        let urgency = response.get_score("urgency");
        assert_eq!(urgency.legends(), ["not", "very"]);
        assert_eq!(urgency.probabilities(), [0.25, 0.75]);
        assert_eq!(urgency.legend(1), Some("very"));
    }

    #[test]
    fn score_levels_must_be_numbers_from_zero_without_gaps() {
        let score = |legend: Value| {
            response(json!({"s": {
                "type": "score",
                "score": 0.0,
                "legend": legend,
                "probabilities": {"0": 1.0},
                "confidence": 1.0,
            }}))
            .unwrap_err()
            .to_string()
        };
        assert!(score(json!({"0": "a", "2": "c"})).contains("level 1 is missing"));
        assert!(score(json!({"x": "a"})).contains("level `x` is not a number"));
    }
}
