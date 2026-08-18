//! Narrowing a problem list, shared by `leetctl list` and the TUI
use crate::cache::models::Problem;
use crate::err::Error;
use crate::helper::Difficulty;

/// Everything a problem list can be narrowed by, in one value.
///
/// Both frontends build this and hand it to [`apply`], so a filter cannot behave differently
/// between them. Fields are all optional; an empty `ProblemFilters` keeps every problem.
///
/// `tag_ids` is the resolved id list rather than a tag name, which keeps [`apply`] synchronous
/// and offline — fetching a tag's members is a network call the caller makes first.
#[derive(Clone, Debug, Default)]
pub struct ProblemFilters {
    /// Substring of the problem name, case-insensitive.
    pub keyword: Option<String>,
    /// Exact category, e.g. `algorithms`.
    pub category: Option<String>,
    /// Query-letter conditions — see [`crate::helper::filter`].
    pub query: Option<String>,
    /// Slug of a bundled problem set.
    pub set: Option<String>,
    pub difficulty: Option<Difficulty>,
    /// Inclusive frontend-id bounds.
    pub range: Option<(i32, i32)>,
    /// Internal problem ids a tag resolved to.
    pub tag_ids: Option<Vec<String>>,
    /// Frontend ids the review deck reports due, resolved by the caller for the same reason
    /// `tag_ids` is: reading the deck is a cache call, and [`apply`] stays synchronous.
    pub due_fids: Option<Vec<i32>>,
}

/// Narrow `ps` in place and sort it by frontend id.
///
/// The order the filters run in is the order `leetctl list` has always run them in. It matters:
/// [`crate::helper::squash`] and [`crate::helper::retain_set`] join on different id spaces, and a
/// caller reading the code should see one sequence rather than reconstruct it per frontend.
pub fn apply(ps: &mut Vec<Problem>, filters: &ProblemFilters) -> Result<(), Error> {
    if let Some(ref tag_ids) = filters.tag_ids {
        crate::helper::squash(ps, tag_ids.clone())?;
    }

    if let Some(ref set_slug) = filters.set {
        crate::helper::retain_set(ps, set_slug)?;
    }

    if let Some(ref due_fids) = filters.due_fids {
        // Indexed first: a linear scan of the deck per problem is 4,000 x the deck size, and this
        // runs on every keystroke in the TUI.
        let due: std::collections::HashSet<i32> = due_fids.iter().copied().collect();
        ps.retain(|p| due.contains(&p.fid));
    }

    if let Some(ref category) = filters.category {
        ps.retain(|p| p.category == *category);
    }

    if let Some(ref query) = filters.query {
        crate::helper::filter(ps, query.to_string());
    }

    if let Some(difficulty) = filters.difficulty {
        ps.retain(|p| p.level == difficulty.level());
    }

    if let Some((low, high)) = filters.range {
        ps.retain(|p| low <= p.fid && p.fid <= high);
    }

    if let Some(ref keyword) = filters.keyword {
        let lowercase_keyword = keyword.to_lowercase();
        ps.retain(|p| p.name.to_lowercase().contains(&lowercase_keyword));
    }

    ps.sort_unstable_by_key(|p| p.fid);

    Ok(())
}

/// How a problem list breaks down: `leetctl list --stat`, and the TUI's footer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgressStats {
    pub listed: i32,
    pub locked: i32,
    pub starred: i32,
    pub ac: i32,
    pub notac: i32,
    pub easy: i32,
    pub medium: i32,
    pub hard: i32,
    pub easy_ac: i32,
    pub medium_ac: i32,
    pub hard_ac: i32,
}

impl ProgressStats {
    /// Problems neither solved nor attempted.
    pub fn remain(&self) -> i32 {
        self.listed - self.ac - self.notac
    }
}

