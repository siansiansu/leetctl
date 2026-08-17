//! Rendering. Difficulty colors follow the CLI's green / yellow / red.
mod list;
mod overlays;

use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::helper::Difficulty;
use crate::tui::Model;

pub(crate) fn draw(m: &Model, f: &mut Frame) {
    if m.loading {
        draw_loading(m, f);
        return;
    }

    list::draw_list(m, f);
    // The picker floats over the table it is narrowing.
    overlays::draw_set_picker(m, f);
}

fn draw_loading(m: &Model, f: &mut Frame) {
    let mut lines = vec![
        pad1(Line::from(Span::styled(
            format!("leetctl {}", env!("CARGO_PKG_VERSION")),
            Style::new().add_modifier(Modifier::BOLD),
        ))),
        pad1(Line::from(Span::styled(
            "Loading problems…",
            Style::new().dim(),
        ))),
    ];
    if !m.status.is_empty() {
        lines.push(pad1(Line::from(Span::styled(
            m.status.clone(),
            Style::new().fg(Color::Red),
        ))));
    }

    f.render_widget(Paragraph::new(lines), f.area());
}

/// One cell of horizontal padding, so text never touches the terminal edge.
pub(super) fn pad1(line: Line) -> Line {
    Line::from_iter(std::iter::once(Span::raw(" ")).chain(line.spans))
}

pub(super) fn difficulty_color(level: i32) -> Color {
    match Difficulty::from_level(level) {
        Some(Difficulty::Easy) => Color::Green,
        Some(Difficulty::Medium) => Color::Yellow,
        Some(Difficulty::Hard) => Color::Red,
        None => Color::Gray,
    }
}

/// The `key:action` hint bar along the bottom.
pub(super) fn hints_line(pairs: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (key, action)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            *key,
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(":{action}"),
            Style::new().fg(Color::DarkGray),
        ));
    }

    Line::from(spans)
}

#[cfg(test)]
pub(crate) mod test_util {
    use crate::cache::models::fixture;
    use crate::tui::{Model, test_model};
    use ratatui::{Terminal, backend::TestBackend};

    /// Renders a model to plain text, the way the terminal would show it.
    pub(crate) fn render(m: &Model, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| super::draw(m, f)).unwrap();

