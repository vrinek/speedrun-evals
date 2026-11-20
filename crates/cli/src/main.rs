mod catalog;
mod fetch;
mod hardware;
mod runner;
mod server;

use clap::{Parser, Subcommand};
use runner::Completer;
use speedrun_core::{ModelRef, RunResult, Runtime, SCHEMA_VERSION};
use std::{
    env, fs,
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
        /// Run even though another program is using the GPU. The result will not
        /// be comparable, and the other program may slow down or fail.
        #[arg(long)]
        allow_busy_gpu: bool,
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
            allow_busy_gpu,
        } => benchmark_model(&model, &benchmark, out, allow_busy_gpu),
    }
}

fn benchmark_model(model_id: &str, benchmark: &str, out: PathBuf, allow_busy_gpu: bool) -> Result<(), String> {
    let model = catalog::models()
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or(format!("unknown model {model_id}, see `speedrun list`"))?;
    let suite = catalog::suites()
        .into_iter()
        .find(|s| s.name == benchmark)
        .ok_or(format!("unknown benchmark {benchmark}, see `speedrun list`"))?;
    let devices = server::list_devices()?;
    let busy = hardware::busy_gpus(&devices);
    if !busy.is_empty() && !allow_busy_gpu {
        return Err(format!(
            "another program is using the GPU ({}). A benchmark now would not be comparable \
             and may slow that program down or make it fail. Close it first, or pass --allow-busy-gpu.",
            busy.join("; ")
        ));
    }
    let hardware = hardware::detect(&devices)?;
    let runtime = Runtime {
        name: "llama.cpp".into(),
        version: server::version()?,
    };
    eprintln!("Hardware: {}", hardware.label());

    let path = fetch::ensure(&model, &cache_dir()?)?;
    eprintln!("Loading {} ...", model.id);
    let mut server = server::LlamaServer::start(&path)?;
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
        hardware,
        model: ModelRef {
            id: model.id,
            sha256: model.sha256,
        },
        runtime,
        benchmark: suite.name,
        benchmark_version: suite.version,
        tasks,
    };
    let dir = out.join(result.hardware.slug());
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(format!(
        "{}--{}--{started_at_unix}.json",
        result.model.id, result.benchmark
    ));
    fs::write(&file, serde_json::to_string_pretty(&result).unwrap() + "\n")
        .map_err(|e| format!("{}: {e}", file.display()))?;

    eprintln!(
        "Finished in {:.1}s, score {:.0}%, {} output tokens",
        result.wall_ms() as f64 / 1000.0,
        result.score() * 100.0,
        result.output_tokens()
    );
    println!("{}", file.display());
    Ok(())
}

fn cache_dir() -> Result<PathBuf, String> {
    let base = match env::var_os("XDG_CACHE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env::var_os("HOME").ok_or("neither XDG_CACHE_HOME nor HOME is set")?).join(".cache"),
    };
    Ok(base.join("speedrun"))
}