/// Count a problem list by status, difficulty, and flags.
pub fn progress(ps: &[Problem]) -> ProgressStats {
    let mut stats = ProgressStats {
        listed: ps.len() as i32,
        ..Default::default()
    };

    for p in ps {
        let is_ac = p.status == "ac";
        if p.starred {
            stats.starred += 1;
        }
        if p.locked {
            stats.locked += 1;
        }

        match p.status.as_str() {
            "ac" => stats.ac += 1,
            "notac" => stats.notac += 1,
            _ => {}
        }

        let (total, solved) = match Difficulty::from_level(p.level) {
            Some(Difficulty::Easy) => (&mut stats.easy, &mut stats.easy_ac),
            Some(Difficulty::Medium) => (&mut stats.medium, &mut stats.medium_ac),
            Some(Difficulty::Hard) => (&mut stats.hard, &mut stats.hard_ac),
            None => continue,
        };
        *total += 1;
        if is_ac {
            *solved += 1;
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::models::fixture;

    /// Checked against data/sets/blind75.toml: 1, 11 and 217 are members; 4 and 42 are not.
    /// The levels are the fixture's own and do not have to match LeetCode's.
    fn pool() -> Vec<Problem> {
        vec![
            fixture(217, 1, "Contains Duplicate"),
            fixture(1, 1, "Two Sum"),
            fixture(42, 1, "Trapping Rain Water"),
            fixture(4, 3, "Median of Two Sorted Arrays"),
            fixture(11, 2, "Container With Most Water"),
        ]
    }

    fn narrowed(filters: &ProblemFilters) -> Vec<i32> {
        let mut ps = pool();
        apply(&mut ps, filters).unwrap();
        ps.iter().map(|p| p.fid).collect()
    }

    #[test]
    fn no_filters_keeps_everything_sorted_by_id() {
        assert_eq!(
            narrowed(&ProblemFilters::default()),
            vec![1, 4, 11, 42, 217]
        );
    }

    #[test]
    fn difficulty_keeps_only_that_level() {
        let filters = ProblemFilters {
            difficulty: Some(Difficulty::Hard),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), vec![4]);
    }

    #[test]
    fn set_keeps_only_members() {
        let filters = ProblemFilters {
            set: Some("blind75".into()),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), vec![1, 11, 217]);
    }

    #[test]
    fn unknown_set_is_an_error() {
        let filters = ProblemFilters {
            set: Some("not-a-set".into()),
            ..Default::default()
        };
        let mut ps = pool();
        assert!(matches!(
            apply(&mut ps, &filters),
            Err(Error::UnknownSet(_))
        ));
    }

    #[test]
    fn range_bounds_are_inclusive() {
        let filters = ProblemFilters {
            range: Some((4, 42)),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), vec![4, 11, 42]);
    }

    #[test]
    fn keyword_matches_the_name_case_insensitively() {
        let filters = ProblemFilters {
            keyword: Some("wAtEr".into()),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), vec![11, 42]);
    }

    #[test]
    fn query_letters_narrow_by_condition() {
        // trace: 'e' keeps level 1 (Easy), 'L' keeps unlocked — the whole fixture is unlocked.
        let filters = ProblemFilters {
            query: Some("eL".into()),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), vec![1, 42, 217]);
    }

    #[test]
    fn tag_ids_join_on_the_internal_id() {
        // The fixture's internal ids mirror its frontend ids, which is what `squash` matches on.
        let filters = ProblemFilters {
            tag_ids: Some(vec!["1".into(), "4".into()]),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), vec![1, 4]);
    }

    #[test]
    fn filters_compose() {
        let filters = ProblemFilters {
            set: Some("blind75".into()),
            difficulty: Some(Difficulty::Easy),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), vec![1, 217]);
    }

    #[test]
    fn a_pool_narrowed_to_nothing_is_empty_not_an_error() {
        let filters = ProblemFilters {
            keyword: Some("no such problem".into()),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), Vec::<i32>::new());
    }

    #[test]
    fn due_fids_keep_only_the_deck() {
        let filters = ProblemFilters {
            due_fids: Some(vec![11, 217]),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), vec![11, 217]);
    }

    #[test]
    fn an_empty_deck_is_not_the_same_as_no_deck_filter() {
        // `Some(vec![])` is "nothing is due", which has to narrow to nothing rather than being
        // treated as an unset filter.
        let filters = ProblemFilters {
            due_fids: Some(vec![]),
            ..Default::default()
        };
        assert_eq!(narrowed(&filters), Vec::<i32>::new());
    }

    #[test]
    fn progress_counts_status_difficulty_and_flags() {
        let mut ps = pool();
        ps[0].status = "ac".into();
        ps[1].status = "notac".into();
        ps[2].locked = true;
        ps[3].starred = true;

        let stats = progress(&ps);

        assert_eq!(
            stats,
            ProgressStats {
                listed: 5,
                locked: 1,
                starred: 1,
                ac: 1,
                notac: 1,
                easy: 3,
                medium: 1,
                hard: 1,
                easy_ac: 1,
                medium_ac: 0,
                hard_ac: 0,
            }
        );
        assert_eq!(stats.remain(), 3);
    }

    #[test]
    fn progress_of_an_empty_list_is_all_zero() {
        let stats = progress(&[]);

        assert_eq!(stats, ProgressStats::default());
        assert_eq!(stats.remain(), 0);
    }
}
