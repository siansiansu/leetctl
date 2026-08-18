//! A set of helper traits
pub use self::{
    column::fit_width,
    digit::Digit,
    file::{code_path, load_script, test_cases_path},
    filter::{Difficulty, filter, retain_set, squash},
    html::HTML,
};

/// Convert i32 to specific digits string.
mod digit {
    /// Abstract Digit trait, fill the empty space to specific length.
    pub trait Digit<T> {
        fn digit(self, d: T) -> String;
    }

    impl Digit<i32> for i32 {
        fn digit(self, d: i32) -> String {
            let mut s = self.to_string();
            let space = " ".repeat((d as usize) - s.len());
            s.push_str(&space);

            s
        }
    }

    impl Digit<i32> for String {
        fn digit(self, d: i32) -> String {
            let mut s = self.clone();
            let space = " ".repeat((d as usize) - self.len());
            s.push_str(&space);

            s
        }
    }

    impl Digit<i32> for &'static str {
        fn digit(self, d: i32) -> String {
            let mut s = self.to_string();
            let space = " ".repeat((d as usize) - self.len());
            s.push_str(&space);

            s
        }
    }
}

/// Fitting a value into a fixed-width table column.
mod column {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

    /// Pad or truncate `text` to exactly `width` display columns, marking a truncation with
    /// `ellipsis`.
    ///
    /// Widths are counted in columns rather than bytes or chars, so a CJK problem name lines up
    /// with an ASCII one and a one-column `…` costs one column rather than its three bytes. The
    /// ellipsis comes out of the budget: the result is never wider than `width`.
    ///
    /// The ellipsis differs by frontend — the plain lists spell it `...`, the TUI has the room to
    /// use `…` — which is the only reason it is a parameter.
    pub fn fit_width(text: &str, width: usize, ellipsis: &str) -> String {
        let text_width = UnicodeWidthStr::width(text);
        if text_width <= width {
            return format!("{text}{}", " ".repeat(width - text_width));
        }

        let budget = width.saturating_sub(UnicodeWidthStr::width(ellipsis));
        let mut fitted = String::new();
        let mut fitted_width = 0;
        for c in text.chars() {
            let char_width = UnicodeWidthChar::width(c).unwrap_or(0);
            if fitted_width + char_width > budget {
                break;
            }
            fitted.push(c);
            fitted_width += char_width;
        }

        fitted.push_str(ellipsis);
        let padding = width - UnicodeWidthStr::width(fitted.as_str());
        fitted + &" ".repeat(padding)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_short_value_is_padded_to_the_column() {
            assert_eq!(fit_width("Two Sum", 10, "..."), "Two Sum   ");
        }

        #[test]
        fn an_exact_fit_is_left_alone() {
            assert_eq!(fit_width("Two Sum", 7, "..."), "Two Sum");
        }

        #[test]
        fn an_overlong_value_ends_in_an_ellipsis_inside_the_budget() {
            // trace: width 10 leaves 7 columns before the "...", so "Median " is dropped at "Media".
            assert_eq!(
                fit_width("Median of Two Sorted Arrays", 10, "..."),
                "Median ..."
            );
        }

        #[test]
        fn a_one_column_ellipsis_costs_one_column_not_three_bytes() {
            // trace: "…" is 3 bytes but 1 column, so width 10 leaves 9 columns of text.
            assert_eq!(
                fit_width("Median of Two Sorted Arrays", 10, "…"),
                "Median of…"
            );
        }

        #[test]
        fn wide_characters_count_two_columns() {
            // trace: width 6 leaves 3 columns before the "...", and each ideograph is 2 wide, so
            // only the first fits — "兩" + "..." is 5 columns, padded back out to 6.
            assert_eq!(fit_width("兩數之和", 6, "..."), "兩... ");
        }
    }
}

/// Question filter tool
mod filter {
    use crate::cache::models::Problem;

    /// Problem difficulty, as a command-line value and as the integer the cache stores.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
    pub enum Difficulty {
        Easy,
        Medium,
        Hard,
    }

    impl Difficulty {
        /// `Problem::level` is 1/2/3. This is the only place that mapping is written down.
        pub fn level(self) -> i32 {
            match self {
                Difficulty::Easy => 1,
                Difficulty::Medium => 2,
                Difficulty::Hard => 3,
            }
        }

        /// Inverse of [`Difficulty::level`]; `None` for the levels LeetCode does not use.
        pub fn from_level(level: i32) -> Option<Self> {
            match level {
                1 => Some(Difficulty::Easy),
                2 => Some(Difficulty::Medium),
                3 => Some(Difficulty::Hard),
                _ => None,
            }
        }

