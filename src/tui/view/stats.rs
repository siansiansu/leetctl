//! The panel above the table: the same counts the CLI's `--stat` prints, plus a solved-versus-total
//! bar per difficulty. Everything it shows describes the filtered pool, like the table below it.
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::{difficulty_color, pad1};
use crate::helper::Difficulty;
use crate::tui::Model;

/// Two border rows plus one content row per difficulty.
pub(super) const STATS_PANEL_H: u16 = 5;

/// Content rows, which is what the counts get to spread over.
const ROWS: usize = 3;

const BAR_WIDTH: i32 = 10;
const BAR_FILLED: &str = "█";
const BAR_EMPTY: &str = "░";

/// Gap between the counts columns, and between the counts and the bars.
const COLUMN_GAP: &str = "   ";

/// Widest count label, so the numbers of both columns line up.
const LABEL_WIDTH: usize = 6;
/// Room for a count. Five digits covers the whole problem set several times over.
const VALUE_WIDTH: usize = 5;

pub(super) fn draw_stats_panel(m: &Model, f: &mut Frame, area: Rect) {
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::DarkGray))
        .title(Line::from(Span::styled(
            " stats ",
            Style::new().add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(area);
    f.render_widget(block, area);

    f.render_widget(Paragraph::new(panel_lines(m, inner.width)), inner);
}

/// The panel body: two columns of counts, and the difficulty bars when they still fit. The bars go
/// first when width runs out, because a clipped bar reads as a wrong ratio.
fn panel_lines(m: &Model, width: u16) -> Vec<Line<'static>> {
    let stats = m.stats();
    let counts = [
        [("Listed", stats.listed), ("Solved", stats.ac)],
        [("Due", m.due_listed()), ("Tried", stats.notac)],
        [("Locked", stats.locked), ("Remain", stats.remain())],
    ];
    let bars = [
        (Difficulty::Easy, stats.easy_ac, stats.easy),
        (Difficulty::Medium, stats.medium_ac, stats.medium),
        (Difficulty::Hard, stats.hard_ac, stats.hard),
    ];

    let counts_width = 2 * (LABEL_WIDTH + 2 + VALUE_WIDTH) + COLUMN_GAP.len();
    let bars_fit = width as usize >= counts_width + COLUMN_GAP.len() + bar_width();

    (0..ROWS)
        .map(|row| {
            let mut spans = Vec::new();
            for (i, (label, value)) in counts[row].iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(COLUMN_GAP));
                }
                spans.extend(count_spans(label, *value));
            }

            if bars_fit {
                let (difficulty, solved, total) = bars[row];
                spans.push(Span::raw(COLUMN_GAP));
                spans.extend(bar_spans(difficulty, solved, total));
            }

            pad1(Line::from(spans))
        })
        .collect()
}

fn count_spans(label: &str, value: i32) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!("{:<width$}", format!("{label}:"), width = LABEL_WIDTH + 2),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{value:>VALUE_WIDTH$}"),
            Style::new().add_modifier(Modifier::BOLD),
        ),
    ]
}

/// `Easy   ███████░░░  30/50  60%`
fn bar_spans(difficulty: Difficulty, solved: i32, total: i32) -> Vec<Span<'static>> {
    let color = difficulty_color(difficulty.level());
    let filled = match total {
        0 => 0,
        _ => (solved * BAR_WIDTH / total).clamp(0, BAR_WIDTH),
    };
    let percent = match total {
        0 => 0,
        _ => solved * 100 / total,
    };

    vec![
        Span::styled(
            format!("{:<7}", difficulty.as_str()),
            Style::new().fg(color),
        ),
        Span::styled(BAR_FILLED.repeat(filled as usize), Style::new().fg(color)),
        Span::styled(
            BAR_EMPTY.repeat((BAR_WIDTH - filled) as usize),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format!(" {solved:>4}/{total:<4}"),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(format!("{percent:>3}%"), Style::new().fg(color)),
    ]
}

/// Columns one bar row takes: the label, the bar itself, ` 1234/1234 `, and ` 100%`.
fn bar_width() -> usize {
    7 + BAR_WIDTH as usize + 1 + 4 + 1 + 4 + 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_fills_in_proportion_to_the_solved_share() {
        // trace: 3 of 10 solved fills 3 of the 10 cells and reads 30%.
        let line = Line::from(bar_spans(Difficulty::Easy, 3, 10)).to_string();
        assert!(line.contains("███░░░░░░░"), "{line}");
        assert!(line.contains("3/10"), "{line}");
        assert!(line.contains("30%"), "{line}");
    }

    #[test]
    fn an_empty_difficulty_draws_an_empty_bar_rather_than_dividing_by_zero() {
        let line = Line::from(bar_spans(Difficulty::Hard, 0, 0)).to_string();
        assert!(line.contains("░░░░░░░░░░"), "{line}");
        assert!(line.contains("  0%"), "{line}");
    }

    #[test]
    fn the_bars_drop_out_when_the_panel_is_too_narrow_for_them() {
        let m = crate::tui::view::test_util::listed_model();

        let wide = Line::from_iter(panel_lines(&m, 120)[0].spans.clone()).to_string();
        assert!(wide.contains("Easy"), "{wide}");

        let narrow = Line::from_iter(panel_lines(&m, 40)[0].spans.clone()).to_string();
        assert!(narrow.contains("Listed"), "{narrow}");
        assert!(!narrow.contains("Easy"), "{narrow}");
    }
}
