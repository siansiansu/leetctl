//! review subcommand - The spaced-repetition deck
use crate::cache::{Cache, models::Problem, models::ReviewCard};
use crate::err::Error;
use crate::helper::{Digit, fit_width};
use crate::srs::{self, Day, Grade};
use clap::{Args, Subcommand};
use colored::Colorize;
use std::collections::HashMap;

static REVIEW_AFTER_HELP: &str = r#"EXAMPLES:
    leetctl review                 What is due today
    leetctl review --all           The whole deck
    leetctl review next            Open the most overdue problem's description
    leetctl review grade 1 hard    Grade problem 1 "recalled it, painfully"
    leetctl review add 42          Put problem 42 in the deck, due today
    leetctl review drop 42         Take problem 42 out of the deck
    leetctl review stats           Deck breakdown

An accepted `leetctl exec` enrols a problem and grades it `good` on its own, so the deck fills as
you solve. Grade by hand when that is too generous. See docs/srs.md for the schedule.
"#;

/// Display columns each part of a deck row gets.
const DUE_WIDTH: i32 = 8;
const NAME_WIDTH: usize = 44;
const LEVEL_WIDTH: usize = 7;

/// Review command arguments
#[derive(Args)]
#[command(after_help = REVIEW_AFTER_HELP)]
pub struct ReviewArgs {
    #[command(subcommand)]
    pub action: Option<ReviewAction>,

    /// List the whole deck rather than only what is due
    #[arg(short, long)]
    pub all: bool,
}

/// What to do with the deck. Absent means "list what is due", the reason to run this most days.
#[derive(Subcommand)]
pub enum ReviewAction {
    /// Show the most overdue problem's description
    Next,

    /// Grade a problem, adding it to the deck if it is new
    Grade {
        /// Problem id
        #[arg(value_parser = clap::value_parser!(i32))]
        id: i32,

        /// How well the recall went
        #[arg(value_enum)]
        grade: Grade,
    },

    /// Add a problem to the deck, due today
    Add {
        /// Problem id
        #[arg(value_parser = clap::value_parser!(i32))]
        id: i32,
    },

    /// Remove a problem from the deck
    Drop {
        /// Problem id
        #[arg(value_parser = clap::value_parser!(i32))]
        id: i32,
    },

    /// Count the deck by due date and maturity
    Stats,
}

impl ReviewArgs {
    /// `review` handler
    pub async fn run(&self) -> Result<(), Error> {
        trace!("Input review command...");

        let cache = Cache::new()?;
        // Read once, at the top: a command that read the clock per card could schedule half of
        // them against yesterday if it ran across midnight.
        let today = srs::today();

        match self.action {
            None => self.list(&cache, today).await,
            Some(ref action) => {
                if self.all {
                    return Err(anyhow::anyhow!(
                        "--all lists the deck and cannot be combined with a subcommand"
                    )
                    .into());
                }
                action.run(&cache, today).await
            }
        }
    }

    /// The deck as a table: what is due, or all of it under `--all`.
    async fn list(&self, cache: &Cache, today: Day) -> Result<(), Error> {
        let cards = if self.all {
            cache.review_cards()?
        } else {
            cache.due_review_cards(today)?
        };

        if cards.is_empty() {
            println!("{}", self.empty_message());
            return Ok(());
        }

        let problems = super::populated_problems(cache).await?;
        let by_fid: HashMap<i32, &Problem> = problems.iter().map(|p| (p.fid, p)).collect();

        println!("\n{}", header());
        for card in &cards {
            println!("{}", row(card, by_fid.get(&card.fid).copied(), today));
        }
        println!("\n  {} shown\n", cards.len());

        Ok(())
    }

    fn empty_message(&self) -> String {
        if self.all {
            "The deck is empty. Solving a problem with `leetctl exec <id>` adds it, \
             or add one now with `leetctl review add <id>`."
                .to_string()
        } else {
            "Nothing due today. `leetctl review --all` shows the whole deck.".to_string()
        }
    }
}

impl ReviewAction {
    async fn run(&self, cache: &Cache, today: Day) -> Result<(), Error> {
        match *self {
            ReviewAction::Next => next(cache, today).await,
            ReviewAction::Grade { id, grade } => grade_problem(cache, id, grade, today),
            ReviewAction::Add { id } => add(cache, id, today),
            ReviewAction::Drop { id } => drop_problem(cache, id),
            ReviewAction::Stats => stats(cache, today),
        }
    }
}

/// Print the most overdue problem, the way `leetctl pick <id>` prints one.
async fn next(cache: &Cache, today: Day) -> Result<(), Error> {
    let Some(card) = cache.due_review_cards(today)?.into_iter().next() else {
        println!("Nothing due today.");
        return Ok(());
    };

    println!("{}", cache.get_problem(card.fid)?.banner());
    println!("{}", cache.get_question(card.fid).await?.desc());

    Ok(())
}

