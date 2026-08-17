//! Word wrapping for the description pane
//!
//! Ratatui can wrap a paragraph while rendering, but scrolling needs to know how many lines that
//! produced in order to clamp the offset. Wrapping here means the model and the view agree on the
//! line count.

/// Wraps `text` to `width` columns, keeping blank lines as paragraph breaks.
///
/// A word longer than the whole width (a long URL, a wall of digits) is broken across lines rather
/// than pushed past the edge.
pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    for source_line in text.lines() {
        if source_line.trim().is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut line = String::new();
        for word in source_line.split_whitespace() {
            if line.is_empty() {
                line.push_str(word);
            } else if line.chars().count() + 1 + word.chars().count() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line.push_str(word);
            }

            while line.chars().count() > width {
                let split_at = line
                    .char_indices()
                    .nth(width)
                    .map(|(i, _)| i)
                    .unwrap_or(line.len());
                let rest = line.split_off(split_at);
                lines.push(std::mem::replace(&mut line, rest));
            }
        }

        lines.push(line);
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_line_is_left_alone() {
        assert_eq!(wrap("two sum", 20), ["two sum"]);
    }

    #[test]
    fn words_move_to_the_next_line_at_the_boundary() {
        // trace: "given an" is 8 columns; adding " array" would make 14, past the width of 10.
        assert_eq!(
            wrap("given an array of ints", 10),
            ["given an", "array of", "ints"]
        );
    }

    #[test]
    fn blank_lines_survive_as_paragraph_breaks() {
        assert_eq!(wrap("one\n\ntwo", 10), ["one", "", "two"]);
    }

    #[test]
    fn a_word_wider_than_the_line_is_broken_up() {
        assert_eq!(wrap("aaaaaaa", 3), ["aaa", "aaa", "a"]);
    }

    #[test]
    fn every_line_fits_the_width() {
        let text = "1 <= nums.length <= 10^4 and -10^9 <= nums[i] <= 10^9, https://leetcode.com/problems/two-sum/description";
        for line in wrap(text, 24) {
            assert!(line.chars().count() <= 24, "too wide: {line:?}");
        }
    }

    #[test]
    fn a_zero_width_pane_wraps_to_nothing_rather_than_looping() {
        assert!(wrap("two sum", 0).is_empty());
    }
}
