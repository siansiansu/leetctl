//! The `/` prompt's matcher
use crate::cache::models::Problem;

/// Whether `p` satisfies every token in `query`.
///
/// Tokens are whitespace-separated and ANDed; a `!` prefix excludes instead of requires. Each token
/// is matched as a substring against the id, name, slug, difficulty, and status together, so
/// `easy !ac tree` reads as "easy tree problems I have not solved".
///
/// Matching is substring-only on purpose. Subsequence matching (typing `lcp` for
/// *Longest Common Prefix*) needs ranking to stay useful — without it, three letters match hundreds
/// of the four thousand problems and the table stops being an answer.
pub(crate) fn matches(p: &Problem, query: &str) -> bool {
    if query.trim().is_empty() {
        return true;
    }

    let haystack = haystack(p);
    query.split_whitespace().all(|token| {
        match token.strip_prefix('!') {
            // A bare `!` is someone mid-word; it should not blank the table.
            Some("") => true,
            Some(excluded) => !haystack.contains(&excluded.to_lowercase()),
            None => haystack.contains(&token.to_lowercase()),
        }
    })
}

fn haystack(p: &Problem) -> String {
    let difficulty = crate::helper::Difficulty::from_level(p.level).map_or("", |d| d.as_str());

    format!(
        "{} {} {} {} {}",
        p.fid,
        p.name.to_lowercase(),
        p.slug.to_lowercase(),
        difficulty.to_lowercase(),
        p.status.to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::models::fixture;

    fn two_sum() -> Problem {
        fixture(1, 1, "Two Sum")
    }

    #[test]
    fn an_empty_query_matches_everything() {
        assert!(matches(&two_sum(), ""));
        assert!(matches(&two_sum(), "   "));
    }

    #[test]
    fn tokens_are_anded_and_case_insensitive() {
        assert!(matches(&two_sum(), "TWO sum"));
        assert!(!matches(&two_sum(), "two three"));
    }

    #[test]
    fn a_bang_prefix_excludes() {
        assert!(matches(&two_sum(), "sum !hard"));
        assert!(!matches(&two_sum(), "!sum"));
    }

    #[test]
    fn a_lone_bang_is_ignored_rather_than_matching_nothing() {
        assert!(matches(&two_sum(), "two !"));
    }

    #[test]
    fn the_id_difficulty_and_status_are_searchable() {
        let mut solved = fixture(704, 1, "Binary Search");
        solved.status = "ac".into();

        assert!(matches(&solved, "704"));
        assert!(matches(&solved, "easy"));
        assert!(matches(&solved, "ac"));
        assert!(!matches(&solved, "hard"));
    }

    #[test]
    fn the_slug_is_searchable_so_hyphenated_names_work() {
        assert!(matches(
            &fixture(11, 2, "Container With Most Water"),
            "with-most"
        ));
    }
}