fn grade_problem(cache: &Cache, id: i32, grade: Grade, today: Day) -> Result<(), Error> {
    // Resolving the problem first turns a typo into "no such problem" rather than a card for an id
    // that does not exist.
    let problem = cache.get_problem(id)?;
    let card = cache.grade_review(id, grade, today)?;

    println!(
        "  [{}] {} graded {} — back in {} days, on {}",
        id,
        problem.name.bold(),
        grade.as_str().bold(),
        card.interval_days,
        srs::date_of(card.due_day)
    );

    Ok(())
}

fn add(cache: &Cache, id: i32, today: Day) -> Result<(), Error> {
    let problem = cache.get_problem(id)?;
    let already_enrolled = cache.review_card(id)?.is_some();
    let card = cache.enroll_review(id, today)?;

    if already_enrolled {
        println!(
            "  [{}] {} is already in the deck — due {}",
            id,
            problem.name.bold(),
            card.due_label(today)
        );
    } else {
        println!("  [{}] {} added, due today", id, problem.name.bold());
    }

    Ok(())
}

fn drop_problem(cache: &Cache, id: i32) -> Result<(), Error> {
    let problem = cache.get_problem(id)?;

    if cache.drop_review(id)? {
        println!("  [{}] {} removed from the deck", id, problem.name.bold());
    } else {
        println!("  [{}] {} was not in the deck", id, problem.name.bold());
    }

    Ok(())
}

fn stats(cache: &Cache, today: Day) -> Result<(), Error> {
    let cards = cache.review_cards()?;
    if cards.is_empty() {
        println!("The deck is empty. `leetctl review add <id>` puts a problem in it.");
        return Ok(());
    }

    let due = cards.iter().filter(|c| c.is_due(today)).count();
    let fresh = cards.iter().filter(|c| c.repetitions == 0).count();
    let mature = cards.iter().filter(|c| c.schedule().is_mature()).count();
    let young = cards
        .iter()
        .filter(|c| c.repetitions > 0 && !c.schedule().is_mature())
        .count();
    let lapses: i32 = cards.iter().map(|c| c.lapses).sum();

    let due_count = if due > 0 {
        due.to_string().bright_red()
    } else {
        due.to_string().normal()
    };

    println!(
        "
  Deck    {}
  Due     {}
  New     {}   never graded
  Young   {}   interval under {} days
  Mature  {}   interval {} days or more
  Lapses  {}   times a card had to be relearned
",
        cards.len(),
        due_count,
        fresh,
        young,
        srs::MATURE_INTERVAL_DAYS,
        mature,
        srs::MATURE_INTERVAL_DAYS,
        lapses,
    );

    Ok(())
}

fn header() -> String {
    // Every column here has to match the one `row` writes, id right-aligned included.
    let head = format!(
        "{} {:>4} {} {} {}  {}",
        "Due".to_string().digit(DUE_WIDTH),
        "Id",
        fit_width("Problem", NAME_WIDTH),
        fit_width("Level", LEVEL_WIDTH),
        "Ease",
        "Reps",
    );
    let rule = "─".repeat(head.len());

    format!("  {}\n  {}", head.bright_black(), rule.bright_black())
}

/// One deck row. A card whose problem is missing from the cache still prints — the schedule is
/// real even when a stale cache cannot name it.
fn row(card: &ReviewCard, problem: Option<&Problem>, today: Day) -> String {
    let name = problem.map_or("(not in the problem cache)", |p| p.name.as_str());
    let level = problem
        .and_then(|p| crate::helper::Difficulty::from_level(p.level))
        .map_or("", |d| d.as_str());

    let due = card.due_label(today).digit(DUE_WIDTH);
    let due = if card.is_due(today) {
        due.bright_red()
    } else {
        due.normal()
    };

    format!(
        "  {} {:>4} {} {} {:.2}  {:>4}",
        due,
        card.fid,
        fit_width(name, NAME_WIDTH),
        fit_width(level, LEVEL_WIDTH),
        card.ease,
        card.repetitions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::models::fixture;
    use crate::srs::Schedule;

    /// `colored` writes escapes only when it thinks a terminal is listening; under `cargo test` it
    /// does not, so these compare plain text.
    fn rendered(card: &ReviewCard, problem: Option<&Problem>, today: Day) -> String {
        row(card, problem, today)
    }

    #[test]
    fn a_row_carries_the_due_label_id_name_and_schedule() {
        let problem = fixture(1, 1, "Two Sum");
        let card = ReviewCard::new(1, Schedule::default().next(Grade::Good), 100);

        let line = rendered(&card, Some(&problem), 100);

        assert!(line.contains("in 4d"), "{line}");
        assert!(line.contains("Two Sum"), "{line}");
        assert!(line.contains("Easy"), "{line}");
        assert!(line.contains("2.50"), "{line}");
    }

    #[test]
    fn a_card_whose_problem_is_not_cached_still_renders() {
        let card = ReviewCard::new(9999, Schedule::default(), 100);

        let line = rendered(&card, None, 100);

        assert!(line.contains("9999"), "{line}");
        assert!(line.contains("not in the problem cache"), "{line}");
    }

    #[test]
    fn an_overdue_card_says_how_late_it_is() {
        let card = ReviewCard::new(1, Schedule::default(), 98);

        assert!(rendered(&card, None, 100).contains("2d late"));
    }
}
