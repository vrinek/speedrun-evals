//! Turns a pile of result files into leaderboards: one per hardware, and
//! within it one list of runs per benchmark.

use speedrun_core::{Hardware, RunResult};
use std::collections::BTreeMap;

/// One run of one model on one benchmark on one hardware. Runs are never
/// averaged or merged: every submitted result is shown as it was measured.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub model: String,
    pub score: f64,
    pub wall_ms: u64,
    pub output_tokens: u64,
    pub started_at_unix: u64,
}

pub struct Board {
    pub hardware: Hardware,
    /// Benchmark title (`name vN`) to entries, best first.
    pub benchmarks: BTreeMap<String, Vec<Entry>>,
}

/// Groups runs by hardware slug, then benchmark.
pub fn boards(runs: Vec<RunResult>) -> BTreeMap<String, Board> {
    let mut boards: BTreeMap<String, Board> = BTreeMap::new();
    for run in runs {
        let board = boards.entry(run.hardware.slug()).or_insert_with(|| Board {
            hardware: run.hardware.clone(),
            benchmarks: BTreeMap::new(),
        });
        let title = format!("{} v{}", run.benchmark, run.benchmark_version);
        board.benchmarks.entry(title).or_default().push(Entry {
            score: run.score(),
            wall_ms: run.wall_ms(),
            output_tokens: run.output_tokens(),
            started_at_unix: run.started_at_unix,
            // ponytail: llama.cpp is the default and stays unnamed; any other runtime is
            // part of the name, so its runs are their own dots. A runtime column or
            // filter once there are more than two.
            model: match run.runtime.name.as_str() {
                "llama.cpp" => run.model.id,
                other => format!("{} ({other})", run.model.id),
            },
        });
    }
    for entries in boards.values_mut().flat_map(|board| board.benchmarks.values_mut()) {
        entries.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.wall_ms.cmp(&b.wall_ms)));
    }
    boards
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use speedrun_core::{Gpu, ModelRef, Runtime, SCHEMA_VERSION, TaskResult};

    pub fn run(cpu: &str, model: &str, wall_ms: u64, passed: bool) -> RunResult {
        RunResult {
            schema_version: SCHEMA_VERSION,
            cli_version: "0.1.0".into(),
            started_at_unix: 1,
            hardware: Hardware {
                cpu: cpu.into(),
                ram_gb: 64,
                gpus: vec![Gpu {
                    name: "GPU".into(),
                    vram_gb: Some(24),
                }],
                os: "Linux".into(),
            },
            model: ModelRef {
                id: model.into(),
                sha256: "00".repeat(32),
            },
            runtime: Runtime {
                name: "llama.cpp".into(),
                version: "1".into(),
            },
            benchmark: "quick".into(),
            benchmark_version: 1,
            tasks: vec![
                TaskResult {
                    id: "a".into(),
                    passed: true,
                    wall_ms,
                    output_tokens: 10,
                },
                TaskResult {
                    id: "b".into(),
                    passed,
                    wall_ms: 0,
                    output_tokens: 5,
                },
            ],
        }
    }

    #[test]
    fn groups_by_hardware_then_benchmark() {
        let boards = boards(vec![
            run("CPU A", "m1", 100, true),
            run("CPU B", "m1", 100, true),
            run("CPU A", "m2", 50, true),
        ]);
        assert_eq!(
            boards.keys().collect::<Vec<_>>(),
            ["cpu-a--gpu-24gb--64gb-ram", "cpu-b--gpu-24gb--64gb-ram"]
        );
        let a = &boards["cpu-a--gpu-24gb--64gb-ram"];
        assert_eq!(a.hardware.cpu, "CPU A");
        assert_eq!(a.benchmarks.keys().collect::<Vec<_>>(), ["quick v1"]);
        assert_eq!(a.benchmarks["quick v1"].len(), 2);
    }

    #[test]
    fn every_run_is_its_own_entry() {
        let boards = boards(vec![
            run("CPU", "m", 900, true),
            run("CPU", "m", 100, false),
            run("CPU", "m", 300, true),
        ]);
        let entries = &boards.values().next().unwrap().benchmarks["quick v1"];
        let seen: Vec<_> = entries.iter().map(|e| (e.model.as_str(), e.score, e.wall_ms)).collect();
        assert_eq!(seen, [("m", 1.0, 300), ("m", 1.0, 900), ("m", 0.5, 100)]);
    }

    #[test]
    fn a_runtime_other_than_llama_cpp_is_named_beside_the_model() {
        let mut gufo = run("CPU", "m", 100, true);
        gufo.runtime.name = "gufo".into();
        let boards = boards(vec![run("CPU", "m", 200, true), gufo]);
        let names: Vec<_> = boards.values().next().unwrap().benchmarks["quick v1"]
            .iter()
            .map(|e| e.model.as_str())
            .collect();
        assert_eq!(names, ["m (gufo)", "m"]);
    }

    #[test]
    fn best_score_first_then_fastest() {
        let boards = boards(vec![
            run("CPU", "slow-good", 900, true),
            run("CPU", "fast-bad", 100, false),
            run("CPU", "fast-good", 200, true),
        ]);
        let order: Vec<_> = boards.values().next().unwrap().benchmarks["quick v1"]
            .iter()
            .map(|e| e.model.as_str())
            .collect();
        assert_eq!(order, ["fast-good", "slow-good", "fast-bad"]);
    }
}
