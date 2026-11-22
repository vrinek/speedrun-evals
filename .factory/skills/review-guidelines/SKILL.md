---
name: review-guidelines
description: What to check when reviewing a pull request to Speedrun Evals.
---

# Review guidelines

AGENTS.md has the project's rules. Review against it, and check these first.

## Result files (`results/`)

- A result is one machine's measurement. It must come from the `speedrun` CLI, never be written or edited by hand.
  Flag any change to an existing result file, and any new file whose name does not match
  `results/<hardware-slug>/<model>--<benchmark>--<unix>.json`.
- A pull request that adds results should add nothing else.
- Do not judge whether a score or time is plausible. Say so if the model or benchmark is not in
  `models.toml` or `benchmarks/`.

## Comparability

Results are only comparable while every run does the same work. Flag any change to these, because
it silently invalidates earlier results unless a version is bumped:

- `benchmarks/*.toml`: changing tasks, answers, instructions or `max_tokens` needs a new `version`.
- `models.toml`: URLs must pin a commit and carry a SHA-256.
- `flake.lock` and the llama.cpp server flags in `crates/cli/src/server.rs`.
- The result schema in `crates/core/src/result.rs`: a breaking change needs a new `SCHEMA_VERSION`.

## Code

- Rust only. No new dependency for what a few lines can do.
- Behaviour changes come with a test that fails without them.
- Everything read from a result file is escaped before it reaches a page (`escape` in `crates/site/src/html.rs`).
- The website has no JavaScript.

## Workflows and infrastructure

- Flag any workflow change that runs untrusted input (issue or pull request text, fork code) with
  secrets or a write token, or that lets anyone but the repository owner start a Droid.
- Flag any change to `infra/` that widens the manager's access.

## Style of review

Report only what you are confident is wrong, with the file and line. No praise, no summaries of
what the pull request does, no style comments that `cargo fmt` or `cargo clippy` would settle.
