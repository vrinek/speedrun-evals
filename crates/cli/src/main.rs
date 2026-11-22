mod catalog;
mod fetch;
mod hardware;
mod runner;
mod server;

use clap::{Parser, Subcommand};
use runner::Completer;
use speedrun_core::{ModelRef, RunResult, Runtime, SCHEMA_VERSION};
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

/// Benchmark local LLMs: how long a model takes to finish a benchmark on this
/// machine, and how well it does.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download a model, run a benchmark against it and write a result file
    Run {
        /// Model id, see `speedrun list`
        model: String,
        #[arg(long, default_value = "quick")]
        benchmark: String,
        /// Directory to write the result file into
        #[arg(long, default_value = "results")]
        out: PathBuf,
        /// How many times to run the benchmark. Each run restarts the runtime
        /// and writes its own result file.
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..))]
        runs: u32,
        /// The program that runs the model. `gufo` must be on PATH.
        #[arg(long, value_enum, default_value = "llama.cpp")]
        runtime: server::Engine,
    },
    /// List the available models and benchmarks
    List,
    /// Print the hardware that would be recorded in a result
    Hardware,
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Cmd) -> Result<(), String> {
    match command {
        Cmd::List => {
            println!("Models:");
            catalog::models().iter().for_each(|m| println!("  {}", m.id));
            println!("Benchmarks:");
            catalog::suites()
                .iter()
                .for_each(|s| println!("  {} (v{}, {} tasks)", s.name, s.version, s.tasks.len()));
            Ok(())
        }
        Cmd::Hardware => {
            let hardware = hardware::detect(&server::list_devices()?)?;
            println!("{}", hardware.label());
            println!("{}", serde_json::to_string_pretty(&hardware).unwrap());
            Ok(())
        }
        Cmd::Run {
            model,
            benchmark,
            out,
            runs,
            runtime,
        } => benchmark_model(&model, &benchmark, out, runs, runtime),
    }
}

fn benchmark_model(
    model_id: &str,
    benchmark: &str,
    out: PathBuf,
    runs: u32,
    engine: server::Engine,
) -> Result<(), String> {
    let model = catalog::models()
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or(format!("unknown model {model_id}, see `speedrun list`"))?;
    let suite = catalog::suites()
        .into_iter()
        .find(|s| s.name == benchmark)
        .ok_or(format!("unknown benchmark {benchmark}, see `speedrun list`"))?;
    let devices = server::list_devices()?;
    let hardware = hardware::detect(&devices)?;
    let runtime = Runtime {
        name: engine.name().into(),
        version: engine.version()?,
    };
    eprintln!("Hardware: {}", hardware.label());

    let path = fetch::ensure(&model, &cache_dir()?)?;
    let model_bytes = fs::metadata(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    if let Some(why) = hardware::gpu_shortfall(&devices, model_bytes) {
        return Err(why);
    }
    for run in 1..=runs {
        eprintln!("Run {run} of {runs}: loading {} ...", model.id);
        // A fresh runtime per run, so no run benefits from another's cached prompts.
        let mut server = server::Server::start(engine, &path)?;
        // One untimed request so one-off start-up costs do not land on the first task.
        server.complete("Hi", 1)?;

        let started_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let tasks = runner::run_suite(&suite, &mut server, |task| {
            let mark = if task.passed { "pass" } else { "FAIL" };
            eprintln!(
                "  {mark}  {:>7.1}s  {:>6} tok  {}",
                task.wall_ms as f64 / 1000.0,
                task.output_tokens,
                task.id
            );
        })?;
        drop(server);

        let result = RunResult {
            schema_version: SCHEMA_VERSION,
            cli_version: env!("CARGO_PKG_VERSION").into(),
            started_at_unix,
            hardware: hardware.clone(),
            model: ModelRef {
                id: model.id.clone(),
                sha256: model.sha256.clone(),
            },
            runtime: runtime.clone(),
            benchmark: suite.name.clone(),
            benchmark_version: suite.version,
            tasks,
        };
        let dir = out.join(result.hardware.slug());
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = dir.join(format!(
            "{}--{}--{started_at_unix}.json",
            result.model.id, result.benchmark
        ));
        // create_new: two runs in the same second must not overwrite each other.
        File::create_new(&file)
            .and_then(|mut f| f.write_all((serde_json::to_string_pretty(&result).unwrap() + "\n").as_bytes()))
            .map_err(|e| format!("{}: {e}", file.display()))?;

        eprintln!(
            "Finished in {:.1}s, score {:.0}%, {} output tokens",
            result.wall_ms() as f64 / 1000.0,
            result.score() * 100.0,
            result.output_tokens()
        );
        println!("{}", file.display());
    }
    Ok(())
}

fn cache_dir() -> Result<PathBuf, String> {
    let base = match env::var_os("XDG_CACHE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env::var_os("HOME").ok_or("neither XDG_CACHE_HOME nor HOME is set")?).join(".cache"),
    };
    Ok(base.join("speedrun"))
}
