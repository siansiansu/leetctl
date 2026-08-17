//! Creating the solution and test-case files for a problem
use crate::cache::models::{Problem, Question};
use crate::config::Config;
use crate::{Cache, Result};
use anyhow::anyhow;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Whether to print the "is on the run..." banner when a description has to be fetched.
///
/// The TUI passes [`Announce::Silent`]: it owns the screen, and a `println!` would land on top of
/// the alternate screen instead of scrolling past.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Announce {
    Print,
    Silent,
}

/// Make sure the solution file for `id` exists, and return its path.
///
/// Creates the file (and, when `code.test` is on, the test-case file) on first use, filled from the
/// problem's description and the language's code stub. An existing file is left alone — this is
/// safe to call before every edit, test, or submit.
///
/// `lang` overrides the configured language *and persists that choice*, matching what
/// `leetctl edit --lang` has always done.
pub async fn ensure_code_file(
    cache: &Cache,
    id: i32,
    lang: Option<String>,
    announce: Announce,
) -> Result<String> {
    let problem = cache.get_problem(id)?;
    let mut conf = cache.to_owned().0.conf;

    // `code.comment_leading` is its own config key, so the description comment reads the same
    // before and after a language override.
    if let Some(lang) = lang {
        conf.code.lang = lang;
        conf.sync()?;
    }

    let path = crate::helper::code_path(&problem, Some(conf.code.lang.to_owned()))?;
    if Path::new(&path).exists() {
        return Ok(path);
    }

    let mut question: std::result::Result<Question, _> = serde_json::from_str(&problem.desc);
    if question.is_err() {
        if announce == Announce::Print {
            println!("{}", problem.banner());
        }
        question = Ok(cache.get_question(id).await?);
    }

    write_code_file(&path, &problem, &question?, &conf)?;

    Ok(path)
}

/// Fill a fresh solution file, and the test-case file alongside it.
///
/// Removes both again if the language has no stub for this problem, so a later run does not find a
/// half-written file and skip the scaffold.
fn write_code_file(
    path: &str,
    problem: &Problem,
    question: &Question,
    conf: &Config,
) -> Result<()> {
    let lang = &conf.code.lang;
    let test_flag = conf.code.test;
    let test_path = crate::helper::test_cases_path(problem)?;

    let mut file_code = File::create(path)?;
    let problem_comment = problem.desc_comment(conf);
    let question_comment = question.desc_comment(conf) + "\n";

    let mut has_stub = false;
    for definition in &question.defs.0 {
        if definition.value != *lang {
            continue;
        }

        has_stub = true;
        if conf.code.comment_problem_desc {
            file_code.write_all(problem_comment.as_bytes())?;
            file_code.write_all(question_comment.as_bytes())?;
        }
        if let Some(inject_before) = &conf.code.inject_before {
            for line in inject_before {
                file_code.write_all((line.to_string() + "\n").as_bytes())?;
            }
        }
        if conf.code.edit_code_marker {
            file_code.write_all(
                marker(&conf.code.comment_leading, &conf.code.start_marker).as_bytes(),
            )?;
        }
        file_code.write_all((definition.code.to_string() + "\n").as_bytes())?;
        if conf.code.edit_code_marker {
            file_code
                .write_all(marker(&conf.code.comment_leading, &conf.code.end_marker).as_bytes())?;
        }
        if let Some(inject_after) = &conf.code.inject_after {
            for line in inject_after {
                file_code.write_all((line.to_string() + "\n").as_bytes())?;
            }
        }

        if test_flag {
            let mut file_tests = File::create(&test_path)?;
            file_tests.write_all(question.all_cases.as_bytes())?;
        }
    }

    if !has_stub {
        std::fs::remove_file(path)?;
        if test_flag {
            std::fs::remove_file(&test_path)?;
        }

        return Err(anyhow!("This question doesn't support {lang}, please try another").into());
    }

    Ok(())
}

/// A commented-out marker line, e.g. `// @lc code=start`.
fn marker(comment_leading: &str, marker: &str) -> String {
    format!("{comment_leading} {marker}\n")
}

/// A resolved editor invocation.
pub struct EditorCommand {
    pub program: String,
    /// Configured `editor_args`, with the file to open last.
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
}

/// How to open `path` in the configured editor.
///
/// The path to open is the last argument, after any configured `editor_args`:
///
/// ```toml
/// [code]
/// editor = "emacsclient"
/// editor_args = [ "-n", "-s", "doom" ]
/// editor_envs = [ "XDG_CONFIG_HOME=..." ]
/// ```
///
/// becomes `emacsclient -n -s doom <path>` with `XDG_CONFIG_HOME` set.
pub fn editor_command(conf: &Config, path: String) -> Result<EditorCommand> {
    let mut args: Vec<String> = Default::default();
    if let Some(editor_args) = &conf.code.editor_args {
        args.extend_from_slice(editor_args);
    }
    args.push(path);

    let mut envs: Vec<(String, String)> = Default::default();
    if let Some(editor_envs) = &conf.code.editor_envs {
        for env in editor_envs.iter() {
            let parts: Vec<&str> = env.split('=').collect();
            if parts.len() != 2 {
                return Err(anyhow!("Invalid editor environment variable: {env}").into());
            }

            envs.push((parts[0].trim().to_string(), parts[1].trim().to_string()));
        }
    }

    Ok(EditorCommand {
        program: conf.code.editor.clone(),
        args,
        envs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_lines_are_commented_and_newline_terminated() {
        assert_eq!(marker("//", "@lc code=start"), "// @lc code=start\n");
    }
}
