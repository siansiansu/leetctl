//! Edit command
use crate::{Error, Result};
use clap::Args;

/// Edit command arguments
#[derive(Args)]
#[command(group = clap::ArgGroup::new("question-id").args(&["id", "daily"]).required(true))]
pub struct EditArgs {
    /// Question id
    #[arg(value_parser = clap::value_parser!(i32))]
    pub id: Option<i32>,

    /// Edit today's daily challenge
    #[arg(short = 'd', long)]
    pub daily: bool,

    /// Edit with specific language
    #[arg(short, long)]
    pub lang: Option<String>,
}

impl EditArgs {
    /// `edit` handler
    pub async fn run(&self) -> Result<()> {
        use crate::Cache;

        let cache = Cache::new()?;

        let daily_id = if self.daily {
            Some(cache.get_daily_problem_id().await?)
        } else {
            None
        };

        let id = self.id.or(daily_id).ok_or(Error::NoneError)?;
        let path = crate::scaffold::ensure_code_file(&cache, id, self.lang.clone()).await?;

        // Re-read the config rather than threading it out of the scaffold: a `--lang` override only
        // touches `code.lang`, which the editor invocation does not read.
        let conf = cache.0.conf;
        let editor = crate::scaffold::editor_command(&conf, path)?;

        std::process::Command::new(editor.program)
            .envs(editor.envs)
            .args(editor.args)
            .status()?;
        Ok(())
    }
}
