//! Key handling for the description and help pages
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Model;
use crate::cache::Run;

pub(super) fn update(m: &mut Model, k: KeyEvent) {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);

    if ctrl && k.code == KeyCode::Char('c') {
        m.quit();
        return;
    }

    // A pending submission is a question; nothing else happens until it is answered.
    if m.confirm_submit.is_some() {
        match k.code {
            KeyCode::Char('y') => m.confirm_submit(),
            _ => m.cancel_submit(),
        }
        return;
    }

    if m.outcome.is_some() {
        update_outcome(m, k, ctrl);
        return;
    }

    if std::mem::take(&mut m.goto_pending) {
        if k.code == KeyCode::Char('g') {
            m.detail_scroll = 0;
        }
        return;
    }

    match k.code {
        KeyCode::Esc | KeyCode::Char('q') => m.back_to_list(),
        KeyCode::Char('j') | KeyCode::Down => scroll(m, 1),
        KeyCode::Char('k') | KeyCode::Up => scroll(m, -1),
        KeyCode::Char('d') if ctrl => scroll(m, half_page(m)),
        KeyCode::Char('u') if ctrl => scroll(m, -half_page(m)),
        KeyCode::Char('g') => m.goto_pending = true,
        KeyCode::Char('G') => m.detail_scroll = m.detail_last_scroll(),
        KeyCode::Char('?') => m.open_help(),
        KeyCode::Char('e') => m.request_editor(),
        KeyCode::Char('t') => m.start_exec(Run::Test),
        KeyCode::Char('S') => m.ask_to_submit(),
        _ => {}
    }
}

/// Keys while a result is on screen: scroll it, run again, or dismiss it.
fn update_outcome(m: &mut Model, k: KeyEvent, ctrl: bool) {
    match k.code {
        KeyCode::Esc | KeyCode::Char('q') => m.dismiss_outcome(),
        KeyCode::Char('j') | KeyCode::Down => m.scroll_outcome(1),
        KeyCode::Char('k') | KeyCode::Up => m.scroll_outcome(-1),
        KeyCode::Char('d') if ctrl => m.scroll_outcome(half_page(m)),
        KeyCode::Char('u') if ctrl => m.scroll_outcome(-half_page(m)),
        KeyCode::Char('e') => m.request_editor(),
        KeyCode::Char('t') => m.start_exec(Run::Test),
        KeyCode::Char('S') => m.ask_to_submit(),
        _ => {}
    }
}

/// Help is a dead end on purpose: every key that is not "leave" is ignored, so nothing surprising
/// happens while reading it.
pub(super) fn update_help(m: &mut Model, k: KeyEvent) {
    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
        m.quit();
        return;
    }

    if matches!(
        k.code,
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')
    ) {
        // Back to whichever screen asked for help — the list, unless a description is open.
        m.mode = match m.detail_fid {
            Some(_) => super::Mode::Detail,
            None => super::Mode::List,
        };
    }
}

fn half_page(m: &Model) -> isize {
    (m.rows_height() / 2).max(1) as isize
}

