//! Leetcode data models
use super::schemas::{problems, tags};
use crate::helper::HTML;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Number;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// Tag model
#[derive(Clone, Insertable, Queryable, Serialize, Debug)]
#[diesel(table_name = tags)]
pub struct Tag {
    pub tag: String,
    pub refs: String,
}

/// Problem model
#[derive(AsChangeset, Clone, Identifiable, Insertable, Queryable, Serialize, Debug)]
#[diesel(table_name = problems)]
pub struct Problem {
    pub category: String,
    pub fid: i32,
    pub id: i32,
    pub level: i32,
    pub locked: bool,
    pub name: String,
    pub percent: f32,
    pub slug: String,
    pub starred: bool,
    pub status: String,
    pub desc: String,
}

impl Problem {
    fn display_level(&self) -> &str {
        crate::helper::Difficulty::from_level(self.level).map_or("Unknown", |d| d.as_str())
    }

    /// The "problem is being fetched" line the commands print before a description fetch.
    ///
    /// Lives here rather than in `Cache::get_question` because the cache layer must not print —
    /// a `println!` there would land on top of the TUI's alternate screen.
    pub fn banner(&self) -> String {
        let ids = match self.level {
            1 => self.fid.to_string().green(),
            2 => self.fid.to_string().yellow(),
            3 => self.fid.to_string().red(),
            _ => self.fid.to_string().dimmed(),
        };

        format!(
            "\n[{}] {} {}\n\n",
            ids,
            self.name.bold().underline(),
            "is on the run...".dimmed()
        )
    }

    pub fn desc_comment(&self, conf: &Config) -> String {
        let mut res = String::new();
        let comment_leading = &conf.code.comment_leading;
        res += format!("{} Category: {}\n", comment_leading, self.category).as_str();
        res += format!("{} Level: {}\n", comment_leading, self.display_level(),).as_str();
        res += format!("{} Percent: {}%\n\n", comment_leading, self.percent).as_str();

        res + "\n"
    }
}

/// A problem with only the fields a test cares about set, and plausible defaults elsewhere.
/// Internal id mirrors the frontend id, which keeps tag joins (`squash`) easy to reason about.
#[cfg(test)]
pub(crate) fn fixture(fid: i32, level: i32, name: &str) -> Problem {
    Problem {
        category: "algorithms".into(),
        fid,
        id: fid,
        level,
        locked: false,
        name: name.into(),
        percent: 50.0,
        slug: name.to_lowercase().replace(' ', "-"),
        starred: false,
        status: String::new(),
        desc: String::new(),
    }
}

static DONE: &str = " ✔";
static ETC: &str = "...";
static LOCK: &str = "🔒";
static NDONE: &str = " ✘";
static SPACE: &str = " ";
impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let space_2 = SPACE.repeat(2);
        let mut lock = space_2.as_str();
        let mut done = space_2.normal();
        let mut id = "".to_string();
        let mut name = "".to_string();
        let mut level = "".normal();

        if self.locked {
            lock = LOCK
        };
        if self.status == "ac" {
            done = DONE.green().bold();
        } else if self.status == "notac" {
            done = NDONE.green().bold();
        }

        match self.fid.to_string().len() {
            1 => {
                id.push_str(&SPACE.repeat(2));
                id.push_str(&self.fid.to_string());
                id.push_str(SPACE);
            }
            2 => {
                id.push_str(SPACE);
                id.push_str(&self.fid.to_string());
                id.push_str(SPACE);
            }
            3 => {
                id.push_str(SPACE);
                id.push_str(&self.fid.to_string());
            }
            4 => {
                id.push_str(&self.fid.to_string());
            }
            _ => {
                id.push_str(&space_2);
                id.push_str(&space_2);
            }
        }

        let name_width = UnicodeWidthStr::width(self.name.as_str());
        let target_width = 60;
        if name_width <= target_width {
            name.push_str(&self.name);
            name.push_str(&SPACE.repeat(target_width - name_width));
        } else {
            // truncate carefully to target width - 3 (because "..." will take some width)
            let mut truncated = String::new();
            let mut current_width = 0;
            for c in self.name.chars() {
                let char_width = UnicodeWidthChar::width(c).unwrap_or(0);
                if current_width + char_width > target_width - 3 {
                    break;
                }
                truncated.push(c);
                current_width += char_width;
            }
            truncated.push_str(ETC); // add "..."
            let truncated_width = UnicodeWidthStr::width(truncated.as_str());
            truncated.push_str(&SPACE.repeat(target_width - truncated_width));
            name = truncated;
        }

        // Padded to the width of "Medium" so the columns line up.
        level = match crate::helper::Difficulty::from_level(self.level) {
            Some(d @ crate::helper::Difficulty::Easy) => format!("{:6}", d.as_str()).bright_green(),
            Some(d @ crate::helper::Difficulty::Medium) => {
                format!("{:6}", d.as_str()).bright_yellow()
            }
            Some(d @ crate::helper::Difficulty::Hard) => format!("{:6}", d.as_str()).bright_red(),
            None => level,
        };

        let mut pct = self.percent.to_string();
        if pct.len() < 5 {
            pct.push_str(&"0".repeat(5 - pct.len()));
        }
        write!(
            f,
            "  {} {} [{}] {} {} ({} %)",
            lock,
            done,
            id,
            name,
            level,
            &pct[..5]
        )
    }
}

