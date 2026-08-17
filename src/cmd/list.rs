//! list subcommand - List leetcode problems
use crate::{
    cache::Cache,
    err::Error,
    filters::ProblemFilters,
    helper::{Difficulty, Digit},
};
use clap::Args;

static CATEGORY_HELP: &str = r#"Filter problems by category name
[algorithms, database, shell, concurrency]
"#;

static QUERY_HELP: &str = r#"Filter questions by conditions:
Uppercase means negative
e = easy     E = m+h
m = medium   M = e+h
h = hard     H = e+m
d = done     D = not done
l = locked   L = not locked
s = starred  S = not starred"#;

static LIST_AFTER_HELP: &str = r#"EXAMPLES:
    leetctl list                   List all questions
    leetctl list array             List questions that has "array" in name, and this is letter non-sensitive
    leetctl list -c database       List questions that in database category
    leetctl list -q eD             List questions that with easy level and not done
    leetctl list -t linked-list    List questions that under tag "linked-list"
    leetctl list -r 50 100         List questions that has id in between 50 and 100
    leetctl list -S blind75        List questions in the Blind 75 set
    leetctl list -S blind75 -s     Show progress through the Blind 75 set
    leetctl list -S google -D hard List hard questions in the Google set

Run `leetctl sets` for the available --set values.
"#;

/// List command arguments
#[derive(Args)]
#[command(after_help = LIST_AFTER_HELP)]
pub struct ListArgs {
    /// Keyword in select query
    pub keyword: Option<String>,

    /// Filter problems by category name
    #[arg(short, long, help = CATEGORY_HELP)]
    pub category: Option<String>,

    /// Invoking python scripts to filter questions
    #[arg(short, long)]
    pub plan: Option<String>,

    /// Filter questions by conditions
    #[arg(short, long, help = QUERY_HELP)]
    pub query: Option<String>,

    /// Filter questions by id range
    #[arg(short, long, num_args = 2.., value_parser = clap::value_parser!(i32))]
    pub range: Vec<i32>,

    /// Show statistics of listed problems
    #[arg(short, long)]
    pub stat: bool,

    /// Filter questions by tag
    #[arg(short, long)]
    pub tag: Option<String>,

    /// Filter questions by curated set, e.g. blind75, neetcode150, google
    #[arg(
        short = 'S',
        long,
        value_parser = clap::builder::PossibleValuesParser::new(crate::sets::slugs()),
    )]
    pub set: Option<String>,

    /// Filter questions by difficulty
    #[arg(short = 'D', long, value_enum, ignore_case = true)]
    pub difficulty: Option<Difficulty>,
}

impl ListArgs {
    /// `list` command handler
    pub async fn run(&self) -> Result<(), Error> {
        trace!("Input list command...");
        super::ensure_plan_supported(self.plan.as_ref())?;

        let cache = Cache::new()?;
        let mut ps = super::populated_problems(&cache).await?;

        // Python plans stay here rather than in `ProblemFilters`: they are a CLI-only feature, and
        // squashing by id intersects, so running this before or after the shared filters is the
        // same set either way.
        #[cfg(feature = "pym")]
        {
            if let Some(ref plan) = self.plan {
                let ids = crate::pym::exec(plan)?;
                crate::helper::squash(&mut ps, ids)?;
            }
        }

        let tag_ids = match self.tag {
            Some(ref tag) => Some(cache.get_tagged_questions(tag).await?),
            None => None,
        };

        crate::filters::apply(
            &mut ps,
            &ProblemFilters {
                keyword: self.keyword.clone(),
                category: self.category.clone(),
                query: self.query.clone(),
                set: self.set.clone(),
                difficulty: self.difficulty,
                range: (self.range.len() >= 2).then(|| (self.range[0], self.range[1])),
                tag_ids,
            },
        )?;

        // output problem lines sorted by [problem number] like
        // [ 1 ] Two Sum
        // [ 2 ] Add Two Numbers
        let out: Vec<String> = ps.iter().map(ToString::to_string).collect();
        println!("{}", out.join("\n"));

        // one more thing, filter stat
        if self.stat {
            let stats = crate::filters::progress(&ps);
            println!(
                "
        Listed: {}     Locked: {}     Starred: {}
        Accept: {}     Not-Ac: {}     Remain:  {}
        Easy  : {}     Medium: {}     Hard:    {}",
                stats.listed.digit(4),
                stats.locked.digit(4),
                stats.starred.digit(4),
                stats.ac.digit(4),
                stats.notac.digit(4),
                stats.remain().digit(4),
                stats.easy.digit(4),
                stats.medium.digit(4),
                stats.hard.digit(4),
            );
        }
        Ok(())
    }
}
