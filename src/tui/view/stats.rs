//! The panel above the table: the counts `leetctl list --stat` prints, a solved-versus-total bar
//! per difficulty, and — when the terminal is wide enough — catalog, deck, and daily lines.
//!
//! The panel is a row of segments laid out left to right. Everything but the first counts column is
//! optional: a segment is drawn only once the width for all of it is there, so a narrow terminal
//! loses whole segments rather than showing half a bar or a clipped number. Whatever width is left
//! over after the last segment goes into the bars, which is why they grow on a wide screen.
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::{difficulty_color, pad1};
use crate::filters::ProgressStats;
use crate::helper::{Difficulty, fit_width};
use crate::tui::Model;

/// Two border rows plus one content row per difficulty.
pub(super) const STATS_PANEL_H: u16 = 5;

/// Content rows. Three, because that is what the three difficulties need.
const ROWS: usize = 3;

/// Gap between two segments.
const GAP: &str = "   ";

/// A count is `Label:` padded to this, then its value right-aligned in [`VALUE_WIDTH`].
const LABEL_WIDTH: usize = 8;
/// Room for a count. Five digits covers the whole catalog several times over.
const VALUE_WIDTH: usize = 5;
const COUNTS_WIDTH: usize = LABEL_WIDTH + VALUE_WIDTH;

/// Narrowest the difficulty bars are drawn, and the widest they grow to on a roomy terminal.
const MIN_BAR_CELLS: usize = 10;
const MAX_BAR_CELLS: usize = 24;
const BAR_FILLED: &str = "█";
const BAR_EMPTY: &str = "░";

/// The extras column's `Label:` field, the least width worth giving it, and the widest its longest
/// row is allowed to get — past that the daily problem's name is truncated rather than pushing
/// everything else off screen.
const EXTRAS_LABEL_WIDTH: usize = 9;
const MIN_EXTRAS_WIDTH: usize = 28;
const MAX_EXTRAS_WIDTH: usize = 44;

/// The mark a truncated name ends in, as in the table.
const ELLIPSIS: &str = "…";

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

    // One column of the inner width goes to pad1's leading space.
    let lines = panel_lines(m, inner.width.saturating_sub(1) as usize);
    f.render_widget(Paragraph::new(lines), inner);
}

/// The panel body, one line per row, already padded.
fn panel_lines(m: &Model, width: usize) -> Vec<Line<'static>> {
    let listed = m.stats();
    let mut rows = counts_segment([
        ("Listed", listed.listed),
        ("Due", m.due_listed()),
        ("Locked", listed.locked),
    ]);
    let plan = Plan::fit(width, COUNTS_WIDTH, extras_width(m));

    if plan.second_counts {
        let second = counts_segment([
            ("Solved", listed.ac),
            ("Tried", listed.notac),
            ("Remain", listed.remain()),
        ]);
        append(&mut rows, second);
    }

    if let Some(cells) = plan.bar_cells {
        append(&mut rows, bars_segment(&listed, cells));
    }

    if let Some(room) = plan.extras {
        append(&mut rows, extras_segment(m, room));
    }

    rows.into_iter()
        .map(|spans| pad1(Line::from(spans)))
        .collect()
}

/// Which optional segments fit, and how wide that leaves the bars.
struct Plan {
    second_counts: bool,
    bar_cells: Option<usize>,
    /// How many columns the extras column gets, once there is room for a useful amount of it.
    extras: Option<usize>,
}

