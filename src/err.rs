//! Errors in leetctl
use anyhow::anyhow;
use colored::Colorize;

#[cfg(debug_assertions)]
const CONFIG: &str = "~/.leetcode/leetcode.tmp.toml";
#[cfg(not(debug_assertions))]
const CONFIG: &str = "~/.leetcode/leetcode_tmp.toml";

/// Leetcode result.
pub type Result<T> = std::result::Result<T, Error>;

/// Leetcode cli errors
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Nothing matched")]
    MatchError,
    #[error("Download {0} failed, please try again")]
    DownloadError(String),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    HeaderName(#[from] reqwest::header::InvalidHeaderName),
    #[error(transparent)]
    HeaderValue(#[from] reqwest::header::InvalidHeaderValue),
    #[error(
        "Your leetcode cookies seems expired, \
         {} \
         Either you can handwrite your `LEETCODE_SESSION` and `csrf` into `leetcode.toml`, \
         more info please checkout this: \
         https://github.com/siansiansu/leetcli/blob/main/docs/cookies.md",
        "please make sure you have logined in leetcode.com with chrome. ".yellow().bold()
    )]
    CookieError,
    #[error(
        "Your leetcode account lacks a premium subscription, which the given problem requires.\n \
         If this looks like a mistake, please open a new issue at: {}",
        "https://github.com/siansiansu/leetcli/".underline()
    )]
    PremiumError,
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error(
        "json from response parse failed, please open a new issue at: {}.",
        "https://github.com/siansiansu/leetcli/".underline()
    )]
    NoneError,
    #[error(
        "Parse config file failed, \
         leetctl has just generated a new leetcode.toml at {}, \
         the current one at {} seems missing some keys, Please compare \
         the new file and add the missing keys.\n",
        CONFIG,
        "~/.leetcode/leetcode.toml".yellow().bold().underline(),
    )]
    Config(#[from] toml::de::Error),
    #[error("Maybe you not login on the Chrome, you can login and retry")]
    ChromeNotLogin,
    #[error("Unknown problem set `{0}`. Run `leetctl sets` to see what is available.")]
    UnknownSet(String),
    // Deliberately not a `#[from] toml::de::Error`: that maps to `Config`, whose message blames
    // the user's leetcode.toml. This TOML ships inside the binary, so a failure here is our bug.
    #[error(
        "Bundled problem set `{slug}` failed to parse. This is a packaging bug in leetctl, \
         not a problem with your config — please report it at {}\n{source}",
        "https://github.com/siansiansu/leetcli/issues/new".underline()
    )]
    SetData {
        slug: String,
        source: toml::de::Error,
    },
    #[error(
        "No problem matches {0}.\n\
         Relax the filters, or run `leetctl data -u` to refresh the problem cache."
    )]
    NoProblemsMatch(String),
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Toml(#[from] toml::ser::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[cfg(feature = "pym")]
    #[error(transparent)]
    Pyo3(#[from] pyo3::PyErr),
}

impl std::convert::From<diesel::result::Error> for Error {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::NotFound => Error::Anyhow(anyhow!(
                "NotFound, you may update cache with `leetctl data -u`, and try it again\r\n"
            )),
            _ => Error::Anyhow(anyhow!("{err}")),
        }
    }
}
