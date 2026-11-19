//! The leaderboard chart: score against wall-clock time, one dot per model.
//! Rendered as static SVG; colours come from CSS variables in the page.

use crate::board::Entry;
use crate::html::escape;

const WIDTH: f64 = 720.0;
const HEIGHT: f64 = 380.0;
const LEFT: f64 = 56.0;
const RIGHT: f64 = 24.0;
const TOP: f64 = 16.0;
const BOTTOM: f64 = 48.0;
/// ponytail: labels are not collision-checked, so past this many models only the
/// hover tooltip and the table name the dots.
const MAX_LABELLED: usize = 8;

pub fn chart(entries: &[Entry]) -> String {
    let plot_w = WIDTH - LEFT - RIGHT;
    let plot_h = HEIGHT - TOP - BOTTOM;
    let bottom = HEIGHT - BOTTOM;
    let max_ms = entries.iter().map(|e| e.wall_ms).max().unwrap_or(0);
    // One unit for the whole axis, chosen from the slowest model.
    let (unit, unit_ms) = match max_ms {
        0..180_000 => ("seconds", 1000.0),
        180_000..10_800_000 => ("minutes", 60_000.0),
        _ => ("hours", 3_600_000.0),
    };
    let (step, end) = axis(max_ms as f64 / unit_ms);
    let x = |value: f64| LEFT + value / end * plot_w;
    let y = |score: f64| bottom - score * plot_h;

    let mut svg = format!(
        r#"<svg viewBox="0 0 {WIDTH} {HEIGHT}" role="img" aria-label="Score against time to finish, one dot per model. The table below has the same data.">"#
    );
    for percent in [0, 25, 50, 75, 100] {
        let gy = y(percent as f64 / 100.0);
        svg += &format!(
            r#"<line class="grid" x1="{LEFT}" x2="{}" y1="{gy}" y2="{gy}"/><text class="tick" x="{}" y="{}" text-anchor="end">{percent}%</text>"#,
            WIDTH - RIGHT,
            LEFT - 8.0,
            gy + 4.0
        );
    }
    let ticks = (end / step).round() as u32;
    for i in 0..=ticks {
        let value = i as f64 * step;
        // Trim float noise such as 0.30000000000000004.
        let label = format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
        svg += &format!(
            r#"<text class="tick" x="{}" y="{}" text-anchor="middle">{label}</text>"#,
            x(value),
            bottom + 18.0
        );
    }
    svg += &format!(
        r#"<text class="axis" x="{}" y="{}" text-anchor="middle">Time to finish in {unit} (left is faster)</text>"#,
        LEFT + plot_w / 2.0,
        HEIGHT - 8.0
    );

    for entry in entries {
        let (cx, cy) = (x(entry.wall_ms as f64 / unit_ms), y(entry.score));
        let name = escape(&entry.model);
        svg += &format!(
            r#"<g class="point"><title>{name}: {:.0}% in {}</title><circle class="dot" cx="{cx}" cy="{cy}" r="5"/><circle class="hit" cx="{cx}" cy="{cy}" r="14"/>"#,
            entry.score * 100.0,
            duration(entry.wall_ms)
        );
        if entries.len() <= MAX_LABELLED {
            // Keep the label inside the plot: flip it to the left of dots near the right edge.
            let (dx, anchor) = if cx > LEFT + plot_w * 0.7 {
                (-10.0, "end")
            } else {
                (10.0, "start")
            };
            svg += &format!(
                r#"<text class="label" x="{}" y="{}" text-anchor="{anchor}">{name}</text>"#,
                cx + dx,
                cy + 4.0
            );
        }
        svg += "</g>";
    }
    svg + "</svg>"
}

/// A round tick step and axis end that cover `max` in about five ticks.
pub fn axis(max: f64) -> (f64, f64) {
    if max <= 0.0 {
        return (0.2, 1.0);
    }
    let rough = max / 5.0;
    let magnitude = 10f64.powf(rough.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|s| *s >= rough * 0.999)
        .unwrap();
    // Round away float noise so ticks land on exact values.
    let round = |v: f64| (v * 1e6).round() / 1e6;
    (round(step), round((max / step - 1e-9).ceil() * step))
}

/// `42.3 s`, `12.5 min` or `3.2 h`, whichever reads best.
pub fn duration(ms: u64) -> String {
    match ms {
        0..180_000 => format!("{:.1} s", ms as f64 / 1000.0),
        180_000..10_800_000 => format!("{:.1} min", ms as f64 / 60_000.0),
        _ => format!("{:.1} h", ms as f64 / 3_600_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(model: &str, score: f64, wall_ms: u64) -> Entry {
        Entry {
            model: model.into(),
            score,
            wall_ms,
            output_tokens: 1,
            runs: 1,
        }
    }

    #[test]
    fn axis_steps_are_round_numbers() {
        assert_eq!(axis(47.0), (10.0, 50.0));
        assert_eq!(axis(100.0), (20.0, 100.0));
        assert_eq!(axis(7.3), (2.0, 8.0));
        assert_eq!(axis(0.4), (0.1, 0.4));
        assert_eq!(axis(0.0), (0.2, 1.0));
    }

    #[test]
    fn durations_pick_a_readable_unit() {
        assert_eq!(duration(42_300), "42.3 s");
        assert_eq!(duration(750_000), "12.5 min");
        assert_eq!(duration(11_520_000), "3.2 h");
    }

    #[test]
    fn draws_one_labelled_dot_per_model() {
        let svg = chart(&[entry("alpha", 0.9, 60_000), entry("beta", 0.5, 30_000)]);
        assert_eq!(svg.matches("<circle class=\"dot\"").count(), 2);
        assert!(svg.contains(">alpha</text>") && svg.contains(">beta</text>"));
        // Hover tooltip carries the exact values.
        assert!(svg.contains("<title>alpha: 90% in 60.0 s</title>"), "{svg}");
    }

    #[test]
    fn faster_is_further_left_and_better_is_higher() {
        let svg = chart(&[entry("fast-good", 1.0, 10_000), entry("slow-bad", 0.0, 50_000)]);
        let at = |name: &str| {
            let dot = svg.split("<g class=\"point\">").find(|g| g.contains(name)).unwrap();
            let num = |attr: &str| {
                dot.split(&format!("{attr}=\""))
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            };
            (num("cx"), num("cy"))
        };
        let (fast, slow) = (at("fast-good"), at("slow-bad"));
        assert!(fast.0 < slow.0 && fast.1 < slow.1);
        assert_eq!(fast.1, TOP);
        assert_eq!(slow.1, HEIGHT - BOTTOM);
    }

    #[test]
    fn crowded_charts_drop_direct_labels_but_keep_tooltips() {
        let many: Vec<_> = (0..MAX_LABELLED + 1)
            .map(|i| entry(&format!("m{i}"), 0.5, 1000 * (i as u64 + 1)))
            .collect();
        let svg = chart(&many);
        assert!(!svg.contains("class=\"label\""));
        assert_eq!(svg.matches("<title>").count(), many.len());
    }

    #[test]
    fn model_names_are_escaped() {
        let svg = chart(&[entry("<script>", 0.5, 1000)]);
        assert!(!svg.contains("<script>") && svg.contains("&lt;script&gt;"));
    }
}