/// desc model
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Question {
    pub content: String,
    pub stats: Stats,
    pub defs: CodeDefintion,
    pub case: String,
    pub all_cases: String,
    pub metadata: MetaData,
    pub test: bool,
    pub t_content: String,
}

impl Question {
    pub fn desc(&self) -> String {
        self.content.render()
    }

    pub fn desc_comment(&self, conf: &Config) -> String {
        let desc = self.content.render();

        let mut res = desc.lines().fold("\n".to_string(), |acc, e| {
            acc + "" + conf.code.comment_leading.as_str() + " " + e + "\n"
        });
        res += " \n";

        res
    }
}

use question::*;
/// deps of Question
mod question {
    use serde::{Deserialize, Serialize};

    /// Code samples
    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct CodeDefintion(pub Vec<CodeDefintionInner>);

    /// CodeDefinition Inner struct
    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct CodeDefintionInner {
        pub value: String,
        pub text: String,
        #[serde(alias = "defaultCode")]
        pub code: String,
    }

    /// Question status
    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct Stats {
        #[serde(alias = "totalAccepted")]
        tac: String,
        #[serde(alias = "totalSubmission")]
        tsm: String,
        #[serde(alias = "totalAcceptedRaw")]
        tacr: i32,
        #[serde(alias = "totalSubmissionRaw")]
        tsmr: i32,
        #[serde(alias = "acRate")]
        rate: String,
    }

    /// Algorithm metadata
    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct MetaData {
        pub name: Option<String>,
        pub params: Option<Vec<Param>>,
        pub r#return: Return,
    }

    /// MetaData nested fields
    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct Param {
        pub name: String,
        pub r#type: String,
    }

    /// MetaData nested fields
    #[derive(Debug, Default, Serialize, Deserialize)]
    pub struct Return {
        pub r#type: String,
    }
}

/// run_code Result
#[derive(Debug, Deserialize)]
pub struct RunCode {
    #[serde(default)]
    pub interpret_id: String,
    #[serde(default)]
    pub test_case: String,
    #[serde(default)]
    pub submission_id: i64,
}

use super::parser::ssr;
use crate::cache::Run;

/// verify result model
#[derive(Default, Debug, Deserialize)]
pub struct VerifyResult {
    pub state: String,
    #[serde(skip)]
    pub name: String,
    #[serde(skip)]
    pub data_input: String,
    #[serde(skip)]
    pub result_type: Run,
    // #[serde(default)]
    // lang: String,
    #[serde(default)]
    pretty_lang: String,
    // #[serde(default)]
    // submission_id: String,
    // #[serde(default)]
    // run_success: bool,
    #[serde(default)]
    correct_answer: bool,
    #[serde(default, deserialize_with = "ssr")]
    code_answer: Vec<String>,
    #[serde(default, deserialize_with = "ssr")]
    code_output: Vec<String>,
    #[serde(default, deserialize_with = "ssr")]
    expected_output: Vec<String>,
    #[serde(default, deserialize_with = "ssr")]
    std_output: Vec<String>,

