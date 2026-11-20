//! Page templates. Everything that comes from a result file passes through `escape`.

use crate::board::Board;
use crate::chart::{chart, duration};
use std::collections::BTreeMap;

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out += "&amp;",
            '<' => out += "&lt;",
            '>' => out += "&gt;",
            '"' => out += "&quot;",
            '\'' => out += "&#39;",
            c => out.push(c),
        }
    }
    out
}

/// The landing page: pick your hardware.
pub fn index_page(boards: &BTreeMap<String, Board>) -> String {
    let mut body = String::from(
        "<h1>Speedrun Evals</h1>\n<p>How long local LLMs take to finish a benchmark on real hardware, and how well they do. \
         Pick a machine.</p>\n<ul class=\"hardware\">\n",
    );
    for (slug, board) in boards {
        let models: usize = board.benchmarks.values().map(Vec::len).sum();
        body += &format!(
            "<li><a href=\"hw/{slug}.html\">{}</a> <span class=\"muted\">{models} result{}</span></li>\n",
            escape(&board.hardware.label()),
            if models == 1 { "" } else { "s" }
        );
    }
    body += "</ul>\n<p class=\"muted\">Is your machine missing? \
             <a href=\"https://github.com/vrinek/speedrun-evals#run-a-benchmark\">Run the benchmark and add it.</a></p>\n";
    page("Speedrun Evals", &body)
}

/// One hardware's page: a chart and a table per benchmark.
pub fn hardware_page(board: &Board) -> String {
    let label = escape(&board.hardware.label());
    let mut body = format!("<p><a href=\"../index.html\">All hardware</a></p>\n<h1>{label}</h1>\n");
    for (title, entries) in &board.benchmarks {
        body += &format!("<h2>{}</h2>\n{}\n", escape(title), chart(entries));
        body += "<table>\n<thead><tr><th>Model</th><th>Score</th><th>Time to finish</th><th>Output tokens</th></tr></thead>\n<tbody>\n";
        for e in entries {
            body += &format!(
                "<tr><td>{}</td><td>{:.0}%</td><td>{}</td><td>{}</td></tr>\n",
                escape(&e.model),
                e.score * 100.0,
                duration(e.wall_ms),
                e.output_tokens
            );
        }
        body += "</tbody>\n</table>\n";
    }
    page(&board.hardware.label(), &body)
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>{}</style>\n</head>\n<body>\n<main>\n{body}</main>\n</body>\n</html>\n",
        escape(title),
        include_str!("style.css")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{boards, tests::run};

    #[test]
    fn escapes_html_special_characters() {
        assert_eq!(
            escape(r#"<a href="x">&'</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn index_links_to_every_hardware() {
        let page = index_page(&boards(vec![
            run("CPU <A>", "m1", 100, true),
            run("CPU B", "m1", 100, true),
        ]));
        assert!(page.contains("href=\"hw/cpu-a--gpu-24gb--64gb-ram.html\""));
        assert!(page.contains("CPU &lt;A&gt; · GPU 24GB · 64GB RAM"));
        assert!(page.contains("href=\"hw/cpu-b--gpu-24gb--64gb-ram.html\""));
        assert!(!page.contains("<A>"));
    }

    #[test]
    fn hardware_page_has_a_chart_and_a_table_per_benchmark() {
        let boards = boards(vec![run("CPU", "good", 2000, true), run("CPU", "<bad>", 1000, false)]);
        let page = hardware_page(boards.values().next().unwrap());
        assert!(page.contains("<h2>quick v1</h2>"));
        assert_eq!(page.matches("<svg").count(), 1);
        // Table rows in ranking order, with the headline numbers.
        let good = page.find("<td>good</td>").unwrap();
        let bad = page.find("<td>&lt;bad&gt;</td>").unwrap();
        assert!(good < bad);
        assert!(page.contains("<td>100%</td><td>2.0 s</td>"), "{page}");
        assert!(!page.contains("<bad>"));
        assert!(
            page.contains("<th>Output tokens</th></tr>"),
            "no summary columns: every row is one run"
        );
    }
}
