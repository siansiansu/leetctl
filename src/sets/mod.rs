//! Curated problem sets bundled into the binary, filterable without a network call.
use crate::err::Error;
use serde::Deserialize;
use std::collections::HashSet;

/// Every bundled set: the slug accepted on the command line, and its TOML embedded at compile time.
///
/// Adding a row here is what makes a slug valid in `--set` and in generated shell completions. It
/// compiles in the accepted spellings and fails the build if a file is missing — but nothing more.
/// That the TOML parses, that its inner `slug` matches the key here, and that its problems are
/// unique are all checked by the tests at the bottom of this file, not by the compiler.
const REGISTRY: &[(&str, &str)] = &[
    ("blind75", include_str!("../../data/sets/blind75.toml")),
    (
        "neetcode150",
        include_str!("../../data/sets/neetcode150.toml"),
    ),
    (
        "neetcode250",
        include_str!("../../data/sets/neetcode250.toml"),
    ),
    (
        "neetcode-all",
        include_str!("../../data/sets/neetcode-all.toml"),
    ),
    (
        "top-interview-150",
        include_str!("../../data/sets/top-interview-150.toml"),
    ),
    (
        "top-100-liked",
        include_str!("../../data/sets/top-100-liked.toml"),
    ),
    (
        "leetcode-75",
        include_str!("../../data/sets/leetcode-75.toml"),
    ),
    ("google", include_str!("../../data/sets/google.toml")),
    ("facebook", include_str!("../../data/sets/facebook.toml")),
    ("amazon", include_str!("../../data/sets/amazon.toml")),
    ("microsoft", include_str!("../../data/sets/microsoft.toml")),
];

/// One curated list, as stored in `data/sets/<slug>.toml`.
#[derive(Clone, Debug, Deserialize)]
pub struct ProblemSet {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub source_url: String,
    pub source_license: String,
    /// Vintage of the data itself, which is not the same as when it was last fetched. The company
    /// snapshots are `"2022"` however recently they were regenerated.
    pub source_as_of: String,
    pub generated_at: String,
    pub problems: Vec<SetProblem>,
}

/// A set member. `fid` is the LeetCode frontend id, which is what `Problem` is filtered on; `slug`
/// is carried alongside so the files stay auditable and regenerated diffs stay readable.
#[derive(Clone, Debug, Deserialize)]
pub struct SetProblem {
    pub fid: i32,
    pub slug: String,
}

impl ProblemSet {
    /// Membership test set, for filtering a problem list down to this set.
    pub fn fids(&self) -> HashSet<i32> {
        self.problems.iter().map(|problem| problem.fid).collect()
    }
}

/// Slugs accepted by `--set`, in registry order. Drives clap's value parser and its help text.
pub fn slugs() -> Vec<&'static str> {
    REGISTRY.iter().map(|(slug, _)| *slug).collect()
}

/// Load one set. Only the requested file is parsed.
pub fn get(slug: &str) -> Result<ProblemSet, Error> {
    let raw = REGISTRY
        .iter()
        .find(|(key, _)| *key == slug)
        .map(|(_, raw)| *raw)
        .ok_or_else(|| Error::UnknownSet(slug.to_string()))?;

    toml::from_str(raw).map_err(|source| Error::SetData {
        slug: slug.to_string(),
        source,
    })
}

/// Load every set, for the `sets` catalog.
pub fn all() -> Result<Vec<ProblemSet>, Error> {
    slugs().into_iter().map(get).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The counts the sets are named for, and — for the company snapshots, which have no canonical
    /// size — the size their source held when the data was last reviewed. Mirrors EXPECTED_COUNTS
    /// in scripts/gen_sets.py, so a regeneration that silently drops entries fails here too.
    const EXPECTED_COUNTS: &[(&str, usize)] = &[
        ("blind75", 75),
        ("neetcode150", 150),
        ("neetcode250", 250),
        ("neetcode-all", 450),
        ("top-interview-150", 150),
        ("top-100-liked", 100),
        ("leetcode-75", 75),
        ("google", 488),
        ("facebook", 371),
        ("amazon", 592),
        ("microsoft", 363),
    ];

    #[test]
    fn every_bundled_set_parses() {
        for slug in slugs() {
            get(slug).unwrap_or_else(|e| panic!("set `{slug}` failed to parse: {e}"));
        }
    }

    #[test]
    fn registry_keys_are_unique() {
        let unique: HashSet<&str> = slugs().into_iter().collect();
        assert_eq!(unique.len(), slugs().len(), "duplicate slug in REGISTRY");
    }

    #[test]
    fn registry_key_matches_the_slug_inside_the_file() {
        for slug in slugs() {
            assert_eq!(get(slug).unwrap().slug, slug);
        }
    }

    #[test]
    fn set_members_are_unique_by_both_fid_and_slug() {
        for slug in slugs() {
            let set = get(slug).unwrap();
            let fids: HashSet<i32> = set.problems.iter().map(|p| p.fid).collect();
            let problem_slugs: HashSet<&str> =
                set.problems.iter().map(|p| p.slug.as_str()).collect();
            assert_eq!(fids.len(), set.problems.len(), "{slug}: duplicate fid");
            assert_eq!(
                problem_slugs.len(),
                set.problems.len(),
                "{slug}: duplicate problem slug"
            );
        }
    }

    #[test]
    fn sets_have_their_expected_size() {
        for (slug, expected) in EXPECTED_COUNTS {
            assert_eq!(
                get(slug).unwrap().problems.len(),
                *expected,
                "{slug} changed size; regenerate and review before updating this number"
            );
        }
        assert_eq!(
            EXPECTED_COUNTS.len(),
            slugs().len(),
            "a set has no expected count"
        );
    }

    #[test]
    fn set_metadata_is_populated() {
        for slug in slugs() {
            let set = get(slug).unwrap();
            assert!(!set.name.is_empty(), "{slug}: no name");
            assert!(!set.description.is_empty(), "{slug}: no description");
            assert!(
                set.source_url.starts_with("https://"),
                "{slug}: source_url is not a URL"
            );
            assert!(!set.source_license.is_empty(), "{slug}: no source_license");
            assert!(!set.source_as_of.is_empty(), "{slug}: no source_as_of");
        }
    }

    #[test]
    fn unknown_slug_is_rejected() {
        assert!(matches!(get("nope"), Err(Error::UnknownSet(_))));
    }

    /// The NeetCode lists are published as nested tiers, and `neetcode250` is generated from the
    /// website bundle while the other two come from NeetCode's MIT data file — so this also
    /// guards the two sources against drifting apart.
    #[test]
    fn the_neetcode_lists_nest() {
        let blind = get("blind75").unwrap().fids();
        let hundred_fifty = get("neetcode150").unwrap().fids();
        let two_fifty = get("neetcode250").unwrap().fids();
        assert!(
            blind.is_subset(&hundred_fifty),
            "NeetCode 150 is supposed to extend Blind 75"
        );
        assert!(
            hundred_fifty.is_subset(&two_fifty),
            "NeetCode 250 is supposed to extend NeetCode 150"
        );
    }
}
