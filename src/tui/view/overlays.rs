//! The set picker, drawn over the table
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use super::{hints_line, pad1};
use crate::tui::Model;

const PICKER_HINTS: [(&str, &str); 3] = [("j/k", "move"), ("enter", "apply"), ("x", "clear set")];

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

    let visible = inner.height.saturating_sub(1) as usize;
    let first = cursor.saturating_sub(visible.saturating_sub(1));
    let mut lines: Vec<Line> = m
        .sets
        .iter()
        .enumerate()
        .skip(first)
        .take(visible)
        .map(|(i, choice)| set_row(choice, i == cursor))
        .collect();
    lines.push(pad1(hints_line(&PICKER_HINTS)));

    f.render_widget(Paragraph::new(lines), inner);
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
}
