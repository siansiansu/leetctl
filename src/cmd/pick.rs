//! Pick command
use crate::cache::{Cache, models::Problem};
use crate::err::Error;
use crate::helper::Difficulty;
use clap::Args;
use rand::RngExt;

static QUERY_HELP: &str = r#"Filter questions by conditions:
Uppercase means negative
e = easy     E = m+h
m = medium   M = e+h
h = hard     H = e+m
d = done     D = not done
l = locked   L = not locked
s = starred  S = not starred"#;

static PICK_AFTER_HELP: &str = r#"EXAMPLES:
    leetctl pick                                  Pick a random problem
    leetctl pick 1                                Pick problem 1
    leetctl pick --daily                          Pick today's daily challenge
    leetctl pick -D medium                        Random medium problem
    leetctl pick -t dynamic-programming -D hard   Random hard dynamic-programming problem
    leetctl pick -S blind75 -D medium             Random medium problem from Blind 75
    leetctl pick -S neetcode150 -q DL             Random unsolved, unlocked NeetCode 150 problem

Filters narrow the pool a random pick draws from, so they cannot be combined with an explicit
problem id or with --daily, which name a single problem outright. Run `leetctl sets` for the
available --set values.
"#;

/// The args that narrow the pool a random pick draws from, as one clap group so that naming a
/// single problem outright — by id or `--daily` — can exclude all of them without this list being
/// written down twice.
const FILTER_ARGS: [&str; 5] = ["plan", "query", "tag", "difficulty", "set"];
const FILTER_GROUP: &str = "filters";

/// How many times to re-request a problem's description before giving up on a flaky network.
const MAX_FETCH_ATTEMPTS: u32 = 3;

/// Pick command arguments
#[derive(Args, Debug)]
#[command(after_help = PICK_AFTER_HELP)]
#[command(group = clap::ArgGroup::new(FILTER_GROUP).args(FILTER_ARGS).multiple(true))]
pub struct PickArgs {
    /// Problem id
    #[arg(
        value_parser = clap::value_parser!(i32),
        conflicts_with_all = ["name", "daily", FILTER_GROUP],
    )]
    pub id: Option<i32>,

    /// Problem name
    #[arg(short = 'n', long, conflicts_with = "daily")]
    pub name: Option<String>,

    /// Invoking python scripts to filter questions
    #[arg(short = 'p', long)]
    pub plan: Option<String>,

    /// Filter questions by conditions
    #[arg(short, long, help = QUERY_HELP)]
    pub query: Option<String>,

    /// Filter questions by tag, e.g. array, dynamic-programming
    #[arg(short, long)]
    pub tag: Option<String>,

    /// Filter questions by difficulty
    #[arg(short = 'D', long, value_enum, ignore_case = true)]
    pub difficulty: Option<Difficulty>,

    /// Filter questions by curated set, e.g. blind75, neetcode150, google
    #[arg(
        short = 'S',
        long,
        value_parser = clap::builder::PossibleValuesParser::new(crate::sets::slugs()),
    )]
    pub set: Option<String>,

    /// Pick today's daily challenge
    #[arg(short = 'd', long, conflicts_with = FILTER_GROUP)]
    pub daily: bool,
}

