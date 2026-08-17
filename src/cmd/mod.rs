//! All subcommands in leetctl
//!
//! ```sh
//! SUBCOMMANDS:
//!     data    Manage Cache [aliases: d]
//!     edit    Edit question by id [aliases: e]
//!     exec    Submit solution [aliases: x]
//!     list    List problems [aliases: l]
//!     pick    Pick a problem [aliases: p]
//!     sets    Show bundled problem sets
//!     stat    Show simple chart about submissions [aliases: s]
//!     test    Test a question [aliases: t]
//!     completions    Generate shell completions [aliases: c]
//!     help    Prints this message or the help of the given subcommand(s)
//! ```

use crate::{
    cache::{Cache, models::Problem},
    err::Error,
};

/// The cached problem list, downloading it first on a fresh install.
///
/// Every command that resolves a problem needs this, including the ones that name a problem
/// outright — `Cache::get_question` looks the problem up in sqlite before it fetches anything.
/// Errors when a download leaves the cache still empty, rather than re-entering the command,
/// which would loop forever if the download kept yielding nothing.
pub(crate) async fn populated_problems(cache: &Cache) -> Result<Vec<Problem>, Error> {
    let problems = cache.get_problems()?;
    if !problems.is_empty() {
        return Ok(problems);
    }

    cache.clone().download_problems().await?;
    let problems = cache.get_problems()?;
    if problems.is_empty() {
        return Err(Error::DownloadError("problems".into()));
    }
    Ok(problems)
}

/// `--plan` runs a Python script through the optional `pym` feature. Without that feature the flag
/// parses and then does nothing, which also makes it show up in "no problem matches" messages as a
/// filter that was never applied — so reject it outright instead.
pub(crate) fn ensure_plan_supported(plan: Option<&String>) -> Result<(), Error> {
    #[cfg(not(feature = "pym"))]
    if plan.is_some() {
        return Err(anyhow::anyhow!(
            "--plan needs the optional `pym` feature, which this build does not have. \
             Reinstall with `cargo install leetctl --features pym`."
        )
        .into());
    }
    let _ = plan;
    Ok(())
}

mod completions;
mod data;
mod edit;
mod exec;
mod list;
mod pick;
mod sets;
mod stat;
mod test;

pub use completions::CompletionsArgs;
pub use data::DataArgs;
pub use edit::EditArgs;
pub use exec::ExecArgs;
pub use list::ListArgs;
pub use pick::PickArgs;
pub use sets::SetsArgs;
pub use stat::StatArgs;
pub use test::TestArgs;
