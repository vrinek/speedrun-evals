mod board;
mod chart;
mod html;

use speedrun_core::RunResult;
use std::{env, fs, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let [results, out] = &args[..] else {
        eprintln!("usage: speedrun-site <results-dir> <output-dir>");
        return ExitCode::FAILURE;
    };
    match build(Path::new(results), Path::new(out)) {
        Ok(pages) => {
            eprintln!("Wrote {pages} pages to {out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Reads every result file and writes the static site. Returns the page count.
/// Any invalid result file fails the build, which is how CI checks submissions.
fn build(results: &Path, out: &Path) -> Result<usize, String> {
    let mut runs = Vec::new();
    load(results, &mut runs)?;
    let boards = board::boards(runs);
    let write =
        |path: std::path::PathBuf, page: String| fs::write(&path, page).map_err(|e| format!("{}: {e}", path.display()));
    fs::create_dir_all(out.join("hw")).map_err(|e| format!("{}: {e}", out.display()))?;
    write(out.join("index.html"), html::index_page(&boards))?;
    for (slug, board) in &boards {
        write(out.join("hw").join(format!("{slug}.html")), html::hardware_page(board))?;
    }
    Ok(boards.len() + 1)
}

/// Every `.json` file under `dir`, in a stable order.
fn load(dir: &Path, runs: &mut Vec<RunResult>) -> Result<(), String> {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .and_then(|entries| entries.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>())
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    paths.sort();
    for path in paths {
        if path.is_dir() {
            load(&path, runs)?;
        } else if path.extension().is_some_and(|ext| ext == "json") {
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            runs.push(RunResult::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::tests::run;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = env::temp_dir().join(format!("speedrun-site-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("results/nested")).unwrap();
        dir
    }

    #[test]
    fn builds_an_index_and_a_page_per_hardware() {
        let dir = temp("ok");
        for (file, cpu) in [("results/a.json", "CPU A"), ("results/nested/b.json", "CPU B")] {
            fs::write(
                dir.join(file),
                serde_json::to_string(&run(cpu, "m", 100, true)).unwrap(),
            )
            .unwrap();
        }
        fs::write(dir.join("results/README.md"), "not a result").unwrap();

        assert_eq!(build(&dir.join("results"), &dir.join("dist")), Ok(3));
        assert!(
            fs::read_to_string(dir.join("dist/index.html"))
                .unwrap()
                .contains("CPU A")
        );
        assert!(
            fs::read_to_string(dir.join("dist/hw/cpu-b--gpu-24gb--64gb-ram.html"))
                .unwrap()
                .contains("<svg")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_invalid_result_file_fails_the_build_and_is_named() {
        let dir = temp("bad");
        fs::write(dir.join("results/nested/broken.json"), "{\"schema_version\": 1}").unwrap();
        let err = build(&dir.join("results"), &dir.join("dist")).unwrap_err();
        assert!(err.contains("broken.json"), "{err}");
        fs::remove_dir_all(dir).unwrap();
    }
}