        pub fn as_str(self) -> &'static str {
            match self {
                Difficulty::Easy => "Easy",
                Difficulty::Medium => "Medium",
                Difficulty::Hard => "Hard",
            }
        }
    }

    /// Abstract query filter
    ///
    /// ```sh
    ///     -q, --query <query>          Filter questions by conditions:
    ///                                  Uppercase means negative
    ///                                  e = easy     E = m+h
    ///                                  m = medium   M = e+h
    ///                                  h = hard     H = e+m
    ///                                  d = done     D = not done
    ///                                  l = locked   L = not locked
    ///                                  s = starred  S = not starred
    /// ```
    pub fn filter(ps: &mut Vec<Problem>, query: String) {
        for p in query.chars() {
            match p {
                'l' => ps.retain(|x| x.locked),
                'L' => ps.retain(|x| !x.locked),
                's' => ps.retain(|x| x.starred),
                'S' => ps.retain(|x| !x.starred),
                'e' => ps.retain(|x| x.level == Difficulty::Easy.level()),
                'E' => ps.retain(|x| x.level != Difficulty::Easy.level()),
                'm' => ps.retain(|x| x.level == Difficulty::Medium.level()),
                'M' => ps.retain(|x| x.level != Difficulty::Medium.level()),
                'h' => ps.retain(|x| x.level == Difficulty::Hard.level()),
                'H' => ps.retain(|x| x.level != Difficulty::Hard.level()),
                'd' => ps.retain(|x| x.status == "ac"),
                'D' => ps.retain(|x| x.status != "ac"),
                _ => {}
            }
        }
    }

    /// Narrow to the members of a bundled problem set.
    ///
    /// Sets are keyed on the frontend id, so this joins on `Problem::fid` — unlike [`squash`],
    /// which matches the internal ids the tag API returns against `Problem::id`.
    pub fn retain_set(ps: &mut Vec<Problem>, set_slug: &str) -> crate::Result<()> {
        let fids = crate::sets::get(set_slug)?.fids();
        ps.retain(|p| fids.contains(&p.fid));
        Ok(())
    }

    /// Squash questions and ids
    pub fn squash(ps: &mut Vec<Problem>, ids: Vec<String>) -> crate::Result<()> {
        use std::collections::HashMap;

        let mut map: HashMap<String, bool> = HashMap::new();
        ids.iter().for_each(|x| {
            map.insert(x.to_string(), true).unwrap_or_default();
        });

        ps.retain(|x| map.contains_key(&x.id.to_string()));
        Ok(())
    }
}

pub fn superscript(n: u8) -> String {
    match n {
        x if x >= 10 => format!("{}{}", superscript(n / 10), superscript(n % 10)),
        0 => "⁰".to_string(),
        1 => "¹".to_string(),
        2 => "²".to_string(),
        3 => "³".to_string(),
        4 => "⁴".to_string(),
        5 => "⁵".to_string(),
        6 => "⁶".to_string(),
        7 => "⁷".to_string(),
        8 => "⁸".to_string(),
        9 => "⁹".to_string(),
        _ => n.to_string(),
    }
}

pub fn subscript(n: u8) -> String {
    match n {
        x if x >= 10 => format!("{}{}", subscript(n / 10), subscript(n % 10)),
        0 => "₀".to_string(),
        1 => "₁".to_string(),
        2 => "₂".to_string(),
        3 => "₃".to_string(),
        4 => "₄".to_string(),
        5 => "₅".to_string(),
        6 => "₆".to_string(),
        7 => "₇".to_string(),
        8 => "₈".to_string(),
        9 => "₉".to_string(),
        _ => n.to_string(),
    }
}

/// Render html to command-line
mod html {
    use crate::helper::{subscript, superscript};
    use regex::Captures;
    use scraper::Html;

    /// Html render plugin
    pub trait HTML {
        fn render(&self) -> String;
    }

    impl HTML for String {
        fn render(&self) -> String {
            let sup_re = regex::Regex::new(r"<sup>(?P<num>[0-9]*)</sup>").unwrap();
            let sub_re = regex::Regex::new(r"<sub>(?P<num>[0-9]*)</sub>").unwrap();

            let res = sup_re.replace_all(self, |cap: &Captures| {
                let num: u8 = cap["num"].to_string().parse().unwrap();
                superscript(num)
            });

            let res = sub_re.replace_all(&res, |cap: &Captures| {
                let num: u8 = cap["num"].to_string().parse().unwrap();
                subscript(num)
            });

            let frag = Html::parse_fragment(&res);
            frag.root_element()
                .text()
                .fold(String::new(), |acc, e| acc + e)
        }
    }
}

mod file {
    /// Convert file suffix from language type
    pub fn suffix(l: &str) -> crate::Result<&'static str> {
        match l {
            "bash" => Ok("sh"),
            "c" => Ok("c"),
            "cpp" => Ok("cpp"),
            "csharp" => Ok("cs"),
            "elixir" => Ok("ex"),
            "golang" => Ok("go"),
            "java" => Ok("java"),
            "javascript" => Ok("js"),
            "kotlin" => Ok("kt"),
            "mysql" => Ok("sql"),
            "php" => Ok("php"),
            "python" => Ok("py"),
            "python3" => Ok("py"),
            "ruby" => Ok("rb"),
            "rust" => Ok("rs"),
            "scala" => Ok("scala"),
            "swift" => Ok("swift"),
            "typescript" => Ok("ts"),
            _ => Ok("c"),
        }
    }

    use crate::{Error, cache::models::Problem};

    /// Generate test cases path by fid
    pub fn test_cases_path(problem: &Problem) -> crate::Result<String> {
        let conf = crate::config::Config::locate()?;
        let mut path = format!("{}/{}.tests.dat", conf.storage.code()?, conf.code.pick);

        path = path.replace("${fid}", &problem.fid.to_string());
        path = path.replace("${slug}", &problem.slug.to_string());
        Ok(path)
    }

    /// Generate code path by fid
    pub fn code_path(problem: &Problem, l: Option<String>) -> crate::Result<String> {
        let conf = crate::config::Config::locate()?;
        let mut lang = conf.code.lang;
        if l.is_some() {
            lang = l.ok_or(Error::NoneError)?;
        }

        let mut path = format!(
            "{}/{}.{}",
            conf.storage.code()?,
            conf.code.pick,
            suffix(&lang)?,
        );

        path = path.replace("${fid}", &problem.fid.to_string());
        path = path.replace("${slug}", &problem.slug.to_string());

        Ok(path)
    }

    /// Load python scripts
    pub fn load_script(module: &str) -> crate::Result<String> {
        use std::fs::File;
        use std::io::Read;
        let conf = crate::config::Config::locate()?;
        let mut script = "".to_string();
        File::open(format!("{}/{}.py", conf.storage.scripts()?, module))?
            .read_to_string(&mut script)?;

        Ok(script)
    }
}
