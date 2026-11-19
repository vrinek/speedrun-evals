use serde::{Deserialize, Serialize};

/// Bump when the result file format changes incompatibly.
pub const SCHEMA_VERSION: u32 = 1;

/// One benchmark run of one model on one machine. This is the file format
/// contributors submit under `results/`, so unknown fields are rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunResult {
    pub schema_version: u32,
    pub cli_version: String,
    pub started_at_unix: u64,
    pub hardware: Hardware,
    pub model: ModelRef,
    pub runtime: Runtime,
    pub benchmark: String,
    pub benchmark_version: u32,
    pub tasks: Vec<TaskResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hardware {
    pub cpu: String,
    pub ram_gb: u32,
    pub gpus: Vec<Gpu>,
    pub os: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gpu {
    pub name: String,
    pub vram_gb: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRef {
    pub id: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskResult {
    pub id: String,
    pub passed: bool,
    pub wall_ms: u64,
    pub output_tokens: u64,
}

impl RunResult {
    pub fn from_json(text: &str) -> Result<RunResult, String> {
        let run: RunResult = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if run.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema_version {}, expected {SCHEMA_VERSION}",
                run.schema_version
            ));
        }
        if run.tasks.is_empty() {
            return Err("run has no tasks".into());
        }
        Ok(run)
    }

    /// Fraction of tasks passed, 0.0 to 1.0.
    pub fn score(&self) -> f64 {
        self.tasks.iter().filter(|t| t.passed).count() as f64 / self.tasks.len() as f64
    }

    /// Wall-clock time to finish the whole benchmark: the headline number.
    pub fn wall_ms(&self) -> u64 {
        self.tasks.iter().map(|t| t.wall_ms).sum()
    }

    pub fn output_tokens(&self) -> u64 {
        self.tasks.iter().map(|t| t.output_tokens).sum()
    }
}

impl Hardware {
    fn parts(&self) -> Vec<String> {
        let mut parts = vec![self.cpu.clone()];
        for gpu in &self.gpus {
            parts.push(match gpu.vram_gb {
                Some(gb) => format!("{} {gb}GB", gpu.name),
                None => gpu.name.clone(),
            });
        }
        parts.push(format!("{}GB RAM", self.ram_gb));
        parts
    }

    /// Human-readable one-liner, e.g. `Ryzen 9 7950X · RTX 4090 24GB · 64GB RAM`.
    pub fn label(&self) -> String {
        self.parts().join(" · ")
    }

    /// Stable identifier used for directory names and URLs.
    pub fn slug(&self) -> String {
        self.parts().iter().map(|p| slug(p)).collect::<Vec<_>>().join("--")
    }
}

/// Lowercase ASCII letters and digits, everything else collapsed to single hyphens.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RunResult {
        RunResult {
            schema_version: SCHEMA_VERSION,
            cli_version: "0.1.0".into(),
            started_at_unix: 1_795_000_000,
            hardware: Hardware {
                cpu: "AMD Ryzen 9 7950X".into(),
                ram_gb: 64,
                gpus: vec![Gpu {
                    name: "NVIDIA GeForce RTX 4090".into(),
                    vram_gb: Some(24),
                }],
                os: "Linux 7.2".into(),
            },
            model: ModelRef {
                id: "tiny-q4".into(),
                sha256: "ab".repeat(32),
            },
            runtime: Runtime {
                name: "llama.cpp".into(),
                version: "b1234".into(),
            },
            benchmark: "demo".into(),
            benchmark_version: 1,
            tasks: vec![
                TaskResult {
                    id: "a".into(),
                    passed: true,
                    wall_ms: 1500,
                    output_tokens: 100,
                },
                TaskResult {
                    id: "b".into(),
                    passed: false,
                    wall_ms: 500,
                    output_tokens: 40,
                },
                TaskResult {
                    id: "c".into(),
                    passed: true,
                    wall_ms: 1000,
                    output_tokens: 60,
                },
                TaskResult {
                    id: "d".into(),
                    passed: true,
                    wall_ms: 1000,
                    output_tokens: 50,
                },
            ],
        }
    }

    #[test]
    fn round_trips_through_json() {
        let run = sample();
        let json = serde_json::to_string_pretty(&run).unwrap();
        assert_eq!(RunResult::from_json(&json).unwrap(), run);
    }

    #[test]
    fn totals_wall_clock_score_and_tokens() {
        let run = sample();
        assert_eq!(run.wall_ms(), 4000);
        assert_eq!(run.score(), 0.75);
        assert_eq!(run.output_tokens(), 250);
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = serde_json::to_string(&sample())
            .unwrap()
            .replacen('{', "{\"extra\":1,", 1);
        assert!(RunResult::from_json(&json).is_err());
    }

    #[test]
    fn rejects_other_schema_versions() {
        let mut run = sample();
        run.schema_version = SCHEMA_VERSION + 1;
        let err = RunResult::from_json(&serde_json::to_string(&run).unwrap()).unwrap_err();
        assert!(err.contains("schema_version"), "{err}");
    }

    #[test]
    fn rejects_a_run_with_no_tasks() {
        let mut run = sample();
        run.tasks.clear();
        assert!(RunResult::from_json(&serde_json::to_string(&run).unwrap()).is_err());
    }

    #[test]
    fn slugs_are_url_and_path_safe() {
        assert_eq!(slug("AMD Ryzen™ 9 7950X (16-Core)"), "amd-ryzen-9-7950x-16-core");
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
        assert_eq!(slug("  "), "");
    }

    #[test]
    fn hardware_label_and_slug() {
        let hw = sample().hardware;
        assert_eq!(
            hw.label(),
            "AMD Ryzen 9 7950X · NVIDIA GeForce RTX 4090 24GB · 64GB RAM"
        );
        assert_eq!(hw.slug(), "amd-ryzen-9-7950x--nvidia-geforce-rtx-4090-24gb--64gb-ram");
    }

    #[test]
    fn hardware_without_gpu_or_vram() {
        let mut hw = sample().hardware;
        hw.gpus[0].vram_gb = None;
        assert_eq!(hw.label(), "AMD Ryzen 9 7950X · NVIDIA GeForce RTX 4090 · 64GB RAM");
        hw.gpus.clear();
        assert_eq!(hw.label(), "AMD Ryzen 9 7950X · 64GB RAM");
        assert_eq!(hw.slug(), "amd-ryzen-9-7950x--64gb-ram");
    }
}