        let buf = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    /// A loaded model with a solved problem, an attempted one, and a locked one.
    pub(crate) fn listed_model() -> Model {
        let mut m = test_model();
        m.all = vec![
            fixture(1, 1, "Two Sum"),
            fixture(4, 3, "Median of Two Sorted Arrays"),
            fixture(11, 2, "Container With Most Water"),
        ];
        m.all[0].status = "ac".into();
        m.all[1].status = "notac".into();
        m.all[2].locked = true;
        m.apply_filters();
        m
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::{listed_model, render};
    use crate::tui::test_model;

    #[test]
    fn the_loading_screen_names_the_tool_and_what_it_is_doing() {
        let mut m = test_model();
        m.loading = true;

        let screen = render(&m, 60, 10);

        assert!(
            screen.contains("leetctl"),
            "version line missing:\n{screen}"
        );
        assert!(screen.contains("Loading problems"), "{screen}");
    }

    #[test]
    fn a_failed_load_shows_the_error_instead_of_an_empty_screen() {
        let mut m = test_model();
        m.loading = true;
        m.status = "Nothing matched".into();

        let screen = render(&m, 60, 10);

        assert!(screen.contains("Nothing matched"), "{screen}");
    }

    #[test]
    fn the_table_shows_id_name_difficulty_and_status() {
        let screen = render(&listed_model(), 80, 20);

        assert!(screen.contains("Two Sum"), "{screen}");
        assert!(screen.contains("Easy"), "{screen}");
        assert!(screen.contains("Hard"), "{screen}");
        assert!(screen.contains('✔'), "solved glyph missing:\n{screen}");
        assert!(screen.contains('✘'), "attempted glyph missing:\n{screen}");
        assert!(screen.contains('🔒'), "locked glyph missing:\n{screen}");
    }

    #[test]
    fn the_footer_counts_match_the_filtered_pool() {
        let screen = render(&listed_model(), 80, 20);

        // trace: 3 problems, 1 ac, 1 notac, so 1 remains; one of each difficulty; 1 locked.
        assert!(screen.contains("Listed: 3"), "{screen}");
        assert!(screen.contains("Solved: 1"), "{screen}");
        assert!(screen.contains("Remain: 1"), "{screen}");
        assert!(screen.contains("Locked: 1"), "{screen}");
    }

    #[test]
    fn the_panel_title_carries_the_problem_count() {
        let screen = render(&listed_model(), 80, 20);

        assert!(screen.contains("problems[3]"), "{screen}");
    }

    #[test]
    fn active_filters_are_named_in_the_header() {
        let mut m = listed_model();
        m.filters.set = Some("blind75".into());
        m.filters.difficulty = Some(crate::helper::Difficulty::Medium);
        m.apply_filters();

        let screen = render(&m, 80, 20);

        assert!(screen.contains("set:blind75"), "{screen}");
        assert!(screen.contains("difficulty:Medium"), "{screen}");
    }

    #[test]
    fn an_empty_result_says_so_rather_than_showing_a_blank_table() {
        let mut m = listed_model();
        m.filters.keyword = Some("no such problem".into());
        m.apply_filters();

        let screen = render(&m, 80, 20);

        assert!(screen.contains("No problems match"), "{screen}");
    }

    #[test]
    fn an_open_search_prompt_takes_over_the_top_line() {
        let mut m = listed_model();
        m.open_prompt(crate::tui::PromptKind::Search);
        m.search = "two".into();
        m.apply_filters();

        let screen = render(&m, 80, 20);

        assert!(screen.contains("/ "), "prompt prefix missing:\n{screen}");
        assert!(
            !screen.contains("leetctl 0"),
            "header should step aside:\n{screen}"
        );
        assert!(
            screen.contains("!word = exclude"),
            "help missing:\n{screen}"
        );
    }

    #[test]
    fn the_tag_prompt_says_what_it_wants() {
        let mut m = listed_model();
        m.open_prompt(crate::tui::PromptKind::Tag);

        let screen = render(&m, 80, 20);

        assert!(screen.contains("tag:"), "{screen}");
        assert!(
            screen.contains("dynamic-programming"),
            "example missing:\n{screen}"
        );
    }

    #[test]
    fn interactive_filters_show_up_as_chips() {
        let mut m = listed_model();
        m.search = "sum".into();
        m.unsolved_only = true;
        m.tag = Some("array".into());
        m.apply_filters();

        let screen = render(&m, 100, 20);

        assert!(screen.contains("search:sum"), "{screen}");
        assert!(screen.contains("unsolved:on"), "{screen}");
        assert!(screen.contains("tag:array"), "{screen}");
    }

    #[test]
    fn the_footer_offers_escape_once_a_filter_is_on() {
        let mut m = listed_model();
        assert!(!render(&m, 100, 20).contains("esc:clear filters"));

        m.unsolved_only = true;
        m.apply_filters();
        assert!(render(&m, 100, 20).contains("esc:clear filters"));
    }

    #[test]
    fn the_set_picker_lists_every_bundled_set_with_its_size() {
        let mut m = listed_model();
        m.open_picker();

        let screen = render(&m, 100, 30);

        assert!(screen.contains("problem set"), "title missing:\n{screen}");
        assert!(screen.contains("Blind 75"), "{screen}");
        assert!(screen.contains("75 problems"), "count missing:\n{screen}");
        assert!(screen.contains('❯'), "selection marker missing:\n{screen}");
        assert!(screen.contains("enter:apply"), "{screen}");
    }

    #[test]
    fn the_picker_keeps_the_highlighted_set_on_screen_when_scrolled() {
        let mut m = listed_model();
        m.open_picker();
        m.picker = Some(m.sets.len() - 1);
        let last = m.sets.last().unwrap().name.clone();

        // A terminal too short for eleven sets still shows the one under the cursor.
        let screen = render(&m, 100, 12);

        assert!(screen.contains(&last), "{screen}");
    }

    #[test]
    fn a_terminal_too_small_to_lay_out_renders_without_panicking() {
        for (w, h) in [(20, 10), (10, 4), (4, 1), (1, 1)] {
            let screen = render(&listed_model(), w, h);
            assert_eq!(screen.lines().count(), h as usize);
        }
    }
}
