//! TUI state: the message type, the model, and the update dispatch
mod detail;
mod list;
mod run;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;

use crate::cache::Run;
use crate::cache::models::{Problem, ReviewCard, VerifyResult};
use crate::filters::{ProblemFilters, ProgressStats, progress};
use crate::helper::Difficulty;
use crate::srs::Grade;
use crate::tui::input::Input;
use crate::{Cache, Result};

pub use run::run;

/// Rows the list chrome takes off the terminal height: the top info line, the stats panel's five
/// rows, the table's two border rows, and the hints line. The model needs this to know how many
/// problems fit, and the view lays the same rows out — they have to agree or the cursor scrolls to
/// the wrong place.
pub(crate) const LIST_ROWS_MARGIN: u16 = 9;

/// Rows the description chrome takes: the panel's two border rows and the hints line. Kept
/// separate from the list because that page has no stats panel.
const DETAIL_ROWS_MARGIN: u16 = 3;

/// Columns the description pane loses to the panel border and its one-column padding on each side.
const DETAIL_MARGIN: u16 = 4;

/// Everything that can change the model.
///
/// Loads carry their results rather than being awaited: the UI thread never blocks on I/O.
pub enum Msg {
    Key(KeyEvent),
    Resize(u16, u16),
    ProblemsLoaded(Result<Vec<Problem>>),
    /// The internal problem ids a tag resolved to. `generation` identifies the request, so a slow
    /// answer to a tag the user has since replaced is dropped instead of applied.
    TagIdsLoaded {
        generation: u64,
        res: Result<Vec<String>>,
    },
    /// A problem description, already rendered to plain text.
    QuestionLoaded {
        generation: u64,
        fid: i32,
        res: Result<String>,
    },
    /// Today's challenge, by frontend id.
    DailyLoaded(Result<i32>),
    /// The solution file is on disk and can be opened.
    CodeFileReady {
        fid: i32,
        res: Result<String>,
    },
    /// A finished test run or submission. Boxed: a `VerifyResult` is an order of magnitude larger
    /// than any other message, and every message would otherwise be sized for it.
    ExecDone {
        generation: u64,
        fid: i32,
        kind: Run,
        res: Result<Box<VerifyResult>>,
    },
    /// The frontend ids the review deck says are due. Re-read whenever the deck changes, so the
    /// badge and the footer count cannot drift from what `leetctl review` would print.
    DueLoaded(Result<Vec<i32>>),
    /// A card the user graded from the description page.
    ReviewGraded {
        fid: i32,
        grade: Grade,
        res: Result<ReviewCard>,
    },
    /// Advances the spinner while a run is in flight.
    Tick,
}

/// How often the spinner advances while a test or submission is running.
const SPINNER_INTERVAL: Duration = Duration::from_millis(150);

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A test or submission the TUI is waiting on.
pub(crate) struct ExecInFlight {
    pub(crate) kind: Run,
    pub(crate) fid: i32,
    started: Instant,
    frame: usize,
}

impl ExecInFlight {
    /// `⠹ testing #1 · 3.2s` — proof that something is happening while LeetCode judges.
    pub(crate) fn label(&self) -> String {
        let verb = match self.kind {
            Run::Test => "testing",
            Run::Submit => "submitting",
        };

        format!(
            "{} {verb} #{} · {:.1}s",
            SPINNER_FRAMES[self.frame % SPINNER_FRAMES.len()],
            self.fid,
            self.started.elapsed().as_secs_f32()
        )
    }
}

/// The result of the last run, kept on screen until dismissed.
pub(crate) struct ExecOutcome {
    pub(crate) kind: Run,
    /// Already formatted through `VerifyResult`'s `Display`, which is ANSI-free here because the
    /// TUI turns `colored` off.
    pub(crate) text: String,
    pub(crate) accepted: bool,
    pub(crate) scroll: usize,
}

/// Which screen is up. Prompts and the set picker are overlays on the list, not modes of their own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mode {
    List,
    Detail,
    Help,
}

/// Which one-line prompt is open, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PromptKind {
    /// Free-text search, applied on every keystroke.
    Search,
    /// A tag name, applied when the fetch it triggers comes back.
    Tag,
}

