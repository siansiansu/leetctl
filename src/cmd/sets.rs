//! sets subcommand - Show the curated problem sets bundled with leetctl
use crate::err::Error;
use clap::Args;
use colored::Colorize;

static SETS_AFTER_HELP: &str = r#"EXAMPLES:
    leetctl sets                        Show every bundled set
    leetctl list --set blind75          List the problems in a set
    leetctl list --set blind75 --stat   Show progress through a set
    leetctl pick --set blind75          Pick a random problem from a set
"#;

/// Sets command arguments
#[derive(Args)]
#[command(after_help = SETS_AFTER_HELP)]
pub struct SetsArgs {
    /// Also show where each set came from and when its data was collected
    #[arg(short, long)]
    pub sources: bool,
}

impl SetsArgs {
    /// `sets` command handler
    pub fn run(&self) -> Result<(), Error> {
        let sets = crate::sets::all()?;
        let slug_width = sets.iter().map(|s| s.slug.len()).max().unwrap_or(0);
        let name_width = sets.iter().map(|s| s.name.len()).max().unwrap_or(0);

        println!(
            "{:slug_width$}  {:name_width$}  {:>8}",
            "SLUG".bold(),
            "NAME".bold(),
            "PROBLEMS".bold(),
        );

        for set in &sets {
            println!(
                "{:slug_width$}  {:name_width$}  {:>8}",
                set.slug.green(),
                set.name,
                set.problems.len(),
            );
            if self.sources {
                println!(
                    "{:slug_width$}  {}",
                    "",
                    format!(
                        "{}  ({}, data as of {})",
                        set.source_url, set.source_license, set.source_as_of
                    )
                    .dimmed(),
                );
            }
        }

        if !self.sources {
            println!(
                "\n{}",
                "Run with --sources for provenance, or see docs/sets.md.".dimmed()
            );
        }

        Ok(())
    }
}