    // flatten
    // #[serde(flatten, default)]
    // info: VerifyInfo,
    #[serde(flatten, default)]
    status: VerifyStatus,
    #[serde(flatten, default)]
    analyse: Analyse,
    #[serde(flatten, default)]
    expected: Expected,
    #[serde(flatten, default)]
    error: CompileError,
    #[serde(flatten, default)]
    submit: Submit,
}

impl VerifyResult {
    /// A submission LeetCode accepted: status 10 plus the per-case comparison string it only
    /// sends for a finished submission.
    pub fn is_accepted(&self) -> bool {
        self.status.status_code == 10
            && matches!(self.result_type, Run::Submit)
            && !self.submit.compare_result.is_empty()
    }

    /// Internal problem id of an accepted submission, for marking the cached row solved.
    /// `None` for anything that was not accepted; an error only if LeetCode sent a
    /// `question_id` that is not an integer.
    pub fn accepted_problem_id(&self) -> std::result::Result<Option<i32>, std::num::ParseIntError> {
        if !self.is_accepted() {
            return Ok(None);
        }

        self.submit.question_id.parse().map(Some)
    }
}

impl std::fmt::Display for VerifyResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ca = match &self.code_answer.len() {
            1 => self.code_answer[0].to_string(),
            _ => self.code_answer.join("↩ "),
        };

        let eca = match &self.expected.expected_code_answer.len() {
            1 => self.expected.expected_code_answer[0].to_string(),
            _ => self.expected.expected_code_answer.join("↩ "),
        };

        debug!("{:#?}", self);

        match &self.status.status_code {
            10 => {
                if matches!(self.result_type, Run::Test) && self.correct_answer {
                    // Pass Tests
                    write!(
                        f,
                        "\n{}{}{}\n{}{}{}{}{}{}\n",
                        self.status.status_msg.green().bold(),
                        "Runtime: ".before_spaces(7).dimmed(),
                        self.status.status_runtime.dimmed(),
                        "\nYour input:".after_spaces(4),
                        self.data_input.replace('\n', "↩ "),
                        "\nOutput:".after_spaces(8),
                        ca,
                        "\nExpected:".after_spaces(6),
                        eca,
                    )?
                } else if matches!(self.result_type, Run::Submit)
                    && !self.submit.compare_result.is_empty()
                {
                    // only Submit execute this branch
                    // Submit Successfully
                    // TODO: result should be all 1;
                    let rp = if let Some(n) = &self.analyse.runtime_percentile {
                        if n.is_f64() {
                            n.as_f64().unwrap_or(0.0) as i64
                        } else {
                            n.as_i64().unwrap_or(0)
                        }
                    } else {
                        0
                    };

                    let mp = if let Some(n) = &self.analyse.memory_percentile {
                        if n.is_f64() {
                            n.as_f64().unwrap_or(0.0) as i64
                        } else {
                            n.as_i64().unwrap_or(0)
                        }
                    } else {
                        0
                    };

                    write!(
                        f,
                        "\n{}{}{}\
                         , faster than \
                         {}{}\
                         of \
                         {} \
                         online submissions for \
                         {}.\n\n\
                         {}{}\
                         , less than \
                         {}{}\
                         of \
                         {} {}.\n\n",
                        "Success\n\n".green().bold(),
                        "Runtime: ".dimmed(),
                        self.status.status_runtime.bold(),
                        rp.to_string().bold(),
                        "% ".bold(),
                        self.pretty_lang,
                        self.name,
                        "Memory Usage: ".dimmed(),
                        self.status.status_memory.bold(),
                        mp.to_string().bold(),
                        "% ".bold(),
                        self.pretty_lang,
                        self.name,
                    )?
                } else {
                    // Wrong Answer during testing
                    write!(
                        f,
                        "\n{}{}{}\n{}{}{}{}{}{}\n",
                        "Wrong Answer".red().bold(),
                        "   Runtime: ".dimmed(),
                        self.status.status_runtime.dimmed(),
                        "\nYour input:".after_spaces(4),
                        self.data_input.replace('\n', "↩ "),
                        "\nOutput:".after_spaces(8),
                        ca,
                        "\nExpected:".after_spaces(6),
                        eca,
                    )?
                }
            }
            // Failed some tests during submission
            11 => write!(
                f,
                "\n{}\n\n{}{}\n{}{}\n{}{}{}{}{}{}\n",
                self.status.status_msg.red().bold(),
                "Cases passed:".after_spaces(2).green(),
                self.analyse
                    .total_correct
                    .as_ref()
                    .unwrap_or(&Number::from(0))
                    .to_string()
                    .green(),
                "Total cases:".after_spaces(3).yellow(),
                self.analyse
                    .total_testcases
                    .as_ref()
                    .unwrap_or(&Number::from(0))
                    .to_string()
                    .bold()
                    .yellow(),
                "Last case:".after_spaces(5).dimmed(),
                self.submit.last_testcase.replace('\n', "↩ ").dimmed(),
                "\nOutput:".after_spaces(8),
                self.code_output[0],
                "\nExpected:".after_spaces(6),
                self.expected_output[0],
            )?,
            // Memory Exceeded
            12 => write!(
                f,
                "\n{}\n\n{}{}\n",
                self.status.status_msg.yellow().bold(),
                "Last case:".after_spaces(5).dimmed(),
                self.data_input.replace('\n', "↩ "),
            )?,
            // Output Timeout Exceeded
            //
            // TODO: 13 and 14 might have some different,
            // if anybody reach this, welcome to fix this!
            13 | 14 => write!(f, "\n{}\n", self.status.status_msg.yellow().bold(),)?,
            // Runtime error
            15 => write!(
                f,
                "\n{}\n{}\n'",
                self.status.status_msg.red().bold(),
                self.status.runtime_error
            )?,
            // Compile Error
            20 => write!(
                f,
                "\n{}:\n\n{}\n",
                self.status.status_msg.red().bold(),
                self.error.full_compile_error.dimmed()
            )?,
            _ => write!(
                f,
                "{}{}{}{}{}{}{}{}",
                "\nUnknown Error...\n".red().bold(),
                "\nBingo! Welcome to fix this! Pull your request at ".yellow(),
                "https://github.com/siansiansu/leetctl/pulls"
                    .dimmed()
                    .underline(),
                ", and this file is located at ".yellow(),
                "leetctl/src/cache/models.rs".dimmed().underline(),
                " waiting for you! Yep, line ".yellow(),
                "385".dimmed().underline(),
                ".\n".yellow(),
            )?,
        };

        match &self.result_type {
            Run::Test => {
                if !self.code_output.is_empty() {
                    write!(
                        f,
                        "{}{}",
                        "Stdout:".after_spaces(8).purple(),
                        self.code_output.join(&"\n".after_spaces(15))
                    )
                } else {
                    write!(f, "")
                }
            }
            _ => {
                if !self.std_output.is_empty() {
                    write!(
                        f,
                        "{}{}",
                        "Stdout:".after_spaces(8).purple(),
                        self.std_output[0].replace('\n', &"\n".after_spaces(15))
                    )
                } else {
                    write!(f, "")
                }
            }
        }
    }
}

