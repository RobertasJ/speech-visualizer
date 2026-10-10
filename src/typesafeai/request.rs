use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct Request {
    state: Value,
    model: Model,
    questions: HashMap<String, Question>,
}

impl Request {
    pub fn new(state: impl Serialize) -> Self {
        Request {
            state: serde_json::to_value(state).expect("Failed to serialize state"),
            model: Model::default(),
            questions: HashMap::new(),
        }
    }

    pub fn with_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }

    pub fn with_question(mut self, name: impl Into<String>, question: impl Into<Question>) -> Self {
        self.questions.insert(name.into(), question.into());
        self
    }
}

#[derive(Default)]
pub enum Model {
    #[default]
    Latest,
}

impl Serialize for Model {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Model::Latest => serializer.serialize_str("jev-latest"),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    Noul(Noul),
    Choice(Choice),
    Score(Score),
}

#[derive(Serialize)]
pub struct Noul {
    instructions: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    criteria: Option<NoulCriteria>,
}

impl Noul {
    pub fn new(instructions: impl Serialize) -> Self {
        Noul {
            instructions: serde_json::to_value(instructions)
                .expect("Failed to serialize instructions"),
            criteria: None,
        }
    }

    pub fn with_criteria(
        mut self,
        true_criteria: impl Serialize,
        false_criteria: impl Serialize,
    ) -> Self {
        self.criteria = Some(NoulCriteria {
            r#true: serde_json::to_value(true_criteria).expect("Failed to serialize true criteria"),
            r#false: serde_json::to_value(false_criteria)
                .expect("Failed to serialize false criteria"),
        });
        self
    }
}

impl From<Noul> for Question {
    fn from(noul: Noul) -> Self {
        Question::Noul(noul)
    }
}

#[derive(Serialize)]
struct NoulCriteria {
    r#true: Value,
    r#false: Value,
}

#[derive(Serialize)]
pub struct Choice {
    instructions: Value,
    criteria: HashMap<String, Value>,
}

impl Choice {
    pub fn new(instructions: impl Serialize) -> Self {
        Choice {
            instructions: serde_json::to_value(instructions)
                .expect("Failed to serialize instructions"),
            criteria: HashMap::new(),
        }
    }

    pub fn with_option(
        mut self,
        option: impl Into<String>,
        option_criteria: impl Serialize,
    ) -> Self {
        self.criteria.insert(
            option.into(),
            serde_json::to_value(option_criteria).expect("Failed to serialize criteria value"),
        );
        self
    }
}

impl From<Choice> for Question {
    fn from(choice: Choice) -> Self {
        Question::Choice(choice)
    }
}

#[derive(Serialize)]
pub struct Score {
    instructions: Value,
    criteria: Vec<Value>,
}

impl Score {
    pub fn new(instructions: impl Serialize) -> Self {
        Score {
            instructions: serde_json::to_value(instructions)
                .expect("Failed to serialize instructions"),
            criteria: Vec::new(),
        }
    }

    pub fn with_level(mut self, level_criteria: impl Serialize) -> Self {
        self.criteria.push(
            serde_json::to_value(level_criteria).expect("Failed to serialize criteria value"),
        );
        self
    }
}

impl From<Score> for Question {
    fn from(score: Score) -> Self {
        Question::Score(score)
    }
}
