# Instructions for agents working on this repository

Read `README.md` first. It explains what the project is and where things live.

## Checks

Run all of these before every commit. All must pass.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -p speedrun-site -- results dist
```

## Rules

1. Rust only. No JavaScript on the website, no scripts in other languages.
2. Test first: write a failing test, run it, see it fail, then write the code.
3. Small commits, one purpose each. Never amend or force-push.
4. Never push to `main`. Open a pull request from a branch.
5. Add a dependency only when the standard library cannot do the job in a few lines.
6. Never write, edit or "fix" a file under `results/` by hand. Result files come
   only from `speedrun run`. If one is wrong, ask the submitter to rerun.
7. Never change the tasks of an existing benchmark without raising its `version`.
8. Every string taken from a result file goes through `html::escape` before it reaches a page.
9. If you are not sure, say so and ask a maintainer. Do not guess.

## How to do common jobs

**Review a result submission (a pull request that only adds files under `results/`).**
- CI must be green. It parses every result file.
- The pull request must add files only. It must not change existing results or any code.
- Each file's directory must be the hardware slug the tool generated; the file name must be unchanged.
- `model.id` and `model.sha256` must match an entry in `models.toml`.
- Compare with existing results for similar hardware. If the time is several
  times better than comparable machines, ask how the run was made. Do not merge.
- If all of that holds, approve. A maintainer merges.

**Add a model.** Add an entry to `models.toml`. The URL must contain
`/resolve/<commit>/`, never `/resolve/main/`. Take the SHA-256 from the file's
page on Hugging Face. Do not invent or guess either value. `cargo test` checks the format.

**Add a benchmark.** Add `benchmarks/<name>.toml` in the format of `quick.toml`
and add one `include_str!` line to `SUITES` in `crates/cli/src/catalog.rs`.
Each task needs exactly one correct answer that can be compared as text.
Solve every task yourself and check the answer before committing it.

**Triage an issue.** Decide which label fits: `bug`, `benchmark`, `model`,
`website` or `question`. If your account cannot add labels, name the label in
your comment. Ask for the output of `speedrun hardware` and the exact command
when a bug report lacks them. For a duplicate, link the original and say it can be closed.
Never add the `droid` label: it starts an automated implementation and only the
repository owner may decide that. If an issue is clear, small and ready to build,
say so in a comment and mention the owner.

## Known shortcuts

Deliberate simplifications are marked with a `ponytail:` comment that names the
limit and the upgrade path. Search for them before proposing larger changes:

```sh
grep -rn "ponytail:" --include='*.rs' --include='*.nix' --include='*.toml' .
```
