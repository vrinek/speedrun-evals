use serde::Deserialize;
use std::collections::HashSet;

/// A benchmark: a fixed list of tasks, each with one correct final answer.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub name: String,
    pub version: u32,
    /// System prompt sent with every task. Must tell the model to finish with `ANSWER: <x>`.
    pub system: String,
    /// Output token cap per task, so a looping model cannot run forever.
    pub max_tokens: u32,
    #[serde(rename = "task")]
    pub tasks: Vec<Task>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub prompt: String,
    pub answer: String,
}

impl Suite {
    pub fn parse(toml_text: &str) -> Result<Suite, String> {
        let suite: Suite = toml::from_str(toml_text).map_err(|e| e.to_string())?;
        if suite.tasks.is_empty() {
            return Err(format!("suite {} has no tasks", suite.name));
        }
        let mut seen = HashSet::new();
        for task in &suite.tasks {
            if !seen.insert(&task.id) {
                return Err(format!("duplicate task id: {}", task.id));
            }
        }
        Ok(suite)
    }
}

impl Task {
    /// True if the model's output ends with the expected answer.
    pub fn passed(&self, output: &str) -> bool {
        extract_answer(output).is_some_and(|got| normalise(got) == normalise(&self.answer))
    }
}

/// The text after the last `ANSWER:` marker, up to the end of that line.
pub fn extract_answer(output: &str) -> Option<&str> {
    let (_, after) = output.rsplit_once("ANSWER:")?;
    Some(after.lines().next().unwrap_or("").trim())
}

/// Models decorate answers (`**Paris**.`); compare without the decoration.
fn normalise(answer: &str) -> String {
    answer.trim_matches(|c: char| c.is_whitespace() || "*`\"'.".contains(c)).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUITE: &str = r#"
name = "demo"
version = 1
system = "End with ANSWER: <x>"
max_tokens = 512

[[task]]
id = "add"
prompt = "2+2?"
answer = "4"

[[task]]
id = "capital"
prompt = "Capital of France?"
answer = "Paris"
"#;

    #[test]
    fn parses_a_suite() {
        let suite = Suite::parse(SUITE).unwrap();
        assert_eq!(suite.name, "demo");
        assert_eq!(suite.max_tokens, 512);
        assert_eq!(suite.tasks.len(), 2);
        assert_eq!(suite.tasks[1].answer, "Paris");
    }

    #[test]
    fn rejects_duplicate_task_ids() {
        let dup = SUITE.replace("id = \"capital\"", "id = \"add\"");
        assert!(Suite::parse(&dup).unwrap_err().contains("add"));
    }

    #[test]
    fn rejects_an_empty_suite() {
        assert!(Suite::parse("name = \"x\"\nversion = 1\nsystem = \"s\"\nmax_tokens = 1\ntask = []").is_err());
    }

    #[test]
    fn extracts_the_last_answer_line() {
        assert_eq!(extract_answer("thinking\nANSWER: 4"), Some("4"));
        assert_eq!(extract_answer("ANSWER: 3\nno wait\nANSWER: 4\ntrailing"), Some("4"));
        assert_eq!(extract_answer("the answer is 4"), None);
    }

    #[test]
    fn passes_on_a_matching_answer_ignoring_case_and_decoration() {
        let task = &Suite::parse(SUITE).unwrap().tasks[1];
        assert!(task.passed("ANSWER: Paris"));
        assert!(task.passed("It is Paris.\nANSWER: **paris**."));
        assert!(task.passed("ANSWER: `Paris`  "));
    }

    #[test]
    fn fails_on_a_wrong_or_missing_answer() {
        let task = &Suite::parse(SUITE).unwrap().tasks[1];
        assert!(!task.passed("ANSWER: Lyon"));
        assert!(!task.passed("Paris"));
        assert!(!task.passed("ANSWER: Paris or Lyon"));
        assert!(!task.passed(""));
    }
}
