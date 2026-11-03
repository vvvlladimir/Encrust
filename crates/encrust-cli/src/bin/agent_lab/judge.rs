//! Each preparation drawn, the autopilot's and a client's, and a judge of its own — another
//! command, so no model grades its own work — asked which prints better (ADR 0403).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::agent::{Programs, run_bounded};

/// The sides each preparation is drawn from: as it stands, and its supports' feet and drains.
const VIEWS: &str = "iso,bottom";
const SIZE_PX: u32 = 512;

/// What the judge is asked; it reads the pictures itself.
const ASK: &str = "Two preparations of the resin print {model} are drawn below, each from the \
front right above and from under the plate, with what its check measured. A: the engine's \
autopilot, {a_pictures}, measured {a_score}. B: a chat client, {b_pictures}, measured \
{b_score}. Look at the pictures. Judge which prints better: fewer defects first, then supports \
off the surfaces a person sees, then resin and time. Answer with one line of JSON and nothing \
after it: {\"winner\": \"a\" or \"b\" or \"tie\", \"why\": \"one sentence\"}.";

/// The judge, a command split at whitespace with `{prompt}` filled in.
#[derive(Debug, Clone, PartialEq)]
pub struct Judge {
    pub words: Vec<String>,
}

/// What the judge said of a client's preparation against the autopilot's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub winner: Winner,
    pub why: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Winner {
    Autopilot,
    Agent,
    Tie,
}

/// A preparation as it is drawn and measured, for the judge.
pub struct Drawn<'a> {
    pub pictures: &'a [PathBuf],
    pub score: &'a Value,
}

impl Judge {
    pub fn parse(command: &str) -> Result<Self> {
        let words: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        if words.is_empty() {
            bail!("a judge needs a command");
        }
        Ok(Self { words })
    }

    /// Asks which of `autopilot` and `agent` prints `model` better, in `work`.
    pub fn rule(
        &self,
        model: &str,
        (autopilot, agent): (Drawn, Drawn),
        work: &Path,
        programs: &Programs,
    ) -> Result<Verdict> {
        std::fs::create_dir_all(work).with_context(|| format!("creating {}", work.display()))?;
        let listed = |pictures: &[PathBuf]| {
            let paths: Vec<String> = pictures
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            paths.join(" and ")
        };
        let prompt = ASK
            .replace("{model}", model)
            .replace("{a_pictures}", &listed(autopilot.pictures))
            .replace("{a_score}", &autopilot.score.to_string())
            .replace("{b_pictures}", &listed(agent.pictures))
            .replace("{b_score}", &agent.score.to_string());
        let words: Vec<String> = self
            .words
            .iter()
            .map(|word| word.replace("{prompt}", &prompt))
            .collect();
        let mut command = Command::new(&words[0]);
        command.args(&words[1..]).current_dir(work);
        let stdout = work.join("stdout.txt");
        let status = run_bounded(command, &stdout, &work.join("stderr.txt"), programs.timeout)?;
        if status.is_none() {
            bail!("the judge ran past {} s", programs.timeout.as_secs());
        }
        verdict_in(&std::fs::read_to_string(&stdout)?)
            .with_context(|| format!("the judge gave no verdict; see {}", work.display()))
    }
}

/// The pictures of a preparation the command line stages with `args`, drawn into `dir`.
pub fn draw(args: &[String], dir: &Path, programs: &Programs) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut command = Command::new(&programs.encrust);
    command
        .args(["--json", "render"])
        .args(args)
        .arg(format!("--view={VIEWS}"))
        .arg(format!("--size={SIZE_PX}"))
        .arg(format!("--output={}", dir.display()));
    let stdout = dir.join("render.json");
    let status = run_bounded(command, &stdout, &dir.join("stderr.txt"), programs.timeout)?;
    if !status.is_some_and(|status| status.success()) {
        bail!("drawing failed; see {}", dir.display());
    }
    let report: Value = serde_json::from_str(std::fs::read_to_string(&stdout)?.trim())
        .with_context(|| format!("render printed no report; see {}", dir.display()))?;
    Ok(report["pictures"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|picture| picture["path"].as_str().map(PathBuf::from))
        .collect())
}

/// The last `{"winner": …}` the judge printed: on a line of its own, or inside a client's
/// JSON `result` text. The judge names the preparations `a` and `b`.
fn verdict_in(stdout: &str) -> Option<Verdict> {
    let said = |text: &str| -> Option<Verdict> {
        let start = text.rfind("{\"winner\"")?;
        let value: Value = serde_json::Deserializer::from_str(&text[start..])
            .into_iter()
            .next()?
            .ok()?;
        let winner = match value["winner"].as_str()? {
            "a" => Winner::Autopilot,
            "b" => Winner::Agent,
            "tie" => Winner::Tie,
            _ => return None,
        };
        Some(Verdict {
            winner,
            why: value["why"].as_str().unwrap_or_default().to_owned(),
        })
    };
    let wrapped = serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .and_then(|value| value["result"].as_str().map(str::to_owned));
    wrapped.as_deref().and_then(said).or_else(|| said(stdout))
}

/// The score a judge is shown, or that there is none.
pub fn shown(score: Option<&impl Serialize>) -> Value {
    score.map_or_else(|| json!("no score"), |score| json!(score))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verdict_is_read_off_its_own_line_after_anything_else() {
        let said = "thinking…\n{\"winner\": \"b\", \"why\": \"fewer marks\"}\n";
        assert_eq!(
            verdict_in(said),
            Some(Verdict {
                winner: Winner::Agent,
                why: "fewer marks".into()
            })
        );
    }

    #[test]
    fn a_verdict_inside_a_client_s_json_result_is_read() {
        let said = r#"{"type": "result", "result": "A is cleaner.\n{\"winner\": \"a\", \"why\": \"no cups\"}"}"#;
        assert_eq!(
            verdict_in(said).map(|verdict| verdict.winner),
            Some(Winner::Autopilot)
        );
    }

    #[test]
    fn no_verdict_or_an_unknown_winner_is_none() {
        assert_eq!(verdict_in("I cannot tell."), None);
        assert_eq!(verdict_in("{\"winner\": \"c\", \"why\": \"\"}"), None);
    }

    #[test]
    fn a_judge_with_no_command_is_refused() {
        assert!(Judge::parse("  ").is_err());
    }
}
