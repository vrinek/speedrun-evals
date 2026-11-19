//! Runs a suite against a model and times every task.

use speedrun_core::{Suite, TaskResult};
use std::time::Instant;

pub struct Completion {
    pub text: String,
    pub output_tokens: u64,
}

/// Anything that can answer a prompt: the real inference server, or a fake in tests.
pub trait Completer {
    fn complete(&mut self, system: &str, prompt: &str, max_tokens: u32) -> Result<Completion, String>;
}

/// Runs every task in order. A failed request aborts the run: a broken server
/// must not be recorded as the model getting the answer wrong.
pub fn run_suite(
    suite: &Suite,
    model: &mut dyn Completer,
    mut on_task: impl FnMut(&TaskResult),
) -> Result<Vec<TaskResult>, String> {
    let mut results = Vec::new();
    for task in &suite.tasks {
        let start = Instant::now();
        let completion = model
            .complete(&suite.system, &task.prompt, suite.max_tokens)
            .map_err(|e| format!("task {}: {e}", task.id))?;
        let result = TaskResult {
            id: task.id.clone(),
            passed: task.passed(&completion.text),
            wall_ms: start.elapsed().as_millis() as u64,
            output_tokens: completion.output_tokens,
        };
        on_task(&result);
        results.push(result);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread::sleep, time::Duration};

    struct Fake(Vec<Result<&'static str, &'static str>>);

    impl Completer for Fake {
        fn complete(&mut self, system: &str, _prompt: &str, max_tokens: u32) -> Result<Completion, String> {
            assert_eq!(system, "sys");
            assert_eq!(max_tokens, 99);
            sleep(Duration::from_millis(15));
            let text = self.0.remove(0)?;
            Ok(Completion { text: text.into(), output_tokens: 7 })
        }
    }

    fn suite() -> Suite {
        Suite::parse(
            "name='s'\nversion=1\nsystem='sys'\nmax_tokens=99\n\
             [[task]]\nid='a'\nprompt='p'\nanswer='1'\n[[task]]\nid='b'\nprompt='q'\nanswer='2'\n",
        )
        .unwrap()
    }

    #[test]
    fn scores_and_times_each_task() {
        let mut seen = 0;
        let results = run_suite(&suite(), &mut Fake(vec![Ok("ANSWER: 1"), Ok("ANSWER: 9")]), |_| seen += 1).unwrap();
        assert_eq!(seen, 2);
        assert_eq!(results.iter().map(|r| (r.id.as_str(), r.passed)).collect::<Vec<_>>(), [("a", true), ("b", false)]);
        assert!(results.iter().all(|r| r.wall_ms >= 15 && r.output_tokens == 7));
    }

    #[test]
    fn a_failed_request_aborts_the_run() {
        let err = run_suite(&suite(), &mut Fake(vec![Ok("ANSWER: 1"), Err("boom")]), |_| {}).unwrap_err();
        assert!(err.contains("boom") && err.contains("b"), "{err}");
    }
}
