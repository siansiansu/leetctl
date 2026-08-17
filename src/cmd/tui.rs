//! tui subcommand - browse problems interactively
use crate::{Cache, Result, filters::ProblemFilters, helper::Difficulty};
use anyhow::anyhow;
use clap::Args;

/// Tui command arguments
#[derive(Args)]
pub struct TuiArgs {
    /// Open filtered to a curated set, e.g. blind75, neetcode150, google
    #[arg(
        short = 'S',
        long,
        value_parser = clap::builder::PossibleValuesParser::new(crate::sets::slugs()),
    )]
    pub set: Option<String>,

    /// Open filtered to a difficulty
    #[arg(short = 'D', long, value_enum, ignore_case = true)]
    pub difficulty: Option<Difficulty>,
}

impl TuiArgs {
    /// `tui` handler
    pub async fn run(&self) -> Result<()> {
        let options = crate::tui::Options {
            rt: tokio::runtime::Handle::current(),
            cache: Cache::new()?,
            filters: ProblemFilters {
                set: self.set.clone(),
                difficulty: self.difficulty,
                ..Default::default()
            },
        };

        // The UI loop is synchronous and owns its thread for the whole session, so it goes on the
        // blocking pool. It talks to the runtime through the handle above.
        tokio::task::spawn_blocking(move || crate::tui::run(options))
            .await
            .map_err(|e| anyhow!("the terminal UI thread died: {e}"))?
    }
}