/// An open prompt: what it is for, and what has been typed into it.
pub(crate) struct Prompt {
    pub(crate) kind: PromptKind,
    pub(crate) input: Input,
}

/// One row of the set picker.
pub(crate) struct SetChoice {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) count: usize,
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
    /// Set while an editor owns the terminal, so the input thread stops reading keys the editor
    /// should be getting.
    suspended: Arc<AtomicBool>,
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

    fn load_question(&self, fid: i32, generation: u64) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = cache
                .get_question(fid)
                .await
                .map(|question| question.desc());
            let _ = tx.send(Msg::QuestionLoaded {
                generation,
                fid,
                res,
            });
        });
    }

    fn load_daily(&self) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let _ = tx.send(Msg::DailyLoaded(cache.get_daily_problem_id().await));
        });
    }

    fn prepare_code_file(&self, fid: i32) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = crate::scaffold::ensure_code_file(
                &cache,
                fid,
                None,
                crate::scaffold::Announce::Silent,
            )
            .await;
            let _ = tx.send(Msg::CodeFileReady { fid, res });
        });
    }

    fn exec(&self, fid: i32, kind: Run, generation: u64) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        let run = kind.clone();
        self.rt.spawn(async move {
            let res = cache
                .exec_problem(fid, run.clone(), None)
                .await
                .map(Box::new);
            let _ = tx.send(Msg::ExecDone {
                generation,
                fid,
                kind: run,
                res,
            });
        });
    }

    fn load_due(&self) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = cache.due_review_fids(crate::srs::today());
            let _ = tx.send(Msg::DueLoaded(res));
        });
    }

    fn grade(&self, fid: i32, grade: Grade) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = cache.grade_review(fid, grade, crate::srs::today());
            let _ = tx.send(Msg::ReviewGraded { fid, grade, res });
        });
    }

    /// One delayed tick. The handler asks for the next one while a run is still going, so the chain
    /// stops on its own instead of needing a thread to be told to stop.
    fn schedule_tick(&self) {
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            tokio::time::sleep(SPINNER_INTERVAL).await;
            let _ = tx.send(Msg::Tick);
        });
    }

    fn load_tag_ids(&self, tag: String, generation: u64) {
        let cache = self.cache.clone();
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = cache.get_tagged_questions(&tag).await;
            let _ = tx.send(Msg::TagIdsLoaded { generation, res });
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
    /// The `/` query. Applied on top of `filters`, because it is interactive-only: `leetctl list`
    /// has no equivalent, and its `--keyword` means something narrower.
    pub(crate) search: String,
    /// Only problems that are neither solved nor attempted.
    pub(crate) unsolved_only: bool,
    /// Every frontend id the review deck currently calls due, whether or not the list is narrowed
    /// to them: the badge and the footer count need the whole set. A set rather than a list because
    /// the footer counts it against the whole table on every draw.
    pub(crate) due: HashSet<i32>,
    /// Only problems the deck says are due.
    pub(crate) due_only: bool,
    /// The tag whose members are being shown, once its ids have arrived.
    pub(crate) tag: Option<String>,
    pub(crate) prompt: Option<Prompt>,
    /// Cursor into `sets`, when the set picker is open.
    pub(crate) picker: Option<usize>,
    /// The bundled sets, parsed on first use — the picker is the only thing that needs them.
    pub(crate) sets: Vec<SetChoice>,
    /// Identifies the newest tag request, so slower older ones can be discarded.
    tag_gen: u64,
    pub(crate) mode: Mode,
    /// Which problem the detail page is showing.
    pub(crate) detail_fid: Option<i32>,
    pub(crate) detail_scroll: usize,
    /// Descriptions already fetched, keyed by frontend id. Reopening a problem is instant.
    pub(crate) descriptions: HashMap<i32, String>,
    /// Identifies the newest description request.
    desc_gen: u64,
    /// Today's challenge, once LeetCode has told us. Badges its row in the table.
    pub(crate) daily_fid: Option<i32>,
    /// `D` was pressed before the answer arrived, so jump as soon as it does.
    daily_jump_pending: bool,
    pub(crate) exec: Option<ExecInFlight>,
    pub(crate) outcome: Option<ExecOutcome>,
    /// Identifies the newest run, so a superseded one cannot report over it.
    exec_gen: u64,
    /// Frontend id awaiting a yes/no before it is submitted.
    pub(crate) confirm_submit: Option<i32>,
    /// A solution file the run loop should open in the editor, once it can free the terminal.
    editor_request: Option<String>,
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
    fn new(opts: Options, tx: Sender<Msg>, suspended: Arc<AtomicBool>) -> Self {
        Self {
            backend: Some(Backend {
                rt: opts.rt,
                cache: opts.cache,
                tx,
                suspended,
            }),
            all: Vec::new(),
            filtered: Vec::new(),
            filters: opts.filters,
            search: String::new(),
            unsolved_only: false,
            due: HashSet::new(),
            due_only: false,
            tag: None,
            prompt: None,
            picker: None,
            sets: Vec::new(),
            tag_gen: 0,
            mode: Mode::List,
            detail_fid: None,
            detail_scroll: 0,
            descriptions: HashMap::new(),
            desc_gen: 0,
            daily_fid: None,
            daily_jump_pending: false,
            exec: None,
            outcome: None,
            exec_gen: 0,
            confirm_submit: None,
            editor_request: None,
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
            // Fetched up front so the table can badge today's problem without being asked.
            backend.load_daily();
            backend.load_due();
        }
    }

    pub(crate) fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Key(k) => match self.mode {
                Mode::List => list::update(self, k),
                Mode::Detail => detail::update(self, k),
                Mode::Help => detail::update_help(self, k),
            },
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
            Msg::QuestionLoaded {
                generation,
                fid,
                res,
            } => {
                if generation != self.desc_gen {
                    return;
                }
                match res {
                    Ok(desc) => {
                        self.descriptions.insert(fid, desc);
                        self.status.clear();
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
            Msg::DailyLoaded(res) => match res {
                Ok(fid) => {
                    self.daily_fid = Some(fid);
                    if std::mem::take(&mut self.daily_jump_pending) {
                        self.open_daily();
                    }
                }
                Err(e) => {
                    if std::mem::take(&mut self.daily_jump_pending) {
                        self.status = e.to_string();
                    }
                }
            },
            Msg::CodeFileReady { fid, res } => match res {
                Ok(path) => {
                    self.status.clear();
                    self.editor_request = Some(path);
                }
                Err(e) => self.status = format!("Could not prepare the file for #{fid}: {e}"),
            },
            Msg::ExecDone {
                generation,
                fid,
                kind,
                res,
            } => {
                if generation != self.exec_gen {
                    return;
                }
                self.exec = None;
                match res {
                    Ok(result) => {
                        // A submission grades its own card inside `exec_problem`, so the due set
                        // on screen is stale the moment one finishes.
                        if let (Run::Submit, Some(backend)) = (&kind, &self.backend) {
                            backend.load_due();
                        }
                        let accepted = result.is_accepted();
                        if accepted {
                            // `exec_problem` already wrote this to sqlite; the rows on screen are
                            // copies, so they need telling too.
                            self.mark_solved(fid);
                        }
                        self.status.clear();
                        self.outcome = Some(ExecOutcome {
                            kind,
                            text: result.to_string(),
                            accepted,
                            scroll: 0,
                        });
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
            Msg::DueLoaded(res) => match res {
                Ok(fids) => {
                    self.due = fids.into_iter().collect();
                    // The deck is one of the filters, so a fresh answer re-derives the table.
                    self.apply_filters();
                }
                Err(e) => self.status = e.to_string(),
            },
            Msg::ReviewGraded { fid, grade, res } => match res {
                Ok(card) => {
                    self.status = format!(
                        "#{fid} graded {} — back in {} days",
                        grade.as_str(),
                        card.interval_days
                    );
                    if let Some(backend) = &self.backend {
                        backend.load_due();
                    }
                }
                Err(e) => self.status = e.to_string(),
            },
            Msg::Tick => {
                if let Some(exec) = &mut self.exec {
                    exec.frame += 1;
                    if let Some(backend) = &self.backend {
                        backend.schedule_tick();
                    }
                }
            }
            Msg::TagIdsLoaded { generation, res } => {
                if generation != self.tag_gen {
                    return;
                }
                match res {
                    Ok(ids) => {
                        self.filters.tag_ids = Some(ids);
                        self.apply_filters();
                    }
                    Err(e) => {
                        self.tag = None;
                        self.status = e.to_string();
                    }
                }
            }
        }
    }

    /// Re-derives `filtered` from `all`, keeping the cursor inside it.
    ///
    /// Goes through [`crate::filters::apply`], the same engine `leetctl list` uses, so a footer
    /// count here matches `leetctl list --stat` for the equivalent flags.
    pub(crate) fn apply_filters(&mut self) {
        // `u` is the only thing that sets a query, so deriving it here keeps one source of truth.
        self.filters.query = self.unsolved_only.then(|| "D".to_string());
        // Same for `r`: the deck is held whole in `due`, and the filter is a view of it.
        self.filters.due_fids = self.due_only.then(|| self.due.iter().copied().collect());

        let mut ps = self.all.clone();
        match crate::filters::apply(&mut ps, &self.filters) {
            Ok(()) => {
                ps.retain(|p| crate::tui::search::matches(p, &self.search));
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

    /// Opens a prompt, replacing whatever was open.
    pub(crate) fn open_prompt(&mut self, kind: PromptKind) {
        self.prompt = Some(Prompt {
            kind,
            input: Input::default(),
        });
    }

    /// Difficulty cycles all -> easy -> medium -> hard -> all, so one key covers the whole axis.
    pub(crate) fn cycle_difficulty(&mut self) {
        self.filters.difficulty = match self.filters.difficulty {
            None => Some(Difficulty::Easy),
            Some(Difficulty::Easy) => Some(Difficulty::Medium),
            Some(Difficulty::Medium) => Some(Difficulty::Hard),
            Some(Difficulty::Hard) => None,
        };
        self.apply_filters();
    }

    pub(crate) fn toggle_unsolved(&mut self) {
        self.unsolved_only = !self.unsolved_only;
        self.apply_filters();
    }

    pub(crate) fn toggle_due(&mut self) {
        self.due_only = !self.due_only;
        self.apply_filters();
    }

    /// Grades the problem whose description is open, and says so on the status line once the write
    /// lands. Only from the description page: grading a row you have not looked at is a slip.
    pub(crate) fn grade_detail(&mut self, grade: Grade) {
        let Some(fid) = self.detail_fid else { return };

        self.status = format!("Grading #{fid} {}…", grade.as_str());
        if let Some(backend) = &self.backend {
            backend.grade(fid, grade);
        }
    }

    /// Whether the deck says this problem is due — what the table badges.
    pub(crate) fn is_due(&self, fid: i32) -> bool {
        self.due.contains(&fid)
    }

    /// How many of the problems on screen are due, for the footer.
    pub(crate) fn due_listed(&self) -> i32 {
        self.filtered.iter().filter(|p| self.is_due(p.fid)).count() as i32
    }

    /// Starts a tag fetch, superseding any request still in flight.
    pub(crate) fn request_tag(&mut self, tag: String) {
        if tag.trim().is_empty() {
            return;
        }

        self.tag_gen += 1;
        self.tag = Some(tag.clone());
        self.status = format!("Looking up problems tagged {tag}…");
        if let Some(backend) = &self.backend {
            backend.load_tag_ids(tag, self.tag_gen);
        }
    }

    /// Opens the set picker, parsing the bundled sets the first time it is asked for.
    pub(crate) fn open_picker(&mut self) {
        if self.sets.is_empty() {
            match crate::sets::all() {
                Ok(sets) => {
                    self.sets = sets
                        .into_iter()
                        .map(|set| SetChoice {
                            slug: set.slug,
                            name: set.name,
                            count: set.problems.len(),
                        })
                        .collect();
                }
                Err(e) => {
                    self.status = e.to_string();
                    return;
                }
            }
        }

        // Opens on whichever set is already active, so the picker reads as a current selection.
        let active = self
            .sets
            .iter()
            .position(|choice| Some(&choice.slug) == self.filters.set.as_ref());
        self.picker = Some(active.unwrap_or(0));
    }

    /// Applies the highlighted set and closes the picker.
    pub(crate) fn choose_set(&mut self) {
        if let Some(i) = self.picker.take() {
            self.filters.set = self.sets.get(i).map(|choice| choice.slug.clone());
            self.apply_filters();
        }
    }

    /// Drops every filter, including the interactive ones. The escape hatch from a pile of them.
    pub(crate) fn clear_filters(&mut self) {
        self.filters = ProblemFilters::default();
        self.search.clear();
        self.unsolved_only = false;
        self.due_only = false;
        self.tag = None;
        // Any tag fetch still in flight belongs to a filter that no longer exists.
        self.tag_gen += 1;
        self.apply_filters();
    }

    pub(crate) fn selected(&self) -> Option<&Problem> {
        self.filtered.get(self.cursor)
    }

    /// Opens the description of the problem under the cursor, fetching it unless it is already in
    /// hand. Locked and non-algorithm problems fail in the fetch, and say so on the status line.
    pub(crate) fn open_detail(&mut self) {
        let Some(problem) = self.selected() else {
            return;
        };
        let fid = problem.fid;

        self.mode = Mode::Detail;
        self.detail_fid = Some(fid);
        self.detail_scroll = 0;
        if self.descriptions.contains_key(&fid) {
            return;
        }

        self.desc_gen += 1;
        self.status = format!("Fetching problem {fid}…");
        if let Some(backend) = &self.backend {
            backend.load_question(fid, self.desc_gen);
        }
    }

    /// The description on screen, if it has arrived.
    pub(crate) fn detail_text(&self) -> Option<&str> {
        self.detail_fid
            .and_then(|fid| self.descriptions.get(&fid))
            .map(String::as_str)
    }

    pub(crate) fn detail_problem(&self) -> Option<&Problem> {
        let fid = self.detail_fid?;

        self.filtered
            .iter()
            .chain(self.all.iter())
            .find(|p| p.fid == fid)
    }

    pub(crate) fn back_to_list(&mut self) {
        self.mode = Mode::List;
    }

    pub(crate) fn open_help(&mut self) {
        self.mode = Mode::Help;
    }

    /// Moves the cursor to today's challenge and opens it, asking LeetCode which problem that is if
    /// the answer has not arrived yet.
    pub(crate) fn request_daily(&mut self) {
        match self.daily_fid {
            Some(_) => self.open_daily(),
            None => {
                self.daily_jump_pending = true;
                self.status = "Looking up today's challenge…".to_string();
                if let Some(backend) = &self.backend {
                    backend.load_daily();
                }
            }
        }
    }

    /// Selects the daily problem, dropping the filters that hide it — being sent to a filtered-out
    /// row would look like the key did nothing.
    fn open_daily(&mut self) {
        let Some(fid) = self.daily_fid else { return };

        if !self.filtered.iter().any(|p| p.fid == fid) {
            if !self.all.iter().any(|p| p.fid == fid) {
                self.status = format!(
                    "Today's challenge is problem {fid}, which is not in the cache yet — \
                     run `leetctl data -u`."
                );
                return;
            }
            self.clear_filters();
        }

        if let Some(i) = self.filtered.iter().position(|p| p.fid == fid) {
            self.cursor = i;
            self.open_detail();
        }
    }

    /// Prepares the solution file for the selected problem; the run loop opens the editor once the
    /// file is there, because only it can hand over the terminal.
    pub(crate) fn request_editor(&mut self) {
        let Some(fid) = self.detail_fid.or_else(|| self.selected().map(|p| p.fid)) else {
            return;
        };

        self.status = format!("Preparing the solution file for #{fid}…");
        if let Some(backend) = &self.backend {
            backend.prepare_code_file(fid);
        }
    }

    /// The file the run loop should open, taken so it is opened once.
    pub(crate) fn take_editor_request(&mut self) -> Option<String> {
        self.editor_request.take()
    }

    /// The editor to open `path` with, and the flag that quiets the input thread while it runs.
    pub(crate) fn editor_command(
        &self,
        path: String,
    ) -> Option<(crate::scaffold::EditorCommand, Arc<AtomicBool>)> {
        let backend = self.backend.as_ref()?;
        match crate::scaffold::editor_command(&backend.cache.0.conf, path) {
            Ok(command) => Some((command, backend.suspended.clone())),
            Err(_) => None,
        }
    }

    /// Runs the sample tests, or submits. One at a time: a second run would report over the first.
    pub(crate) fn start_exec(&mut self, kind: Run) {
        let Some(fid) = self.detail_fid else { return };
        if self.exec.is_some() {
            self.status = "A run is already in flight.".to_string();
            return;
        }

        self.outcome = None;
        self.exec_gen += 1;
        self.exec = Some(ExecInFlight {
            kind: kind.clone(),
            fid,
            started: Instant::now(),
            frame: 0,
        });
        if let Some(backend) = &self.backend {
            backend.exec(fid, kind, self.exec_gen);
            backend.schedule_tick();
        }
    }

    /// Submitting is the one irreversible action here, so it asks first.
    pub(crate) fn ask_to_submit(&mut self) {
        if let Some(fid) = self.detail_fid {
            self.confirm_submit = Some(fid);
        }
    }

    pub(crate) fn confirm_submit(&mut self) {
        if self.confirm_submit.take().is_some() {
            self.start_exec(Run::Submit);
        }
    }

    pub(crate) fn cancel_submit(&mut self) {
        self.confirm_submit = None;
    }

    pub(crate) fn dismiss_outcome(&mut self) {
        self.outcome = None;
    }

    /// Scrolls the result pane, clamped to its text.
    pub(crate) fn scroll_outcome(&mut self, delta: isize) {
        let rows = self.rows_height();
        let Some(outcome) = &mut self.outcome else {
            return;
        };

        let last = outcome.text.lines().count().saturating_sub(rows) as isize;
        outcome.scroll = (outcome.scroll as isize + delta).clamp(0, last.max(0)) as usize;
    }

    fn mark_solved(&mut self, fid: i32) {
        for problem in self.all.iter_mut().chain(self.filtered.iter_mut()) {
            if problem.fid == fid {
                problem.status = "ac".to_string();
            }
        }
    }

    /// Whether anything is narrowing the list, which is what makes `esc` worth offering.
    pub(crate) fn has_filters(&self) -> bool {
        self.filters.set.is_some()
            || self.filters.difficulty.is_some()
            || self.filters.keyword.is_some()
            || self.filters.tag_ids.is_some()
            || self.unsolved_only
            || self.due_only
            || !self.search.is_empty()
            || self.tag.is_some()
    }

    pub(crate) fn stats(&self) -> ProgressStats {
        progress(&self.filtered)
    }

    /// The description, wrapped to the pane it is drawn in. The view wraps the same way, so a
    /// scroll offset means the same thing in both.
    pub(crate) fn detail_lines(&self) -> Vec<String> {
        let width = self.width.saturating_sub(DETAIL_MARGIN) as usize;

        self.detail_text()
            .map(|text| crate::tui::wrap::wrap(text, width))
            .unwrap_or_default()
    }

    /// The furthest the description can scroll: enough to bring its last line into view.
    pub(crate) fn detail_last_scroll(&self) -> usize {
        self.detail_lines()
            .len()
            .saturating_sub(self.detail_rows_height())
    }

    /// How many problem rows fit on screen.
    pub(crate) fn rows_height(&self) -> usize {
        self.height.saturating_sub(LIST_ROWS_MARGIN) as usize
    }

    /// How many description lines fit on screen.
    pub(crate) fn detail_rows_height(&self) -> usize {
        self.height.saturating_sub(DETAIL_ROWS_MARGIN) as usize
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
        search: String::new(),
        unsolved_only: false,
        due: HashSet::new(),
        due_only: false,
        tag: None,
        prompt: None,
        picker: None,
        sets: Vec::new(),
        tag_gen: 0,
        mode: Mode::List,
        detail_fid: None,
        detail_scroll: 0,
        descriptions: HashMap::new(),
        desc_gen: 0,
        daily_fid: None,
        daily_jump_pending: false,
        exec: None,
        outcome: None,
        exec_gen: 0,
        confirm_submit: None,
        editor_request: None,
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
