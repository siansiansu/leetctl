//! Key handling for the description and help pages
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Model;

pub(super) fn update(m: &mut Model, k: KeyEvent) {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);

    if ctrl && k.code == KeyCode::Char('c') {
        m.quit();
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
}
