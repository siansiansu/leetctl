//! The problem table: header line, framed table, stats and hints footer
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::stats::{STATS_PANEL_H, draw_stats_panel};
use super::{difficulty_color, hints_line, pad1};
use crate::cache::models::Problem;
use crate::tui::{LIST_ROWS_MARGIN, Model, PromptKind};

/// Fixed columns around the name: lock, status, `[id]`, difficulty, percent, the due dot, and the
/// spaces between them. The name gets whatever is left.
const FIXED_COLUMNS_WIDTH: u16 = 2 + 2 + 7 + 6 + 7 + 2 + 4;

/// Below this width the table cannot show a name, so the whole screen is skipped rather than drawn
/// as a column of punctuation.
const MIN_WIDTH: u16 = FIXED_COLUMNS_WIDTH + 12;

const LIST_HINTS: [(&str, &str); 8] = [
    ("j/k", "move"),
    ("/", "search"),
    ("s", "set"),
    ("d", "difficulty"),
    ("u", "unsolved"),
    ("r", "due"),
    ("enter", "open"),
    ("?", "help"),
];

/// With any filter on, `esc` is the way back out, so it earns a slot.
const LIST_HINTS_FILTERED: [(&str, &str); 9] = [
    ("j/k", "move"),
    ("/", "search"),
    ("s", "set"),
    ("d", "difficulty"),
    ("u", "unsolved"),
    ("r", "due"),
    ("e", "edit"),
    ("esc", "clear filters"),
    ("?", "help"),
];

/// Shown on the footer while a prompt is open, in place of the hints.
const SEARCH_HELP: &str = "space = and, !word = exclude, enter = keep, esc = drop";
const TAG_HELP: &str = "a LeetCode tag slug, e.g. dynamic-programming — enter to look it up";

pub(super) fn draw_list(m: &Model, f: &mut Frame) {
    let area = f.area();
    if area.height < LIST_ROWS_MARGIN + 1 || area.width < MIN_WIDTH {
        return;
    }

    // A prompt takes over the top line rather than inserting a row, so the table below it never
    // shifts and the cursor cannot land off screen.
    f.render_widget(
        Paragraph::new(pad1(top_line(m))),
        Rect::new(area.x, area.y, area.width, 1),
    );

    draw_stats_panel(
        m,
        f,
        Rect::new(area.x, area.y + 1, area.width, STATS_PANEL_H),
    );

    let table_y = area.y + 1 + STATS_PANEL_H;
    let table_height = area.height - STATS_PANEL_H - 2;
    draw_panel(m, f, Rect::new(area.x, table_y, area.width, table_height));

    f.render_widget(
        Paragraph::new(pad1(status_or_hints(m))),
        Rect::new(area.x, area.y + area.height - 1, area.width, 1),
    );
}

fn top_line(m: &Model) -> Line<'static> {
    match &m.prompt {
        Some(prompt) => prompt_line(prompt),
        None => header_line(m),
    }
}

/// The prompt: a prefix and the typed text with the cursor marked. What to type is explained on the
/// footer, which is the one place that guidance lives.
fn prompt_line(prompt: &crate::tui::Prompt) -> Line<'static> {
    let (prefix, color) = match prompt.kind {
        PromptKind::Search => ("/ ", Color::Magenta),
        PromptKind::Tag => ("tag: ", Color::Cyan),
    };

    let (before, under_cursor, after) = prompt.input.render_parts();

    Line::from(vec![
        Span::styled(prefix, Style::new().fg(color).add_modifier(Modifier::BOLD)),
        Span::raw(before.to_string()),
        Span::styled(under_cursor, Style::new().add_modifier(Modifier::REVERSED)),
        Span::raw(after.to_string()),
    ])
}

/// `leetctl <version>` plus a chip per active filter, so the pool on screen is never a mystery.
fn header_line(m: &Model) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("leetctl {}", env!("CARGO_PKG_VERSION")),
        Style::new().add_modifier(Modifier::BOLD),
    )];

    for (label, value) in filter_chips(m) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("{label}:"),
            Style::new().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(
            value,
            Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}

fn filter_chips(m: &Model) -> Vec<(&'static str, String)> {
    let f = &m.filters;
    let mut chips = Vec::new();

    if let Some(set) = &f.set {
        chips.push(("set", set.clone()));
    }
    if let Some(difficulty) = f.difficulty {
        chips.push(("difficulty", difficulty.as_str().to_string()));
    }
    if let Some(tag) = &m.tag {
        chips.push(("tag", tag.clone()));
    }
    if !m.search.is_empty() {
        chips.push(("search", m.search.clone()));
    }
    if m.unsolved_only {
        chips.push(("unsolved", "on".to_string()));
    }
    if m.due_only {
        chips.push(("due", "on".to_string()));
    }
    if let Some(keyword) = &f.keyword {
        chips.push(("name", keyword.clone()));
    }

    chips
}

