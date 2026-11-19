//! Shared types for Speedrun Evals: the benchmark suite format and the
//! result file format that the CLI writes and the website reads.

mod result;
mod suite;

pub use result::{Gpu, Hardware, ModelRef, RunResult, Runtime, SCHEMA_VERSION, TaskResult, slug};
pub use suite::{Suite, Task, extract_answer};
