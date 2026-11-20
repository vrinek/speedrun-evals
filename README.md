# Speedrun Evals

How long does a local LLM take to finish a benchmark on your hardware, and how
well does it do?

Tokens per second does not answer that. Two models at the same speed and the
same score can differ tenfold in how much they write to get there. Speedrun
Evals measures the thing you actually wait for: wall-clock time to finish the
whole benchmark, next to the score.

There are two parts:

- `speedrun`, a command-line tool that downloads a model, runs a benchmark
  against it on your machine and writes a result file.
- A website built from everyone's result files: pick a machine, see every model
  as a dot on a chart of score against time.

## Run a benchmark

You need Linux and [Nix](https://nixos.org/download/). Nothing else: the tool
brings its own pinned inference runtime (llama.cpp with Vulkan) and GPU drivers,
so everybody measures with the same software.

```sh
nix --extra-experimental-features 'nix-command flakes' run github:vrinek/speedrun-evals -- list
nix --extra-experimental-features 'nix-command flakes' run github:vrinek/speedrun-evals -- run qwen3-4b-q4_k_m
```

`run` detects your hardware, downloads the model (checked against a pinned
SHA-256), runs the benchmark and writes
`results/<your-hardware>/<model>--<benchmark>--<time>.json`.

For numbers worth comparing, close other heavy programs first. The tool refuses
to start when another program already holds more than a quarter of a GPU's
memory; `--allow-busy-gpu` overrides that, at your own risk and the other
program's.

### Known limits

- Linux only.
- GPUs work through Mesa's Vulkan drivers: AMD and Intel. The proprietary
  NVIDIA driver is not supported yet; without a usable GPU the run uses the CPU
  and the result says so.
- The only benchmark so far, `quick`, is a small sanity check that strong
  models all pass. Harder benchmarks are the most useful thing to contribute.

## Submit your result

Fork this repository, add the result file under `results/` exactly as the tool
wrote it, and open a pull request. CI rejects files that do not parse.

Results are self-reported: nothing proves a file came from an unmodified run.
Reviewers look for numbers that are out of line with similar hardware.

## Add a model or a benchmark

- Model: add an entry to [`models.toml`](models.toml). It must pin a commit and
  a SHA-256.
- Benchmark: add a file under [`benchmarks/`](benchmarks) in the format of
  [`quick.toml`](benchmarks/quick.toml), and list it in
  [`crates/cli/src/catalog.rs`](crates/cli/src/catalog.rs). Every task has one
  exact answer. Changing an existing benchmark's tasks requires raising its
  `version`, so old and new results are never mixed.

## Develop

```sh
cargo test                                    # everything
cargo run -p speedrun-site -- results dist    # build the website into dist/
nix build                                     # the packaged CLI, as users get it
```

| Path | What it is |
|---|---|
| `crates/core` | Benchmark format, result file format, scoring |
| `crates/cli` | The `speedrun` tool |
| `crates/site` | Static site generator: `results/` in, HTML out, no JavaScript |
| `benchmarks/`, `models.toml` | What can be run; compiled into the CLI |
| `results/` | Submitted results; the website's only data source |

## Licence

AGPL-3.0-or-later. See [LICENSE](LICENSE).