impl Plan {
    /// Segments are taken in priority order — second counts column, bars, extras — and each is
    /// taken only once its minimum fits. The extras column then stretches up to what its longest
    /// row wants, and whatever is still free after that widens the bars.
    fn fit(width: usize, counts_used: usize, extras_wanted: usize) -> Self {
        let mut free = width.saturating_sub(counts_used);

        let second_counts = free >= GAP.len() + COUNTS_WIDTH;
        if second_counts {
            free -= GAP.len() + COUNTS_WIDTH;
        }

        let has_bars = free >= GAP.len() + bar_width(MIN_BAR_CELLS);
        if has_bars {
            free -= GAP.len() + bar_width(MIN_BAR_CELLS);
        }

        let mut extras = None;
        if has_bars && free >= GAP.len() + MIN_EXTRAS_WIDTH {
            let room = (free - GAP.len()).min(extras_wanted);
            free -= GAP.len() + room;
            extras = Some(room);
        }

        Self {
            second_counts,
            bar_cells: has_bars.then(|| MIN_BAR_CELLS + free.min(MAX_BAR_CELLS - MIN_BAR_CELLS)),
            extras,
        }
    }
}

/// Appends a segment's rows to the panel, gap first.
fn append(rows: &mut [Vec<Span<'static>>], segment: Vec<Vec<Span<'static>>>) {
    for (row, spans) in rows.iter_mut().zip(segment) {
        row.push(Span::raw(GAP));
        row.extend(spans);
    }
}

fn counts_segment(counts: [(&str, i32); ROWS]) -> Vec<Vec<Span<'static>>> {
    counts
        .into_iter()
        .map(|(label, value)| {
            vec![
                Span::styled(
                    format!("{:<LABEL_WIDTH$}", format!("{label}:")),
                    Style::new().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{value:>VALUE_WIDTH$}"),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
            ]
        })
        .collect()
}

fn bars_segment(stats: &ProgressStats, cells: usize) -> Vec<Vec<Span<'static>>> {
    [
        (Difficulty::Easy, stats.easy_ac, stats.easy),
        (Difficulty::Medium, stats.medium_ac, stats.medium),
        (Difficulty::Hard, stats.hard_ac, stats.hard),
    ]
    .into_iter()
    .map(|(difficulty, solved, total)| bar_spans(difficulty, solved, total, cells))
    .collect()
}

/// `Easy   ███████░░░  30/50  60%`
fn bar_spans(difficulty: Difficulty, solved: i32, total: i32, cells: usize) -> Vec<Span<'static>> {
    let color = difficulty_color(difficulty.level());
    let filled = match total {
        0 => 0,
        _ => (solved as usize * cells / total as usize).min(cells),
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
        Span::styled(BAR_FILLED.repeat(filled), Style::new().fg(color)),
        Span::styled(
            BAR_EMPTY.repeat(cells - filled),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format!(" {solved:>4}/{total:<4}"),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(format!("{percent:>3}%"), Style::new().fg(color)),
    ]
}

/// Columns one bar row takes: the difficulty, the bar, ` 1234/1234`, and ` 100%`.
fn bar_width(cells: usize) -> usize {
    7 + cells + 10 + 4
}

/// The three extras rows: the whole catalog, the review deck, and today's challenge. These describe
/// everything cached, not the filtered pool — that is what the counts and bars to their left are
/// for, and having both on screen is the point of the column.
fn extras_segment(m: &Model, width: usize) -> Vec<Vec<Span<'static>>> {
    let catalog = crate::filters::progress(&m.all);
    let solved_percent = match catalog.listed {
        0 => 0,
        total => catalog.ac * 100 / total,
    };

    let rows = [
        (
            "Catalog",
            format!(
                "{} · {} solved {}% · ★{}",
                catalog.listed, catalog.ac, solved_percent, catalog.starred
            ),
        ),
        (
            "Deck",
            format!("{} tracked · {} due", m.deck_tracked, m.due.len()),
        ),
        ("Daily", daily_text(m)),
    ];

    rows.into_iter()
        .map(|(label, value)| {
            vec![
                Span::styled(
                    format!("{:<EXTRAS_LABEL_WIDTH$}", format!("{label}:")),
                    Style::new().fg(Color::DarkGray),
                ),
                Span::styled(
                    fit_width(&value, width.saturating_sub(EXTRAS_LABEL_WIDTH), ELLIPSIS),
                    Style::new().fg(Color::Cyan),
                ),
            ]
        })
        .collect()
}

