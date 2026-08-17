//! The problem table: header line, framed table, stats and hints footer
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::{difficulty_color, hints_line, pad1};
use crate::cache::models::Problem;
use crate::tui::{Model, PromptKind, ROWS_MARGIN};

/// Fixed columns around the name: lock, status, `[id]`, difficulty, percent, and the spaces
/// between them. The name gets whatever is left.
const FIXED_COLUMNS_WIDTH: u16 = 2 + 2 + 7 + 6 + 7 + 4;

/// Below this width the table cannot show a name, so the whole screen is skipped rather than drawn
/// as a column of punctuation.
const MIN_WIDTH: u16 = FIXED_COLUMNS_WIDTH + 12;

const LIST_HINTS: [(&str, &str); 7] = [
    ("j/k", "move"),
    ("/", "search"),
    ("s", "set"),
    ("d", "difficulty"),
    ("u", "unsolved"),
    ("enter", "open"),
    ("?", "help"),
];

/// With any filter on, `esc` is the way back out, so it earns a slot.
const LIST_HINTS_FILTERED: [(&str, &str); 7] = [
    ("j/k", "move"),
    ("/", "search"),
    ("s", "set"),
    ("d", "difficulty"),
    ("u", "unsolved"),
    ("esc", "clear filters"),
    ("?", "help"),
];

/// Shown on the footer while a prompt is open, in place of the hints.
const SEARCH_HELP: &str = "space = and, !word = exclude, enter = keep, esc = drop";
const TAG_HELP: &str = "a LeetCode tag slug, e.g. dynamic-programming — enter to look it up";

pub(super) fn draw_list(m: &Model, f: &mut Frame) {
    let area = f.area();
    if area.height < ROWS_MARGIN + 1 || area.width < MIN_WIDTH {
        return;
    }

    // A prompt takes over the top line rather than inserting a row, so the table below it never
    // shifts and the cursor cannot land off screen.
    f.render_widget(
        Paragraph::new(pad1(top_line(m))),
        Rect::new(area.x, area.y, area.width, 1),
    );

    let panel_height = area.height - 3;
    draw_panel(
        m,
        f,
        Rect::new(area.x, area.y + 1, area.width, panel_height),
    );

    let footer_y = area.y + area.height - 2;
    f.render_widget(
        // One column goes to pad1's leading space.
        Paragraph::new(pad1(stats_line(m, area.width.saturating_sub(1)))),
        Rect::new(area.x, footer_y, area.width, 1),
    );
    f.render_widget(
        Paragraph::new(pad1(status_or_hints(m))),
        Rect::new(area.x, footer_y + 1, area.width, 1),
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
        .map(|(i, p)| problem_row(p, name_width, i == m.cursor, m.daily_fid == Some(p.fid)))
        .collect();

    f.render_widget(Paragraph::new(rows), inner);
}

/// One table row. Built from the problem's fields rather than its `Display` impl, which writes ANSI
/// escapes that ratatui would print literally.
fn problem_row(p: &Problem, name_width: u16, selected: bool, is_daily: bool) -> Line<'static> {
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

/// Pads or truncates to exactly `width` display columns, so the columns after it line up whatever
/// the name contains.
fn fit(text: &str, width: u16) -> String {
    let width = width as usize;
    if UnicodeWidthStr::width(text) <= width {
        return format!("{text}{}", " ".repeat(width - UnicodeWidthStr::width(text)));
    }

    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let char_width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + char_width > width.saturating_sub(1) {
            break;
        }
        out.push(c);
        used += char_width;
    }
    out.push('…');

    format!("{out}{}", " ".repeat(width - used - 1))
}

/// Gap between two stat groups.
const STATS_GAP: &str = "   ";

/// Progress through whatever is currently listed — the same numbers `leetctl list --stat` prints.
///
/// Groups are dropped from the right when they do not fit, most important first: a clipped count is
/// worse than a missing one, because a half-drawn number still reads as a number.
fn stats_line(m: &Model, width: u16) -> Line<'static> {
    let s = m.stats();
    let groups = [
        ("Listed", s.listed, Color::Cyan),
        ("Solved", s.ac, Color::Green),
        ("Tried", s.notac, Color::Yellow),
        ("Remain", s.remain(), Color::Gray),
        ("Locked", s.locked, Color::DarkGray),
        ("Easy", s.easy, difficulty_color(1)),
        ("Medium", s.medium, difficulty_color(2)),
        ("Hard", s.hard, difficulty_color(3)),
    ];

    let mut spans = Vec::new();
    let mut used = 0usize;
    for (label, value, color) in groups {
        let text = format!("{label}: {value}");
        let needed = text.len() + if spans.is_empty() { 0 } else { STATS_GAP.len() };
        if used + needed > width as usize {
            break;
        }

        if !spans.is_empty() {
            spans.push(Span::raw(STATS_GAP));
        }
        spans.push(Span::styled(
            format!("{label}: "),
            Style::new().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(value.to_string(), Style::new().fg(color)));
        used += needed;
    }

    Line::from(spans)
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
    fn the_stats_line_drops_groups_that_do_not_fit_rather_than_clipping_a_number() {
        let m = crate::tui::view::test_util::listed_model();

        let wide = stats_line(&m, 120);
        assert!(wide.to_string().contains("Hard: 1"), "{wide:?}");

        // trace: "Listed: 3" is 9 columns and "Solved: 1" costs 9 + a 3-column gap, so 21 fits
        // exactly those two and nothing after them.
        let narrow = stats_line(&m, 21).to_string();
        assert!(narrow.contains("Listed: 3"), "{narrow}");
        assert!(narrow.contains("Solved: 1"), "{narrow}");
        assert!(!narrow.contains("Tried"), "{narrow}");
        assert!(narrow.len() <= 21, "line grew past the width: {narrow:?}");
    }

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
        assert_eq!(UnicodeWidthStr::width(fit("兩數之和", 5).as_str()), 5);
    }
}
