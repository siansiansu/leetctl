//! The terminal frontend: a filterable problem table
//!
//! Architecture: one `Msg` channel drained by the UI thread ([`app::run`]). Key events, cache
//! reads, and network calls all arrive as messages; rendering happens after each batch. See
//! `docs/tui.md` for the threading contract.

mod app;
mod view;

pub use app::{Options, run};

pub(crate) use app::{Model, ROWS_MARGIN};

#[cfg(test)]
pub(crate) use app::test_model;