fn scroll(m: &mut Model, delta: isize) {
    let last = m.detail_last_scroll() as isize;
    m.detail_scroll = (m.detail_scroll as isize + delta).clamp(0, last) as usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::Mode;
    use crate::tui::test_model;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    /// A detail page showing a description of `lines` single-word lines.
    fn detail_model(lines: usize) -> Model {
        let mut m = test_model();
        m.all = vec![crate::cache::models::fixture(1, 1, "Two Sum")];
        m.apply_filters();
        m.descriptions.insert(
            1,
            (0..lines)
                .map(|i| format!("line{i}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        m.open_detail();
        m
    }

    #[test]
    fn enter_opens_the_description_and_esc_goes_back() {
        let mut m = detail_model(3);
        assert_eq!(m.mode, Mode::Detail);
        assert_eq!(m.detail_fid, Some(1));

        update(&mut m, key(KeyCode::Esc));
        assert_eq!(m.mode, Mode::List);
    }

    #[test]
    fn scrolling_stops_at_both_ends() {
        // trace: height 30 - ROWS_MARGIN 5 = 25 visible lines, so 100 lines scroll to 75.
        let mut m = detail_model(100);

        update(&mut m, key(KeyCode::Char('k')));
        assert_eq!(m.detail_scroll, 0, "already at the top");

        update(&mut m, key(KeyCode::Char('G')));
        assert_eq!(m.detail_scroll, 75);

        update(&mut m, key(KeyCode::Char('j')));
        assert_eq!(m.detail_scroll, 75, "already at the bottom");

        update(&mut m, ctrl('u'));
        assert_eq!(m.detail_scroll, 63);

        update(&mut m, key(KeyCode::Char('g')));
        update(&mut m, key(KeyCode::Char('g')));
        assert_eq!(m.detail_scroll, 0);
    }

    #[test]
    fn a_description_shorter_than_the_pane_does_not_scroll() {
        let mut m = detail_model(4);

        update(&mut m, key(KeyCode::Char('G')));
        assert_eq!(m.detail_scroll, 0);
    }

    #[test]
    fn reopening_a_problem_uses_the_cached_description() {
        let mut m = detail_model(3);
        update(&mut m, key(KeyCode::Esc));
        m.status = "stale".into();

        m.open_detail();

        assert!(
            m.status == "stale",
            "a cached description asks for nothing, so the status line is left alone"
        );
        assert!(m.detail_text().is_some());
    }

    #[test]
    fn help_returns_to_whichever_screen_opened_it() {
        let mut m = detail_model(3);

        update(&mut m, key(KeyCode::Char('?')));
        assert_eq!(m.mode, Mode::Help);
        update_help(&mut m, key(KeyCode::Esc));
        assert_eq!(m.mode, Mode::Detail, "opened from the description");

        m.back_to_list();
        m.detail_fid = None;
        m.open_help();
        update_help(&mut m, key(KeyCode::Char('q')));
        assert_eq!(m.mode, Mode::List, "opened from the list");
    }

    #[test]
    fn help_ignores_everything_that_is_not_a_way_out() {
        let mut m = detail_model(100);
        m.open_help();

        update_help(&mut m, key(KeyCode::Char('j')));
        update_help(&mut m, key(KeyCode::Char('G')));

        assert_eq!(m.mode, Mode::Help);
        assert_eq!(m.detail_scroll, 0);
    }

    #[test]
    fn ctrl_c_quits_from_the_description_and_from_help() {
        let mut m = detail_model(3);
        update(&mut m, ctrl('c'));
        assert!(m.should_quit());

        let mut m = detail_model(3);
        m.open_help();
        update_help(&mut m, ctrl('c'));
        assert!(m.should_quit());
    }

    fn verify_result(json: &str) -> crate::cache::models::VerifyResult {
        serde_json::from_str(json).expect("fixture should deserialize")
    }

    /// A submission LeetCode accepted, captured from a real response (the same payload
    /// `tests/de.rs` decodes). `result_type` is `#[serde(skip)]` and defaults to `Submit`.
    fn accepted() -> crate::cache::models::VerifyResult {
        verify_result(
            r#"{"status_code": 10, "lang": "rust", "run_success": true, "status_runtime": "0 ms", "memory": 2300000, "question_id": "1", "elapsed_time": 0, "compare_result": "11111111111111111111111111111", "code_output": "", "std_output": "", "last_testcase": "", "task_finish_time": 1578193674018, "total_correct": 29, "total_testcases": 29, "runtime_percentile": 100, "status_memory": "2.3 MB", "memory_percentile": 100, "pretty_lang": "Rust", "submission_id": "291285717", "status_msg": "Accepted", "state": "SUCCESS"}"#,
        )
    }

    /// A real submission that failed one of the 29 cases.
    fn rejected() -> crate::cache::models::VerifyResult {
        verify_result(
            r#"{"status_code": 11, "lang": "rust", "run_success": true, "status_runtime": "4 ms", "memory": 2716000, "question_id": "1", "elapsed_time": 0, "compare_result": "11111111111111111111111111011", "code_output": "", "std_output": "", "last_testcase": "[1, 2, 3]", "task_finish_time": 1578590021187, "total_correct": 28, "total_testcases": 29, "runtime_percentile": 76.9231, "status_memory": "2.7 MB", "memory_percentile": 100, "pretty_lang": "Rust", "submission_id": "292701790", "status_msg": "Failed", "state": "SUCCESS"}"#,
        )
    }

    fn exec_done(
        generation: u64,
        fid: i32,
        kind: Run,
        result: crate::cache::models::VerifyResult,
    ) -> super::super::Msg {
        super::super::Msg::ExecDone {
            generation,
            fid,
            kind,
            res: Ok(Box::new(result)),
        }
    }

    #[test]
    fn t_starts_a_test_run_and_the_footer_reports_it() {
        let mut m = detail_model(3);

        update(&mut m, key(KeyCode::Char('t')));

        let exec = m.exec.as_ref().expect("a run is in flight");
        assert!(matches!(exec.kind, Run::Test));
        assert_eq!(exec.fid, 1);
        assert!(exec.label().contains("testing #1"), "{}", exec.label());
    }

    #[test]
    fn a_second_run_is_refused_rather_than_reporting_over_the_first() {
        let mut m = detail_model(3);
        update(&mut m, key(KeyCode::Char('t')));

        update(&mut m, key(KeyCode::Char('t')));

        assert!(m.status.contains("already in flight"), "{}", m.status);
    }

    #[test]
    fn a_result_from_a_superseded_run_is_dropped() {
        let mut m = detail_model(3);
        update(&mut m, key(KeyCode::Char('t')));

        m.handle(exec_done(0, 1, Run::Test, accepted()));

        assert!(m.outcome.is_none(), "the stale answer is ignored");
        assert!(m.exec.is_some(), "and the current run is still waiting");
    }

    #[test]
    fn submitting_asks_first_and_can_be_cancelled() {
        let mut m = detail_model(3);

        update(&mut m, key(KeyCode::Char('S')));
        assert_eq!(m.confirm_submit, Some(1));
        assert!(m.exec.is_none(), "nothing is sent before the answer");

        update(&mut m, key(KeyCode::Char('n')));
        assert!(m.confirm_submit.is_none());
        assert!(m.exec.is_none());

        update(&mut m, key(KeyCode::Char('S')));
        update(&mut m, key(KeyCode::Char('y')));
        assert!(matches!(
            m.exec.as_ref().map(|e| e.kind.clone()),
            Some(Run::Submit)
        ));
    }

    #[test]
    fn an_accepted_submission_marks_the_row_solved_immediately() {
        let mut m = detail_model(3);
        update(&mut m, key(KeyCode::Char('S')));
        update(&mut m, key(KeyCode::Char('y')));

        m.handle(exec_done(1, 1, Run::Submit, accepted()));

        let outcome = m.outcome.as_ref().expect("a result is on screen");
        assert!(outcome.accepted);
        // trace: the real payload reports 0 ms at the 100th percentile against 2.3 MB.
        assert!(outcome.text.contains("Success"), "{}", outcome.text);
        assert!(outcome.text.contains("0 ms"), "{}", outcome.text);
        assert!(outcome.text.contains("100%"), "{}", outcome.text);
        assert!(outcome.text.contains("2.3 MB"), "{}", outcome.text);
        assert_eq!(m.all[0].status, "ac", "the cached row is stale otherwise");
        assert_eq!(m.filtered[0].status, "ac");
        assert!(m.exec.is_none(), "the spinner stops");
    }

    #[test]
    fn a_rejected_submission_reports_the_failing_case_and_leaves_the_row_alone() {
        let mut m = detail_model(3);
        update(&mut m, key(KeyCode::Char('S')));
        update(&mut m, key(KeyCode::Char('y')));

        m.handle(exec_done(1, 1, Run::Submit, rejected()));

        let outcome = m.outcome.as_ref().expect("a result is on screen");
        assert!(!outcome.accepted);
        // trace: 28 of 29 cases passed, and the payload names the case that did not.
        assert!(outcome.text.contains("28"), "{}", outcome.text);
        assert!(outcome.text.contains("29"), "{}", outcome.text);
        assert!(outcome.text.contains("[1, 2, 3]"), "{}", outcome.text);
        assert_eq!(m.all[0].status, "", "an unsolved problem stays unsolved");
    }

    #[test]
    fn a_failed_run_shows_the_error_and_leaves_no_result_pane() {
        let mut m = detail_model(3);
        update(&mut m, key(KeyCode::Char('t')));

        m.handle(super::super::Msg::ExecDone {
            generation: 1,
            fid: 1,
            kind: Run::Test,
            res: Err(crate::Error::CookieError),
        });

        assert!(m.outcome.is_none());
        assert!(m.status.contains("cookies"), "{}", m.status);
    }

    #[test]
    fn the_result_pane_scrolls_and_dismisses() {
        let mut m = detail_model(3);
        m.outcome = Some(super::super::ExecOutcome {
            kind: Run::Test,
            text: (0..60)
                .map(|i| format!("out{i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            accepted: false,
            scroll: 0,
        });

        // trace: 60 lines against 25 visible rows scrolls to 35.
        update(&mut m, key(KeyCode::Char('G')));
        assert_eq!(
            m.outcome.as_ref().unwrap().scroll,
            0,
            "G is not a result-pane key"
        );

        update(&mut m, key(KeyCode::Char('j')));
        assert_eq!(m.outcome.as_ref().unwrap().scroll, 1);
        update(&mut m, ctrl('d'));
        assert_eq!(m.outcome.as_ref().unwrap().scroll, 13);
        update(&mut m, ctrl('u'));
        assert_eq!(m.outcome.as_ref().unwrap().scroll, 1);

        update(&mut m, key(KeyCode::Esc));
        assert!(m.outcome.is_none());
        assert_eq!(m.mode, Mode::Detail, "esc closed the result, not the page");
    }

    #[test]
    fn a_result_on_screen_does_not_swallow_a_re_run() {
        let mut m = detail_model(3);
        m.outcome = Some(super::super::ExecOutcome {
            kind: Run::Test,
            text: "out".into(),
            accepted: false,
            scroll: 0,
        });

        update(&mut m, key(KeyCode::Char('t')));

        assert!(m.exec.is_some());
        assert!(
            m.outcome.is_none(),
            "the old result gives way to the new run"
        );
    }

    #[test]
    fn the_spinner_advances_only_while_a_run_is_in_flight() {
        let mut m = detail_model(3);
        update(&mut m, key(KeyCode::Char('t')));
        let first = m.exec.as_ref().unwrap().label();

        m.handle(super::super::Msg::Tick);
        assert_ne!(m.exec.as_ref().unwrap().label()[..3], first[..3]);

        m.handle(exec_done(1, 1, Run::Test, accepted()));
        m.handle(super::super::Msg::Tick);
        assert!(m.exec.is_none(), "a tick after the run does nothing");
    }

    #[test]
    fn e_asks_for_the_solution_file_and_the_loop_gets_a_path_to_open() {
        let mut m = detail_model(3);

        update(&mut m, key(KeyCode::Char('e')));
        assert!(m.status.contains("Preparing"), "{}", m.status);

        m.handle(super::super::Msg::CodeFileReady {
            fid: 1,
            res: Ok("/tmp/1.two-sum.py".into()),
        });

        assert_eq!(
            m.take_editor_request().as_deref(),
            Some("/tmp/1.two-sum.py")
        );
        assert!(
            m.take_editor_request().is_none(),
            "opened once, not every frame"
        );
        assert!(m.status.is_empty());
    }

    #[test]
    fn a_file_that_could_not_be_prepared_says_so_and_opens_nothing() {
        let mut m = detail_model(3);

        m.handle(super::super::Msg::CodeFileReady {
            fid: 1,
            res: Err(crate::Error::NoneError),
        });

        assert!(m.take_editor_request().is_none());
        assert!(m.status.contains("#1"), "{}", m.status);
    }
}