/// `#2418 Sequential Digits`, or a dash until LeetCode has answered.
fn daily_text(m: &Model) -> String {
    let Some(fid) = m.daily_fid else {
        return "—".to_string();
    };

    match m.all.iter().find(|p| p.fid == fid) {
        Some(problem) => format!("#{fid} {}", problem.name),
        None => format!("#{fid}"),
    }
}

/// What the extras column asks for: its longest row, capped so a long problem name cannot crowd out
/// the bars.
fn extras_width(m: &Model) -> usize {
    let rows = extras_segment(m, MAX_EXTRAS_WIDTH);
    rows.iter()
        .map(|spans| spans.iter().map(|s| s.content.chars().count()).sum())
        .max()
        .unwrap_or(0)
        .min(MAX_EXTRAS_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_text(m: &Model, width: usize, row: usize) -> String {
        panel_lines(m, width)[row].to_string()
    }

    #[test]
    fn a_bar_fills_in_proportion_to_the_solved_share() {
        // trace: 3 of 10 solved fills 3 of the 10 cells and reads 30%.
        let line = Line::from(bar_spans(Difficulty::Easy, 3, 10, MIN_BAR_CELLS)).to_string();
        assert!(line.contains("███░░░░░░░"), "{line}");
        assert!(line.contains("3/10"), "{line}");
        assert!(line.contains("30%"), "{line}");
    }

    #[test]
    fn an_empty_difficulty_draws_an_empty_bar_rather_than_dividing_by_zero() {
        let line = Line::from(bar_spans(Difficulty::Hard, 0, 0, MIN_BAR_CELLS)).to_string();
        assert!(line.contains("░░░░░░░░░░"), "{line}");
        assert!(line.contains("  0%"), "{line}");
    }

    #[test]
    fn segments_drop_from_the_right_as_the_panel_narrows() {
        let m = crate::tui::view::test_util::listed_model();

        let narrow = row_text(&m, 20, 0);
        assert!(narrow.contains("Listed"), "{narrow}");
        assert!(!narrow.contains("Solved"), "{narrow}");

        let medium = row_text(&m, 40, 0);
        assert!(medium.contains("Solved"), "{medium}");
        assert!(!medium.contains("Easy"), "{medium}");

        let wide = row_text(&m, 80, 0);
        assert!(wide.contains("Easy"), "{wide}");
        assert!(!wide.contains("Catalog"), "{wide}");

        let widest = row_text(&m, 140, 0);
        assert!(widest.contains("Catalog"), "{widest}");
    }

    #[test]
    fn no_row_grows_past_the_width_it_was_given() {
        let m = crate::tui::view::test_util::listed_model();

        // From the narrowest panel that can hold one counts column, upwards.
        for width in [COUNTS_WIDTH, 20, 33, 47, 80, 100, 140, 200] {
            for row in 0..ROWS {
                let text = row_text(&m, width, row);
                assert!(
                    unicode_width::UnicodeWidthStr::width(text.as_str()) <= width + 1,
                    "row {row} at width {width}: {text:?}"
                );
            }
        }
    }

    #[test]
    fn the_bars_take_the_width_the_other_segments_leave() {
        let m = crate::tui::view::test_util::listed_model();

        let cells = |width: usize| {
            row_text(&m, width, 0)
                .chars()
                .filter(|c| *c == '█' || *c == '░')
                .count()
        };

        // The bars appear at their narrowest, then take whatever the segments to their right
        // leave, up to the cap.
        assert_eq!(cells(63), MIN_BAR_CELLS);
        assert!(cells(80) > MIN_BAR_CELLS, "leftover width went unused");
        assert_eq!(cells(300), MAX_BAR_CELLS, "bars grew past their cap");
    }

    #[test]
    fn the_daily_row_names_todays_problem_once_it_is_known() {
        let mut m = crate::tui::view::test_util::listed_model();
        m.daily_fid = Some(4);

        let daily = row_text(&m, 140, 2);
        assert!(daily.contains("#4 Median of Two Sorted Arrays"), "{daily}");
    }
}
