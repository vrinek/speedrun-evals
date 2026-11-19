//! Turns a pile of result files into leaderboards: one per hardware, and
//! within it one list of models per benchmark.

use speedrun_core::{Hardware, RunResult};
use std::collections::BTreeMap;

/// One model's standing on one benchmark on one hardware.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub model: String,
    pub score: f64,
    pub wall_ms: u64,
    pub output_tokens: u64,
    /// How many submitted runs this entry summarises.
    pub runs: usize,
}

pub struct Board {
    pub hardware: Hardware,
    /// Benchmark title (`name vN`) to entries, best first.
    pub benchmarks: BTreeMap<String, Vec<Entry>>,
}

type ByBenchmarkAndModel = BTreeMap<String, BTreeMap<String, Vec<RunResult>>>;

/// Groups runs by hardware slug. Several runs of the same model are summarised
/// by the run with the median wall-clock time, so one lucky or unlucky run does not decide.
pub fn boards(runs: Vec<RunResult>) -> BTreeMap<String, Board> {
    // hardware slug -> benchmark title -> model id -> runs
    let mut grouped: BTreeMap<String, (Hardware, ByBenchmarkAndModel)> = BTreeMap::new();
    for run in runs {
        let (_, benchmarks) = grouped
            .entry(run.hardware.slug())
            .or_insert_with(|| (run.hardware.clone(), BTreeMap::new()));
        let title = format!("{} v{}", run.benchmark, run.benchmark_version);
        benchmarks
            .entry(title)
            .or_default()
            .entry(run.model.id.clone())
            .or_default()
            .push(run);
    }
    grouped
        .into_iter()
        .map(|(slug, (hardware, benchmarks))| {
            let benchmarks = benchmarks
                .into_iter()
                .map(|(title, models)| {
                    let mut entries: Vec<Entry> = models.into_values().map(summarise).collect();
                    entries.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.wall_ms.cmp(&b.wall_ms)));
                    (title, entries)
                })
                .collect();
            (slug, Board { hardware, benchmarks })
        })
        .collect()
}

fn summarise(mut runs: Vec<RunResult>) -> Entry {
    runs.sort_by_key(RunResult::wall_ms);
    let median = &runs[(runs.len() - 1) / 2];
    Entry {
        model: median.model.id.clone(),
        score: median.score(),
        wall_ms: median.wall_ms(),
        output_tokens: median.output_tokens(),
        runs: runs.len(),
    }
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
    fn repeated_runs_collapse_to_the_median_wall_clock() {
        let boards = boards(vec![
            run("CPU", "m", 900, true),
            run("CPU", "m", 100, false),
            run("CPU", "m", 300, true),
        ]);
        let entries = &boards.values().next().unwrap().benchmarks["quick v1"];
        assert_eq!(
            entries,
            &[Entry {
                model: "m".into(),
                score: 1.0,
                wall_ms: 300,
                output_tokens: 15,
                runs: 3
            }]
        );
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
