//! The set picker, drawn over the table
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use super::{hints_line, pad1};
use crate::tui::Model;

const PICKER_HINTS: [(&str, &str); 3] = [("j/k", "move"), ("enter", "apply"), ("x", "clear set")];

const OUTCOME_HINTS: [(&str, &str); 4] = [
    ("j/k", "scroll"),
    ("t", "test again"),
    ("S", "submit"),
    ("esc", "close"),
];

/// Width of the picker box, wide enough for the longest set name plus its count.
const PICKER_WIDTH: u16 = 52;

pub(super) fn draw_set_picker(m: &Model, f: &mut Frame) {
    let Some(cursor) = m.picker else { return };

    let area = centered(f.area(), PICKER_WIDTH, m.sets.len() as u16 + 4);
    if area.height < 4 {
        return;
    }

    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::Magenta))
        .title(
            Line::from(Span::styled(
                " problem set ",
                Style::new().add_modifier(Modifier::BOLD),
            ))
            .centered(),
        );
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let (body, footer) = split_footer(inner);
    let visible = body.height as usize;
    let first = cursor.saturating_sub(visible.saturating_sub(1));
    let rows: Vec<Line> = m
        .sets
        .iter()
        .enumerate()
        .skip(first)
        .take(visible)
        .map(|(i, choice)| set_row(choice, i == cursor))
        .collect();

    f.render_widget(Paragraph::new(rows), body);
    f.render_widget(Paragraph::new(pad1(hints_line(&PICKER_HINTS))), footer);
}

fn set_row(choice: &crate::tui::SetChoice, selected: bool) -> Line<'static> {
    let marker = if selected { " ❯ " } else { "   " };
    let style = if selected {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        Style::new()
    };

    Line::from(vec![
        Span::styled(marker, Style::new().fg(Color::Magenta)),
        Span::styled(format!("{:<30}", choice.name), style),
        Span::styled(
            format!("{:>5} problems", choice.count),
            Style::new().fg(Color::DarkGray),
        ),
    ])
}

/// The result of a test run or submission, over the description it came from.
pub(super) fn draw_outcome(m: &Model, f: &mut Frame) {
    let Some(outcome) = &m.outcome else { return };

    let area = f.area();
    if area.height < 7 || area.width < 30 {
        return;
    }

    let (label, color) = match (&outcome.kind, outcome.accepted) {
        (crate::cache::Run::Submit, true) => (" accepted ", Color::Green),
        (crate::cache::Run::Submit, false) => (" submission ", Color::Red),
        (crate::cache::Run::Test, _) => (" test run ", Color::Cyan),
    };

    let box_area = centered(
        area,
        area.width.saturating_sub(8),
        area.height.saturating_sub(4),
    );
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(
            Line::from(Span::styled(
                label,
                Style::new().fg(color).add_modifier(Modifier::BOLD),
            ))
            .centered(),
        );
    let inner = block.inner(box_area);
    f.render_widget(Clear, box_area);
    f.render_widget(block, box_area);

    let (body, footer) = split_footer(inner);
    let lines: Vec<Line> = outcome
        .text
        .lines()
        .skip(outcome.scroll)
        .take(body.height as usize)
        .map(|line| pad1(Line::from(Span::raw(line.to_string()))))
        .collect();

    f.render_widget(Paragraph::new(lines), body);
    f.render_widget(Paragraph::new(pad1(hints_line(&OUTCOME_HINTS))), footer);
}

/// Splits an area into everything-but-the-last-row and that last row, so hints sit on the bottom
/// edge of a box instead of trailing whatever content happens to be there.
fn split_footer(area: Rect) -> (Rect, Rect) {
    let body_height = area.height.saturating_sub(1);

    (
        Rect::new(area.x, area.y, area.width, body_height),
        Rect::new(area.x, area.y + body_height, area.width, 1),
    )
}

/// The yes/no before a submission, which is the one thing here that cannot be taken back.
pub(super) fn draw_submit_confirm(m: &Model, f: &mut Frame) {
    let Some(fid) = m.confirm_submit else { return };

    let name = m
        .detail_problem()
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let area = centered(f.area(), 54, 5);
    if area.height < 5 {
        return;
    }

    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::Yellow));
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    f.render_widget(
        Paragraph::new(vec![
            pad1(Line::from(vec![
                Span::raw("Submit "),
                Span::styled(
                    format!("[{fid}] {name}"),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::raw("?"),
            ])),
            pad1(Line::from(vec![
                Span::styled(
                    "y",
                    Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" submit    ", Style::new().fg(Color::DarkGray)),
                Span::styled(
                    "n / esc",
                    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" cancel", Style::new().fg(Color::DarkGray)),
            ])),
        ]),
        inner,
    );
}

/// A box of at most `width` x `height`, centered, clipped to what the terminal has.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);

    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_larger_than_the_terminal_is_clipped_to_it() {
        let area = Rect::new(0, 0, 20, 6);
        assert_eq!(centered(area, 52, 15), Rect::new(0, 0, 20, 6));
    }

    #[test]
    fn a_box_that_fits_is_centered() {
        let area = Rect::new(0, 0, 100, 30);
        assert_eq!(centered(area, 52, 10), Rect::new(24, 10, 52, 10));
    }

    #[test]
    fn a_footer_row_is_taken_from_the_bottom_edge() {
        let (body, footer) = split_footer(Rect::new(2, 3, 20, 5));

        assert_eq!(body, Rect::new(2, 3, 20, 4));
        assert_eq!(footer, Rect::new(2, 7, 20, 1));
    }

    #[test]
    fn a_single_row_area_gives_the_row_to_the_footer() {
        let (body, footer) = split_footer(Rect::new(0, 0, 10, 1));

        assert_eq!(body.height, 0);
        assert_eq!(footer, Rect::new(0, 0, 10, 1));
    }
}