impl PickArgs {
    /// `pick` handler
    pub async fn run(&self) -> Result<(), Error> {
        super::ensure_plan_supported(self.plan.as_ref())?;

        let cache = Cache::new()?;
        // Every target reads the local problem cache — `get_question` looks the problem up in
        // sqlite before it fetches anything — so the cache has to be populated before any of them
        // resolve, not just before a random draw.
        let problems = super::populated_problems(&cache).await?;
        let fid = self.target_fid(&cache, problems).await?;

        // Only the description fetch is retried, and only for the problem already chosen. Retrying
        // the whole command would re-roll the random pick, so a flaky network would silently hand
        // back a different problem than the one being fetched.
        let mut attempt = 0;
        loop {
            attempt += 1;
            match cache.get_question(fid).await {
                Ok(question) => {
                    println!("{}", question.desc());
                    return Ok(());
                }
                Err(Error::Reqwest(e)) if attempt < MAX_FETCH_ATTEMPTS => {
                    eprintln!(
                        "Fetching problem {fid} failed ({e}), \
                         retrying ({attempt}/{MAX_FETCH_ATTEMPTS})..."
                    );
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Which problem this invocation is about: named outright, or drawn from the filtered pool.
    async fn target_fid(&self, cache: &Cache, problems: Vec<Problem>) -> Result<i32, Error> {
        if self.daily {
            return cache.get_daily_problem_id().await;
        }
        if let Some(id) = self.id {
            return Ok(id);
        }

        let candidates = self.candidates(cache, problems).await?;
        match self.name {
            Some(ref name) => closest_named_problem(&candidates, name)
                .ok_or_else(|| Error::NoProblemsMatch(self.describe_filters())),
            None => {
                let index = rand::rng().random_range(0..candidates.len());
                Ok(candidates[index].fid)
            }
        }
    }

    /// The filtered pool. Never empty — an empty pool is the error, not a sampling edge case.
    async fn candidates(
        &self,
        cache: &Cache,
        mut problems: Vec<Problem>,
    ) -> Result<Vec<Problem>, Error> {
        #[cfg(feature = "pym")]
        if let Some(ref plan) = self.plan {
            let ids = crate::pym::exec(plan)?;
            crate::helper::squash(&mut problems, ids)?;
        }

        // Tag membership is the one filter that needs the network, so it stays here rather than
        // in the offline pass below.
        if let Some(ref tag) = self.tag {
            let ids = cache.clone().get_tagged_questions(tag).await?;
            crate::helper::squash(&mut problems, ids)?;
        }

        self.narrow_offline(problems)
    }

    /// Set, query, and difficulty filters — everything decidable from the local cache alone —
    /// plus the single emptiness check that keeps a filtered-to-nothing pool from reaching the
    /// random draw. Split out from [`PickArgs::candidates`] so that path is testable without a
    /// cache.
    fn narrow_offline(&self, mut problems: Vec<Problem>) -> Result<Vec<Problem>, Error> {
        if let Some(ref set_slug) = self.set {
            crate::helper::retain_set(&mut problems, set_slug)?;
        }
        if let Some(ref query) = self.query {
            crate::helper::filter(&mut problems, query.to_string());
        }
        if let Some(difficulty) = self.difficulty {
            problems.retain(|problem| problem.level == difficulty.level());
        }

        if problems.is_empty() {
            return Err(Error::NoProblemsMatch(self.describe_filters()));
        }
        Ok(problems)
    }

    /// The active filters, for an error message that says which combination came up empty.
    fn describe_filters(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref set_slug) = self.set {
            parts.push(format!("set `{set_slug}`"));
        }
        if let Some(ref tag) = self.tag {
            parts.push(format!("tag `{tag}`"));
        }
        if let Some(difficulty) = self.difficulty {
            parts.push(format!(
                "difficulty `{}`",
                difficulty.as_str().to_lowercase()
            ));
        }
        if let Some(ref query) = self.query {
            parts.push(format!("query `{query}`"));
        }
        if let Some(ref plan) = self.plan {
            parts.push(format!("plan `{plan}`"));
        }
        if let Some(ref name) = self.name {
            parts.push(format!("name `{name}`"));
        }

        if parts.is_empty() {
            "the current filters".to_string()
        } else {
            parts.join(" + ")
        }
    }
}

// Returns the closest problem according to a scoring algorithm
// taking into account both the longest common subsequence and the size
// problem string (to compensate for smaller strings having smaller lcs).
// Returns None if there are no problems in the problem list
fn closest_named_problem(problems: &[Problem], lookup_name: &str) -> Option<i32> {
    let max_name_size: usize = problems.iter().map(|p| p.name.len()).max()?;
    // Init table to the max name length of all the problems to share
    // the same table allocation
    let mut table: Vec<usize> = vec![0; (max_name_size + 1) * (lookup_name.len() + 1)];

    // this is guaranteed because of the earlier max None propegation
    assert!(!problems.is_empty());
    let mut max_score = 0;
    let mut current_problem = &problems[0];
    for problem in problems {
        // In case bug becomes bugged, always return the matching string
        if problem.name == lookup_name {
            return Some(problem.fid);
        }

        let this_lcs = longest_common_subsequence(&mut table, &problem.name, lookup_name);
        let this_score = this_lcs * (max_name_size - problem.name.len());

        if this_score > max_score {
            max_score = this_score;
            current_problem = problem;
        }
    }

    Some(current_problem.fid)
}

// Longest commong subsequence DP approach O(nm) space and time. Table must be at least
// (text1.len() + 1) * (text2.len() + 1) length or greater and is mutated every call
fn longest_common_subsequence(table: &mut [usize], text1: &str, text2: &str) -> usize {
    assert!(table.len() >= (text1.len() + 1) * (text2.len() + 1));
    let height: usize = text1.len() + 1;
    let width: usize = text2.len() + 1;

    // initialize base cases to 0
    for i in 0..height {
        table[i * width + (width - 1)] = 0;
    }
    for j in 0..width {
        table[((height - 1) * width) + j] = 0;
    }

    let mut i: usize = height - 1;
    let mut j: usize;
    for c0 in text1.chars().rev() {
        i -= 1;
        j = width - 1;
        for c1 in text2.chars().rev() {
            j -= 1;
            if c0.to_lowercase().next() == c1.to_lowercase().next() {
                table[i * width + j] = 1 + table[(i + 1) * width + j + 1];
            } else {
                let a = table[(i + 1) * width + j];
                let b = table[i * width + j + 1];
                table[i * width + j] = std::cmp::max(a, b);
            }
        }
    }
    table[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, FromArgMatches, Parser};

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        pick: PickArgs,
    }

    fn parse(args: &[&str]) -> Result<PickArgs, clap::Error> {
        let matches = TestCli::command().try_get_matches_from(args)?;
        PickArgs::from_arg_matches(&matches)
    }

    fn problem(fid: i32, level: i32) -> Problem {
        Problem {
            category: "algorithms".into(),
            fid,
            id: fid,
            level,
            locked: false,
            name: format!("problem {fid}"),
            percent: 50.0,
            slug: format!("problem-{fid}"),
            starred: false,
            status: String::new(),
            desc: String::new(),
        }
    }

    /// Checked against data/sets/blind75.toml: 1, 11 and 217 are members; 4 and 42 are not.
    /// The levels are the fixture's own and do not have to match LeetCode's.
    fn fixture() -> Vec<Problem> {
        vec![
            problem(1, 1),
            problem(4, 3),
            problem(11, 2),
            problem(42, 1),
            problem(217, 1),
        ]
    }

    fn fids(problems: &[Problem]) -> Vec<i32> {
        problems.iter().map(|p| p.fid).collect()
    }

    #[test]
    fn difficulty_filter_keeps_only_that_level() {
        let args = parse(&["pick", "--difficulty", "hard"]).unwrap();
        let kept = args.narrow_offline(fixture()).unwrap();
        assert_eq!(fids(&kept), vec![4]);
    }

    #[test]
    fn set_filter_keeps_only_members() {
        let args = parse(&["pick", "--set", "blind75"]).unwrap();
        let kept = args.narrow_offline(fixture()).unwrap();
        assert_eq!(fids(&kept), vec![1, 11, 217]);
    }

    #[test]
    fn set_and_difficulty_compose() {
        let args = parse(&["pick", "-S", "blind75", "-D", "easy"]).unwrap();
        let kept = args.narrow_offline(fixture()).unwrap();
        assert_eq!(fids(&kept), vec![1, 217]);
    }

    /// The path that used to panic in `random_range(0..0)`.
    #[test]
    fn a_pool_filtered_to_nothing_is_an_error_not_a_panic() {
        // -q h keeps hard, -D easy keeps easy: nothing is both.
        let args = parse(&["pick", "-q", "h", "-D", "easy"]).unwrap();
        match args.narrow_offline(fixture()) {
            Err(Error::NoProblemsMatch(filters)) => {
                assert_eq!(filters, "difficulty `easy` + query `h`");
            }
            other => panic!(
                "expected NoProblemsMatch, got {:?}",
                other.map(|p| fids(&p))
            ),
        }
    }

    #[test]
    fn a_set_with_no_cached_members_is_an_error() {
        let args = parse(&["pick", "-S", "blind75"]).unwrap();
        // 4 and 42 are not in Blind 75, so the set filter empties this pool.
        let outside_the_set = vec![problem(4, 3), problem(42, 1)];
        assert!(matches!(
            args.narrow_offline(outside_the_set),
            Err(Error::NoProblemsMatch(_))
        ));
    }

    #[test]
    fn an_unfiltered_pool_passes_through_untouched() {
        let args = parse(&["pick"]).unwrap();
        let kept = args.narrow_offline(fixture()).unwrap();
        assert_eq!(fids(&kept), vec![1, 4, 11, 42, 217]);
    }

    #[test]
    fn empty_pool_is_described_by_its_filters() {
        let args = parse(&[
            "pick",
            "-S",
            "blind75",
            "-t",
            "dynamic-programming",
            "-D",
            "hard",
        ])
        .unwrap();
        assert_eq!(
            args.describe_filters(),
            "set `blind75` + tag `dynamic-programming` + difficulty `hard`"
        );
    }

    #[test]
    fn no_filters_still_describes_itself() {
        let args = parse(&["pick"]).unwrap();
        assert_eq!(args.describe_filters(), "the current filters");
    }

    #[test]
    fn difficulty_values_are_case_insensitive() {
        assert_eq!(
            parse(&["pick", "-D", "MEDIUM"]).unwrap().difficulty,
            Some(Difficulty::Medium)
        );
    }

    #[test]
    fn unknown_difficulty_is_rejected() {
        assert!(parse(&["pick", "-D", "impossible"]).is_err());
    }

    #[test]
    fn unknown_set_is_rejected_before_any_network_call() {
        let err = parse(&["pick", "--set", "nope"]).unwrap_err();
        assert!(
            err.to_string().contains("blind75"),
            "the error should list the valid sets, got: {err}"
        );
    }

    #[test]
    fn every_registered_set_is_accepted() {
        for slug in crate::sets::slugs() {
            assert!(parse(&["pick", "--set", slug]).is_ok(), "rejected {slug}");
        }
    }

    #[test]
    fn an_explicit_id_conflicts_with_every_filter() {
        for filter in [
            vec!["--set", "blind75"],
            vec!["--difficulty", "easy"],
            vec!["--tag", "array"],
            vec!["--query", "e"],
            vec!["--plan", "p"],
        ] {
            let mut args = vec!["pick", "1"];
            args.extend(filter.iter());
            assert!(parse(&args).is_err(), "id should conflict with {filter:?}");
        }
    }

    #[test]
    fn daily_conflicts_with_every_filter_and_with_naming_a_problem() {
        assert!(parse(&["pick", "--daily", "--difficulty", "easy"]).is_err());
        assert!(parse(&["pick", "--daily", "--set", "blind75"]).is_err());
        assert!(parse(&["pick", "--daily", "--tag", "array"]).is_err());
        assert!(parse(&["pick", "--daily", "1"]).is_err());
        assert!(parse(&["pick", "--daily", "--name", "two sum"]).is_err());
    }

    #[test]
    fn filters_still_narrow_a_name_search() {
        // --name searches within the filtered pool, so it is not a conflict.
        assert!(parse(&["pick", "--name", "two sum", "--set", "blind75"]).is_ok());
    }

    #[test]
    fn a_bare_pick_and_filtered_picks_parse() {
        assert!(parse(&["pick"]).is_ok());
        assert!(parse(&["pick", "-S", "neetcode150", "-D", "medium", "-t", "array"]).is_ok());
    }

    #[test]
    fn closest_named_problem_prefers_an_exact_match() {
        let problems = fixture();
        assert_eq!(closest_named_problem(&problems, "problem 217"), Some(217));
    }

    #[test]
    fn closest_named_problem_returns_none_for_an_empty_pool() {
        assert_eq!(closest_named_problem(&[], "anything"), None);
    }
}
