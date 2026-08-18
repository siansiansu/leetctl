//! Key handling for the problem table
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{Model, PromptKind};

/// Rows a half-page jump moves, when the viewport height is unknown (a resize has not arrived yet).
const FALLBACK_PAGE: usize = 10;

pub(super) fn update(m: &mut Model, k: KeyEvent) {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);

    // Ctrl-c quits from anywhere, including out of a prompt.
    if ctrl && k.code == KeyCode::Char('c') {
        m.quit();
        return;
    }

    if m.prompt.is_some() {
        update_prompt(m, k);
        return;
    }

    if m.picker.is_some() {
        update_picker(m, k);
        return;
    }

    // `g` is a prefix, so it has to be resolved before anything else claims the key.
    if std::mem::take(&mut m.goto_pending) {
        if k.code == KeyCode::Char('g') {
            m.cursor = 0;
        }
        return;
    }

    match k.code {
        KeyCode::Char('q') => m.quit(),
        KeyCode::Char('j') | KeyCode::Down => move_by(m, 1),
        KeyCode::Char('k') | KeyCode::Up => move_by(m, -1),
        KeyCode::Char('d') if ctrl => move_by(m, half_page(m)),
        KeyCode::Char('u') if ctrl => move_by(m, -half_page(m)),
        KeyCode::Char('g') => m.goto_pending = true,
        KeyCode::Char('G') => m.cursor = m.filtered.len().saturating_sub(1),
        KeyCode::Char('/') => m.open_prompt(PromptKind::Search),
        KeyCode::Char('t') => m.open_prompt(PromptKind::Tag),
        KeyCode::Char('s') => m.open_picker(),
        KeyCode::Char('d') => m.cycle_difficulty(),
        KeyCode::Char('u') => m.toggle_unsolved(),
        KeyCode::Char('r') => m.toggle_due(),
        KeyCode::Enter => m.open_detail(),
        KeyCode::Char('D') => m.request_daily(),
        KeyCode::Char('?') => m.open_help(),
        KeyCode::Char('e') => m.request_editor(),
        KeyCode::Esc if m.has_filters() => m.clear_filters(),
        _ => {}
    }
}

/// Keys while a prompt is open. Text goes to the input; enter commits, escape abandons.
///
/// A search applies as it is typed so the table answers immediately; a tag cannot, because
/// resolving it is a network call.
fn update_prompt(m: &mut Model, k: KeyEvent) {
    let Some(prompt) = &mut m.prompt else { return };
    let kind = prompt.kind;

    if prompt.input.handle(&k) {
        if kind == PromptKind::Search {
            m.search = m
                .prompt
                .as_ref()
                .map(|p| p.input.value().to_string())
                .unwrap_or_default();
            m.apply_filters();
        }
        return;
    }

    match k.code {
        KeyCode::Enter => {
            let typed = prompt.input.value().to_string();
            m.prompt = None;
            if kind == PromptKind::Tag {
                m.request_tag(typed);
            }
        }
        KeyCode::Esc => {
            m.prompt = None;
            // An abandoned search leaves nothing behind; a tag was never applied to begin with.
            if kind == PromptKind::Search && !m.search.is_empty() {
                m.search.clear();
                m.apply_filters();
            }
        }
        _ => {}
    }
}

/// Keys while the set picker is open.
fn update_picker(m: &mut Model, k: KeyEvent) {
    let Some(cursor) = m.picker else { return };
    let last = m.sets.len().saturating_sub(1);

    match k.code {
        KeyCode::Char('j') | KeyCode::Down => m.picker = Some((cursor + 1).min(last)),
        KeyCode::Char('k') | KeyCode::Up => m.picker = Some(cursor.saturating_sub(1)),
        KeyCode::Enter => m.choose_set(),
        // The way back to every problem, without leaving the picker to press esc.
        KeyCode::Char('x') => {
            m.picker = None;
            m.filters.set = None;
            m.apply_filters();
        }
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('s') => m.picker = None,
        _ => {}
    }
}

