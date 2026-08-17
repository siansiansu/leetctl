//! TUI state: the message type, the model, and the update dispatch
mod list;
mod run;

use std::sync::mpsc::Sender;

use crossterm::event::KeyEvent;

use crate::cache::models::Problem;
use crate::filters::{ProblemFilters, ProgressStats, progress};
use crate::{Cache, Result};

pub use run::run;

/// Rows the chrome takes off the terminal height: the top info line, the panel's two border rows,
/// and the two footer lines. The model needs this to know how many problems fit, and the view
/// lays the same rows out — they have to agree or the cursor scrolls to the wrong place.
pub(crate) const ROWS_MARGIN: u16 = 5;

/// Everything that can change the model.
///
/// Loads carry their results rather than being awaited: the UI thread never blocks on I/O.
pub enum Msg {
    Key(KeyEvent),
    Resize(u16, u16),
    ProblemsLoaded(Result<Vec<Problem>>),
}

/// What the TUI needs from the rest of the program.
pub struct Options {
    /// Handle to the runtime that owns the network and cache work.
    pub rt: tokio::runtime::Handle,
    pub cache: Cache,
    /// Filters the command line opened the browser with.
    pub filters: ProblemFilters,
}

/// The cache and the runtime, absent in tests.
///
/// Every call clones the cache and the sender into a spawned task, so the UI thread hands work off
/// and returns to drawing immediately.
struct Backend {
    rt: tokio::runtime::Handle,
    cache: Cache,
    tx: Sender<Msg>,
}

impl Backend {
    fn load_problems(&self) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = crate::cmd::populated_problems(&cache).await;
            let _ = tx.send(Msg::ProblemsLoaded(res));
        });
    }
}

/// The whole frontend's state.
///
/// `all` is what the cache returned; `filtered` is what the table shows. Both are kept so a filter
/// can be narrowed or dropped without going back to sqlite.
pub struct Model {
    backend: Option<Backend>,
    pub(crate) all: Vec<Problem>,
    pub(crate) filtered: Vec<Problem>,
    pub(crate) filters: ProblemFilters,
    pub(crate) cursor: usize,
    /// First visible row, moved only to keep the cursor on screen.
    pub(crate) row_offset: usize,
    /// `g` was pressed and is waiting for the second `g` of `gg`.
    pub(crate) goto_pending: bool,
    pub(crate) loading: bool,
    /// One line of feedback under the table: what failed, or what is in flight.
    pub(crate) status: String,
    pub(crate) width: u16,
    pub(crate) height: u16,
    quit: bool,
}

impl Model {
    fn new(opts: Options, tx: Sender<Msg>) -> Self {
        Self {
            backend: Some(Backend {
                rt: opts.rt,
                cache: opts.cache,
                tx,
            }),
            all: Vec::new(),
            filtered: Vec::new(),
            filters: opts.filters,
            cursor: 0,
            row_offset: 0,
            goto_pending: false,
            loading: true,
            status: String::new(),
            width: 0,
            height: 0,
            quit: false,
        }
    }

    /// Kicks off the first load. Separate from [`Model::new`] so tests can build a model without
    /// touching the cache.
    fn init(&mut self) {
        if let Some(backend) = &self.backend {
            backend.load_problems();
        }
    }

    pub(crate) fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Key(k) => list::update(self, k),
            Msg::Resize(w, h) => {
                self.width = w;
                self.height = h;
            }
            Msg::ProblemsLoaded(res) => {
                self.loading = false;
                match res {
                    Ok(problems) => {
                        self.all = problems;
                        self.apply_filters();
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
        }
    }

    /// Re-derives `filtered` from `all`, keeping the cursor inside it.
    ///
    /// Goes through [`crate::filters::apply`], the same engine `leetctl list` uses, so a footer
    /// count here matches `leetctl list --stat` for the equivalent flags.
    pub(crate) fn apply_filters(&mut self) {
        let mut ps = self.all.clone();
        match crate::filters::apply(&mut ps, &self.filters) {
            Ok(()) => {
                self.filtered = ps;
                self.status.clear();
            }
            Err(e) => {
                self.filtered = Vec::new();
                self.status = e.to_string();
            }
        }
        self.clamp_cursor();
    }

    pub(crate) fn stats(&self) -> ProgressStats {
        progress(&self.filtered)
    }

    /// How many problem rows fit on screen.
    pub(crate) fn rows_height(&self) -> usize {
        self.height.saturating_sub(ROWS_MARGIN) as usize
    }

    fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    /// Scrolls just enough to keep the cursor visible. Called before every draw, because a resize
    /// can shrink the viewport out from under it.
    pub(crate) fn ensure_cursor_visible(&mut self) {
        let rows = self.rows_height();
        if rows == 0 {
            return;
        }

        if self.cursor < self.row_offset {
            self.row_offset = self.cursor;
        } else if self.cursor >= self.row_offset + rows {
            self.row_offset = self.cursor + 1 - rows;
        }

        let last_offset = self.filtered.len().saturating_sub(rows);
        self.row_offset = self.row_offset.min(last_offset);
    }

    pub(crate) fn quit(&mut self) {
        self.quit = true;
    }

    fn should_quit(&self) -> bool {
        self.quit
    }
}

/// A model with no cache or runtime behind it, for tests that drive `handle` and render.
#[cfg(test)]
pub(crate) fn test_model() -> Model {
    Model {
        backend: None,
        all: Vec::new(),
        filtered: Vec::new(),
        filters: ProblemFilters::default(),
        cursor: 0,
        row_offset: 0,
        goto_pending: false,
        loading: false,
        status: String::new(),
        width: 100,
        height: 30,
        quit: false,
    }
}
