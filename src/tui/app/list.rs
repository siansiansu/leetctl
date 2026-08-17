//! Key handling for the problem table
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Model;

/// Rows a half-page jump moves, when the viewport height is unknown (a resize has not arrived yet).
const FALLBACK_PAGE: usize = 10;

pub(super) fn update(m: &mut Model, k: KeyEvent) {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);

    // `g` is a prefix, so it has to be resolved before anything else claims the key.
    if std::mem::take(&mut m.goto_pending) {
        if k.code == KeyCode::Char('g') {
            m.cursor = 0;
        }
        return;
    }

    match k.code {
        KeyCode::Char('c') if ctrl => m.quit(),
        KeyCode::Char('q') => m.quit(),
        KeyCode::Char('j') | KeyCode::Down => move_by(m, 1),
        KeyCode::Char('k') | KeyCode::Up => move_by(m, -1),
        KeyCode::Char('d') if ctrl => move_by(m, half_page(m)),
        KeyCode::Char('u') if ctrl => move_by(m, -half_page(m)),
        KeyCode::Char('g') => m.goto_pending = true,
        KeyCode::Char('G') => m.cursor = m.filtered.len().saturating_sub(1),
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
}