fn half_page(m: &Model) -> isize {
    let rows = m.rows_height();
    if rows == 0 {
        return FALLBACK_PAGE as isize;
    }

    (rows / 2).max(1) as isize
}

/// Moves the cursor, stopping at either end rather than wrapping — wrapping loses your place in a
/// list this long.
fn move_by(m: &mut Model, delta: isize) {
    if m.filtered.is_empty() {
        m.cursor = 0;
        return;
    }

    let last = m.filtered.len() - 1;
    m.cursor = (m.cursor as isize + delta).clamp(0, last as isize) as usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_model;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn model_with(rows: usize) -> Model {
        let mut m = test_model();
        m.all = (1..=rows as i32)
            .map(|fid| crate::cache::models::fixture(fid, 1, &format!("problem {fid}")))
            .collect();
        m.apply_filters();
        m
    }

    #[test]
    fn j_and_k_move_one_row_and_stop_at_the_ends() {
        let mut m = model_with(3);

        update(&mut m, key(KeyCode::Char('k')));
        assert_eq!(m.cursor, 0, "k at the top stays put");

        update(&mut m, key(KeyCode::Char('j')));
        update(&mut m, key(KeyCode::Char('j')));
        update(&mut m, key(KeyCode::Char('j')));
        assert_eq!(m.cursor, 2, "j at the bottom stays put");
    }

    #[test]
    fn gg_and_shift_g_jump_to_the_ends() {
        let mut m = model_with(50);

        update(&mut m, key(KeyCode::Char('G')));
        assert_eq!(m.cursor, 49);

        update(&mut m, key(KeyCode::Char('g')));
        assert!(m.goto_pending, "a lone g waits for the second one");
        update(&mut m, key(KeyCode::Char('g')));
        assert_eq!(m.cursor, 0);
        assert!(!m.goto_pending);
    }

    #[test]
    fn an_abandoned_g_prefix_swallows_the_next_key_only() {
        let mut m = model_with(50);

        update(&mut m, key(KeyCode::Char('g')));
        update(&mut m, key(KeyCode::Char('x')));
        assert_eq!(m.cursor, 0);
        assert!(!m.goto_pending);

        update(&mut m, key(KeyCode::Char('j')));
        assert_eq!(m.cursor, 1, "the key after that is handled normally");
    }

    #[test]
    fn ctrl_d_and_ctrl_u_move_half_a_screen() {
        // trace: height 30 - ROWS_MARGIN 5 = 25 rows, half of which is 12.
        let mut m = model_with(50);

        update(&mut m, ctrl('d'));
        assert_eq!(m.cursor, 12);
        update(&mut m, ctrl('u'));
        assert_eq!(m.cursor, 0);
    }

    #[test]
    fn navigation_on_an_empty_table_is_a_no_op() {
        let mut m = test_model();

        update(&mut m, key(KeyCode::Char('j')));
        update(&mut m, key(KeyCode::Char('G')));
        update(&mut m, ctrl('d'));
        assert_eq!(m.cursor, 0);
    }

    #[test]
    fn q_and_ctrl_c_quit() {
        let mut m = model_with(3);
        update(&mut m, key(KeyCode::Char('q')));
        assert!(m.should_quit());

        let mut m = model_with(3);
        update(&mut m, ctrl('c'));
        assert!(m.should_quit());
    }

    #[test]
    fn the_viewport_follows_the_cursor_down_and_back_up() {
        // trace: 25 visible rows, so row 24 is the last one on screen without scrolling.
        let mut m = model_with(50);

        m.cursor = 24;
        m.ensure_cursor_visible();
        assert_eq!(m.row_offset, 0);

        m.cursor = 25;
        m.ensure_cursor_visible();
        assert_eq!(m.row_offset, 1);

        m.cursor = 49;
        m.ensure_cursor_visible();
        assert_eq!(m.row_offset, 25, "the last screenful is flush with the end");

        m.cursor = 0;
        m.ensure_cursor_visible();
        assert_eq!(m.row_offset, 0);
    }

    #[test]
    fn a_table_shorter_than_the_screen_never_scrolls() {
        let mut m = model_with(5);

        m.cursor = 4;
        m.ensure_cursor_visible();
        assert_eq!(m.row_offset, 0);
    }

    fn tag_ids_loaded(generation: u64, ids: &[&str]) -> super::super::Msg {
        super::super::Msg::TagIdsLoaded {
            generation,
            res: Ok(ids.iter().map(|id| id.to_string()).collect()),
        }
    }

    fn typed(m: &mut Model, text: &str) {
        for c in text.chars() {
            update(m, key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn slash_opens_a_search_that_filters_as_it_is_typed() {
        let mut m = test_model();
        m.all = vec![
            crate::cache::models::fixture(1, 1, "Two Sum"),
            crate::cache::models::fixture(15, 2, "3Sum"),
            crate::cache::models::fixture(20, 1, "Valid Parentheses"),
        ];
        m.apply_filters();

        update(&mut m, key(KeyCode::Char('/')));
        typed(&mut m, "sum");
        assert_eq!(m.filtered.len(), 2, "both sums match");

        typed(&mut m, " !3");
        assert_eq!(m.filtered.len(), 1);
        assert_eq!(m.filtered[0].fid, 1);

        // enter keeps the search and closes the prompt
        update(&mut m, key(KeyCode::Enter));
        assert!(m.prompt.is_none());
        assert_eq!(m.search, "sum !3");
        assert_eq!(m.filtered.len(), 1);
    }

    #[test]
    fn escape_abandons_a_search_and_restores_the_table() {
        let mut m = model_with(20);

        update(&mut m, key(KeyCode::Char('/')));
        typed(&mut m, "problem 1");
        assert!(m.filtered.len() < 20);

        update(&mut m, key(KeyCode::Esc));
        assert!(m.prompt.is_none());
        assert_eq!(m.search, "");
        assert_eq!(m.filtered.len(), 20);
    }

    #[test]
    fn r_narrows_to_the_review_deck_and_back() {
        let mut m = model_with(4);
        m.due = vec![2, 3];

        update(&mut m, key(KeyCode::Char('r')));
        assert!(m.due_only);
        assert_eq!(
            m.filtered.iter().map(|p| p.fid).collect::<Vec<_>>(),
            vec![2, 3]
        );

        update(&mut m, key(KeyCode::Char('r')));
        assert!(!m.due_only);
        assert_eq!(m.filtered.len(), 4);
    }

    #[test]
    fn an_empty_deck_narrows_to_nothing_rather_than_to_everything() {
        let mut m = model_with(4);

        update(&mut m, key(KeyCode::Char('r')));

        assert!(m.filtered.is_empty(), "nothing is due, so nothing shows");
    }

    #[test]
    fn esc_drops_the_due_filter_with_the_rest() {
        let mut m = model_with(4);
        m.due = vec![2];
        update(&mut m, key(KeyCode::Char('r')));
        assert!(m.has_filters());

        update(&mut m, key(KeyCode::Esc));

        assert!(!m.due_only);
        assert_eq!(m.filtered.len(), 4);
    }

    #[test]
    fn keys_typed_into_a_prompt_are_not_commands() {
        let mut m = model_with(20);

        update(&mut m, key(KeyCode::Char('/')));
        typed(&mut m, "qjd");
        assert!(!m.should_quit(), "q is text here");
        assert_eq!(m.cursor, 0, "j is text here");
        assert!(m.filters.difficulty.is_none(), "d is text here");
    }

    #[test]
    fn d_cycles_difficulty_all_the_way_around() {
        let mut m = model_with(3);

        for expected in [
            Some(crate::helper::Difficulty::Easy),
            Some(crate::helper::Difficulty::Medium),
            Some(crate::helper::Difficulty::Hard),
            None,
        ] {
            update(&mut m, key(KeyCode::Char('d')));
            assert_eq!(m.filters.difficulty, expected);
        }
    }

    #[test]
    fn u_toggles_unsolved_only() {
        let mut m = test_model();
        m.all = vec![
            crate::cache::models::fixture(1, 1, "Two Sum"),
            crate::cache::models::fixture(2, 2, "Add Two Numbers"),
        ];
        m.all[0].status = "ac".into();
        m.apply_filters();

        update(&mut m, key(KeyCode::Char('u')));
        assert!(m.unsolved_only);
        assert_eq!(m.filtered.len(), 1);
        assert_eq!(m.filtered[0].fid, 2);

        update(&mut m, key(KeyCode::Char('u')));
        assert!(!m.unsolved_only);
        assert_eq!(m.filtered.len(), 2);
    }

    #[test]
    fn the_set_picker_applies_a_set_and_can_clear_it() {
        let mut m = test_model();
        m.all = vec![
            crate::cache::models::fixture(1, 1, "Two Sum"),
            crate::cache::models::fixture(4, 3, "Median of Two Sorted Arrays"),
        ];
        m.apply_filters();

        update(&mut m, key(KeyCode::Char('s')));
        assert_eq!(m.picker, Some(0), "opens on the first set");
        assert!(!m.sets.is_empty(), "the bundled sets are parsed on open");
        assert_eq!(m.sets[0].slug, "blind75", "registry order is preserved");

        update(&mut m, key(KeyCode::Enter));
        assert!(m.picker.is_none());
        assert_eq!(m.filters.set.as_deref(), Some("blind75"));
        // trace: of the two fixtures only fid 1 is in blind75.
        assert_eq!(m.filtered.len(), 1);

        update(&mut m, key(KeyCode::Char('s')));
        update(&mut m, key(KeyCode::Char('x')));
        assert!(m.filters.set.is_none());
        assert_eq!(m.filtered.len(), 2);
    }

    #[test]
    fn the_picker_reopens_on_the_active_set() {
        let mut m = model_with(3);
        m.filters.set = Some("neetcode150".into());

        update(&mut m, key(KeyCode::Char('s')));
        let cursor = m.picker.expect("picker open");
        assert_eq!(m.sets[cursor].slug, "neetcode150");
    }

    #[test]
    fn escape_clears_every_filter_at_once() {
        let mut m = model_with(20);
        m.filters.difficulty = Some(crate::helper::Difficulty::Easy);
        m.search = "problem".into();
        m.unsolved_only = true;
        m.tag = Some("array".into());
        m.filters.tag_ids = Some(vec!["1".into()]);
        m.apply_filters();

        update(&mut m, key(KeyCode::Esc));

        assert!(!m.has_filters());
        assert_eq!(m.filtered.len(), 20);
    }

    #[test]
    fn a_tag_answer_from_a_superseded_request_is_dropped() {
        let mut m = model_with(20);

        m.request_tag("array".into());
        let stale = 1;
        m.request_tag("tree".into());

        m.handle(tag_ids_loaded(stale, &["1"]));
        assert!(m.filters.tag_ids.is_none(), "the old answer is ignored");

        m.handle(tag_ids_loaded(2, &["1", "2"]));
        assert_eq!(m.filtered.len(), 2, "the current answer applies");
    }

    #[test]
    fn clearing_filters_also_discards_a_tag_fetch_in_flight() {
        let mut m = model_with(20);
        m.request_tag("array".into());

        update(&mut m, key(KeyCode::Esc));
        m.handle(tag_ids_loaded(1, &["1"]));

        assert!(m.filters.tag_ids.is_none());
        assert_eq!(m.filtered.len(), 20);
    }
}