/// The framed table: rounded border with an embedded ` problems[n] ` title.
fn draw_panel(m: &Model, f: &mut Frame, area: Rect) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("problems", Style::new().add_modifier(Modifier::BOLD)),
        Span::raw("["),
        Span::styled(
            m.filtered.len().to_string(),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw("] "),
    ]);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::Cyan))
        .title(title.centered());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if m.filtered.is_empty() {
        f.render_widget(
            Paragraph::new(pad1(Line::from(Span::styled(
                "No problems match the active filters.",
                Style::new().fg(Color::DarkGray),
            )))),
            inner,
        );
        return;
    }

    let name_width = inner.width.saturating_sub(FIXED_COLUMNS_WIDTH);
    let rows: Vec<Line> = m
        .filtered
        .iter()
        .enumerate()
        .skip(m.row_offset)
        .take(inner.height as usize)
        .map(|(i, p)| {
            problem_row(
                p,
                name_width,
                i == m.cursor,
                m.daily_fid == Some(p.fid),
                m.is_due(p.fid),
            )
        })
        .collect();

    f.render_widget(Paragraph::new(rows), inner);
}

/// One table row. Built from the problem's fields rather than its `Display` impl, which writes ANSI
/// escapes that ratatui would print literally.
fn problem_row(
    p: &Problem,
    name_width: u16,
    selected: bool,
    is_daily: bool,
    is_due: bool,
) -> Line<'static> {
    let status = match p.status.as_str() {
        "ac" => Span::styled(" ✔", Style::new().fg(Color::Green)),
        "notac" => Span::styled(" ✘", Style::new().fg(Color::Red)),
        _ => Span::raw("  "),
    };
    // Today's challenge outranks the lock in the same column: it is the row you came for, and a
    // premium-locked daily still shows as locked in the description.
    let lock = match (is_daily, p.locked) {
        (true, _) => Span::styled("★ ", Style::new().fg(Color::Yellow)),
        (false, true) => Span::raw("🔒"),
        (false, false) => Span::raw("  "),
    };

    let mut spans = vec![
        lock,
        status,
        Span::styled(
            format!(" [{:>4}] ", p.fid),
            Style::new().fg(Color::DarkGray),
        ),
        Span::raw(fit(&p.name, name_width)),
        Span::raw(" "),
        Span::styled(
            format!("{:<6}", display_level(p.level)),
            Style::new().fg(difficulty_color(p.level)),
        ),
        Span::styled(
            format!(" {:>5.2}%", p.percent),
            Style::new().fg(Color::DarkGray),
        ),
        // Trailing rather than sharing the lock column, which today's challenge already contends
        // for: a problem can be locked, daily, and due at once.
        match is_due {
            true => Span::styled(" ●", Style::new().fg(Color::Magenta)),
            false => Span::raw("  "),
        },
    ];

    if selected {
        for span in &mut spans {
            span.style = span.style.add_modifier(Modifier::REVERSED);
        }
    }

    Line::from(spans)
}

fn display_level(level: i32) -> &'static str {
    crate::helper::Difficulty::from_level(level).map_or("?", |d| d.as_str())
}

/// The mark a truncated name ends in. One column, unlike the plain lists' `...`, because a table
/// row here is already fighting the terminal for width.
const ELLIPSIS: &str = "…";

/// Pads or truncates to exactly `width` display columns, so the columns after it line up whatever
/// the name contains.
fn fit(text: &str, width: u16) -> String {
    crate::helper::fit_width(text, width as usize, ELLIPSIS)
}

/// The last line carries whatever needs saying most: an error or in-flight note if there is one,
/// the key hints otherwise.
fn status_or_hints(m: &Model) -> Line<'static> {
    if let Some(prompt) = &m.prompt {
        let help = match prompt.kind {
            PromptKind::Search => SEARCH_HELP,
            PromptKind::Tag => TAG_HELP,
        };
        return Line::from(Span::styled(help, Style::new().fg(Color::DarkGray)));
    }

    if m.status.is_empty() {
        if m.has_filters() {
            return hints_line(&LIST_HINTS_FILTERED);
        }
        return hints_line(&LIST_HINTS);
    }

    Line::from(Span::styled(
        m.status.clone(),
        Style::new().fg(Color::Yellow),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_pads_short_names_to_the_column_width() {
        assert_eq!(fit("Two Sum", 10), "Two Sum   ");
    }

    #[test]
    fn fit_truncates_long_names_with_an_ellipsis() {
        // trace: width 10 leaves 9 columns of text plus the ellipsis.
        assert_eq!(fit("Median of Two Sorted Arrays", 10), "Median of…");
    }

    #[test]
    fn fit_accounts_for_wide_characters() {
        // Each CJK character is two columns wide, so only two fit before the ellipsis.
        assert_eq!(
            unicode_width::UnicodeWidthStr::width(fit("兩數之和", 5).as_str()),
            5
        );
    }
}
