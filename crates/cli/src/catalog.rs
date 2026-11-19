//! The benchmarks and models this build knows about. Both are compiled in, so a
//! given CLI version always runs exactly the same tasks against the same files.

use serde::Deserialize;
use speedrun_core::Suite;

const MODELS: &str = include_str!("../../../models.toml");
/// To add a benchmark: add a file under `benchmarks/` and one line here.
const SUITES: &[&str] = &[include_str!("../../../benchmarks/quick.toml")];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub id: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelFile {
    model: Vec<Model>,
}

pub fn models() -> Vec<Model> {
    toml::from_str::<ModelFile>(MODELS)
        .expect("models.toml is checked by the tests")
        .model
}

pub fn suites() -> Vec<Suite> {
    SUITES
        .iter()
        .map(|text| Suite::parse(text).expect("suites are checked by the tests"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_model_is_pinned_and_unique() {
        let models = models();
        assert!(!models.is_empty());
        let ids: HashSet<_> = models.iter().map(|m| &m.id).collect();
        assert_eq!(ids.len(), models.len(), "duplicate model id");
        for m in &models {
            assert!(
                m.id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c)),
                "{}",
                m.id
            );
            assert!(m.url.starts_with("https://"), "{}", m.id);
            assert!(
                m.url.contains("/resolve/") && !m.url.contains("/resolve/main/"),
                "{} must pin a commit",
                m.id
            );
            assert!(
                m.sha256.len() == 64 && m.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                "{}",
                m.id
            );
        }
    }

    #[test]
    fn every_suite_parses_and_names_are_unique() {
        let suites = suites();
        let names: HashSet<_> = suites.iter().map(|s| &s.name).collect();
        assert_eq!(names.len(), suites.len());
        assert!(suites.iter().all(|s| s.instructions.contains("ANSWER:")));
    }
}