use crate::Config;
use verify::*;

mod verify {
    use super::super::parser::ssr;
    use serde::Deserialize;
    use serde_json::Number;

    #[derive(Debug, Default, Deserialize)]
    pub struct Submit {
        #[serde(default)]
        pub question_id: String,
        #[serde(default)]
        pub last_testcase: String,
        #[serde(default)]
        pub compare_result: String,
    }

    // #[derive(Debug, Default, Deserialize)]
    // pub struct VerifyInfo {
    //     #[serde(default)]
    //     memory: i64,
    //     #[serde(default)]
    //     elapsed_time: i64,
    //     #[serde(default)]
    //     task_finish_time: i64,
    // }

    #[derive(Debug, Default, Deserialize)]
    pub struct Analyse {
        #[serde(default)]
        pub total_correct: Option<Number>,
        #[serde(default)]
        pub total_testcases: Option<Number>,
        #[serde(default)]
        pub runtime_percentile: Option<Number>,
        #[serde(default)]
        pub memory_percentile: Option<Number>,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct VerifyStatus {
        #[serde(default)]
        pub status_code: i32,
        #[serde(default)]
        pub status_msg: String,
        #[serde(default)]
        pub status_memory: String,
        #[serde(default)]
        pub status_runtime: String,
        #[serde(default)]
        pub runtime_error: String,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct CompileError {
        // #[serde(default)]
        // compile_error: String,
        #[serde(default)]
        pub full_compile_error: String,
    }

    #[derive(Debug, Default, Deserialize)]
    pub struct Expected {
        // #[serde(default)]
        // expected_status_code: i32,
        // #[serde(default)]
        // expected_lang: String,
        // #[serde(default)]
        // expected_run_success: bool,
        // #[serde(default)]
        // expected_status_runtime: String,
        // #[serde(default)]
        // expected_memory: i64,
        // #[serde(default, deserialize_with = "ssr")]
        // expected_code_output: Vec<String>,
        // #[serde(default)]
        // expected_elapsed_time: i64,
        // #[serde(default)]
        // expected_task_finish_time: i64,
        #[serde(default, deserialize_with = "ssr")]
        pub expected_code_answer: Vec<String>,
    }
}

/// Formatter for str
trait Formatter {
    fn after_spaces(&self, spaces: usize) -> String;
    fn before_spaces(&self, spaces: usize) -> String;
}

impl Formatter for str {
    fn after_spaces(&self, spaces: usize) -> String {
        let mut r = String::new();
        r.push_str(self);
        r.push_str(&" ".repeat(spaces));
        r
    }

