//! The description page and the help page
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::{difficulty_color, hints_line, pad1};
use crate::cache::models::Problem;
use crate::tui::Model;

const DETAIL_HINTS: [(&str, &str); 4] = [
    ("j/k", "scroll"),
    ("gg/G", "top/bottom"),
    ("?", "help"),
    ("esc", "back"),
];

const HELP_HINTS: [(&str, &str); 1] = [("esc", "back")];

/// Every key the TUI answers to, by screen.
const HELP_ROWS: [(&str, &str, &str); 15] = [
    (
        "j / k, ↓ / ↑",
        "list, description",
        "move or scroll one line",
    ),
    (
        "g g / G",
        "list, description",
        "jump to the top or the bottom",
    ),
    (
        "ctrl-d / ctrl-u",
        "list, description",
        "half a screen down or up",
    ),
    ("enter", "list", "open the problem description"),
    ("/", "list", "search: tokens are ANDed, !token excludes"),
    ("s", "list", "pick a curated set"),
    ("d", "list", "cycle difficulty: all, easy, medium, hard"),
    ("u", "list", "show only unsolved problems"),
    ("t", "list", "filter by LeetCode tag"),
    ("D", "list", "jump to today's daily challenge"),
    ("esc", "list", "drop every filter"),
    ("esc / q", "description", "back to the table"),
    ("?", "anywhere", "this page"),
    ("q", "list", "quit"),
    ("ctrl-c", "anywhere", "quit"),
];

pub(super) fn draw_detail(m: &Model, f: &mut Frame) {
    let area = f.area();
    if area.height < 5 || area.width < 24 {
        return;
    }

    let title = match m.detail_problem() {
        Some(problem) => problem_title(problem, m.daily_fid == Some(problem.fid)),
        None => Line::from(Span::raw(" problem ")),
    };

    let body = match m.detail_text() {
        Some(_) => m
            .detail_lines()
            .into_iter()
            .skip(m.detail_scroll)
            .map(|line| pad1(Line::from(Span::raw(line))))
            .collect(),
        None => vec![pad1(Line::from(Span::styled(
            "Fetching the description…",
            Style::new().fg(Color::DarkGray),
        )))],
    };

    draw_page(f, area, title, body, &DETAIL_HINTS, m);
}

pub(super) fn draw_help(m: &Model, f: &mut Frame) {
    let area = f.area();
    if area.height < 5 || area.width < 24 {
        return;
    }

    let body = HELP_ROWS
        .iter()
        .map(|(keys, screen, what)| {
            pad1(Line::from(vec![
                Span::styled(
                    format!("{keys:<17}"),
                    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{screen:<20}"), Style::new().fg(Color::DarkGray)),
                Span::raw(*what),
            ]))
        })
        .collect();

    draw_page(
        f,
        area,
        Line::from(Span::styled(
            " keys ",
            Style::new().add_modifier(Modifier::BOLD),
        )),
        body,
        &HELP_HINTS,
        m,
    );
}

/// The shared chrome of a full-screen page: framed body with a title, hints along the bottom.
fn draw_page(
    f: &mut Frame,
    area: Rect,
    title: Line<'static>,
    body: Vec<Line<'static>>,
    hints: &[(&'static str, &'static str)],
    m: &Model,
) {
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::Cyan))
        .title(title.centered());
    let inner = block.inner(Rect::new(area.x, area.y, area.width, area.height - 1));
    f.render_widget(
        block,
        Rect::new(area.x, area.y, area.width, area.height - 1),
    );
    f.render_widget(Paragraph::new(body), inner);

    let footer = if m.status.is_empty() {
        hints_line(hints)
    } else {
        Line::from(Span::styled(
            m.status.clone(),
            Style::new().fg(Color::Yellow),
        ))
    };
    f.render_widget(
        Paragraph::new(pad1(footer)),
        Rect::new(area.x, area.y + area.height - 1, area.width, 1),
    );
}

/// ` [1] Two Sum · Easy · 58.01% ★ daily `
fn problem_title(p: &Problem, is_daily: bool) -> Line<'static> {
    let difficulty = crate::helper::Difficulty::from_level(p.level).map_or("?", |d| d.as_str());
    let mut spans = vec![
        Span::styled(format!(" [{}] ", p.fid), Style::new().fg(Color::DarkGray)),
        Span::styled(p.name.clone(), Style::new().add_modifier(Modifier::BOLD)),
        Span::raw(" · "),
        Span::styled(difficulty, Style::new().fg(difficulty_color(p.level))),
        Span::styled(
            format!(" · {:.2}% ", p.percent),
            Style::new().fg(Color::DarkGray),
        ),
    ];

    if is_daily {
        spans.push(Span::styled(
            "★ daily ",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}