    fn before_spaces(&self, spaces: usize) -> String {
        let mut r = String::new();
        r.push_str(&" ".repeat(spaces));
        r.push_str(self);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one shape a submission response needs for the accepted path: status 10, a compare
    /// string, and the internal problem id. `result_type` is `#[serde(skip)]` and defaults to
    /// `Run::Submit`, which is what a real submit carries.
    fn accepted_submission() -> VerifyResult {
        serde_json::from_str(
            r#"{
                "state": "SUCCESS",
                "status_code": 10,
                "status_msg": "Accepted",
                "status_runtime": "4 ms",
                "status_memory": "2.1 MB",
                "runtime_percentile": 91.5,
                "memory_percentile": 47.0,
                "compare_result": "111",
                "question_id": "704",
                "pretty_lang": "Rust"
            }"#,
        )
        .expect("accepted submission fixture should deserialize")
    }

    #[test]
    fn banner_names_the_problem_and_reads_as_in_flight() {
        let rendered = fixture(704, 1, "Binary Search").banner();

        assert!(
            rendered.contains("704"),
            "banner should carry the frontend id"
        );
        assert!(rendered.contains("Binary Search"));
        assert!(rendered.contains("is on the run..."));
    }

    #[test]
    fn is_accepted_only_for_a_finished_submission() {
        assert!(accepted_submission().is_accepted());

        // A test run reaches status 10 too; only a submission carries `compare_result`.
        let mut passing_test = accepted_submission();
        passing_test.result_type = Run::Test;
        assert!(!passing_test.is_accepted());

        let mut wrong_answer = accepted_submission();
        wrong_answer.status.status_code = 11;
        assert!(!wrong_answer.is_accepted());

        assert!(!VerifyResult::default().is_accepted());
    }

    #[test]
    fn accepted_problem_id_is_the_internal_id() {
        assert_eq!(accepted_submission().accepted_problem_id(), Ok(Some(704)));
        assert_eq!(VerifyResult::default().accepted_problem_id(), Ok(None));

        let mut unparseable = accepted_submission();
        unparseable.submit.question_id = "seven-oh-four".to_string();
        assert!(unparseable.accepted_problem_id().is_err());
    }

    /// Regression guard: formatting an accepted submission used to open the sqlite cache and write
    /// `update_after_ac` from inside `fmt`, panicking through `expect` when that failed. Rendering
    /// must be pure — the TUI formats results while drawing.
    #[test]
    fn display_of_an_accepted_submission_only_formats() {
        let rendered = accepted_submission().to_string();

        assert!(rendered.contains("Success"));
        assert!(rendered.contains("4 ms"), "runtime should be reported");
        assert!(
            rendered.contains("91%"),
            "runtime percentile is truncated to a whole percent"
        );
        assert!(rendered.contains("2.1 MB"));
    }
}
