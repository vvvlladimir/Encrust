//! `cargo xtask i18n <code>`: every message English has and a translation lacks, written as
//! Fluent to paste into the translation and translate; see `docs/design/i18n.md`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(clap::Args)]
pub struct Args {
    /// The language, as its catalogue is named: `ru` for `assets/i18n/ru.ftl`.
    language: String,
}

pub fn run(args: &Args) -> Result<()> {
    let dir = catalogues()?;
    let english = read(&dir.join("en.ftl"))?;
    let translated = read(&dir.join(format!("{}.ftl", args.language)))?;
    let (missing, count) = untranslated(&english, &ids(&translated));
    print!("{missing}");
    eprintln!(
        "{count} of {} messages are not in {}.ftl",
        messages(&english).len(),
        args.language
    );
    Ok(())
}

fn catalogues() -> Result<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/i18n");
    dir.canonicalize()
        .with_context(|| format!("cannot find the catalogues in {}", dir.display()))
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

/// One message of a catalogue: the group it is listed under, its id, and its lines, with
/// the comment over it.
struct Message<'a> {
    group: &'a str,
    id: &'a str,
    text: String,
}

/// The messages of `source` in order. A message starts at an id in the first column and
/// runs over the indented lines under it; a `#` comment over it goes with it.
fn messages(source: &str) -> Vec<Message<'_>> {
    let mut found = Vec::new();
    let (mut group, mut comment) = ("", String::new());
    for line in source.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            group = name;
        } else if line.starts_with("# ") {
            comment.push_str(line);
            comment.push('\n');
        } else if let Some((id, _)) = line.split_once(" =").filter(|_| starts_an_id(line)) {
            let text = std::mem::take(&mut comment) + line + "\n";
            found.push(Message { group, id, text });
        } else if line.starts_with(' ')
            && let Some(last) = found.last_mut()
        {
            last.text.push_str(line);
            last.text.push('\n');
        } else {
            comment.clear();
        }
    }
    found
}

fn starts_an_id(line: &str) -> bool {
    line.starts_with(|c: char| c.is_ascii_alphabetic())
}

fn ids(source: &str) -> BTreeSet<&str> {
    messages(source)
        .into_iter()
        .map(|message| message.id)
        .collect()
}

/// The messages of `english` whose id is not in `have`, under the group headings they
/// stand under there, and how many there are.
fn untranslated(english: &str, have: &BTreeSet<&str>) -> (String, usize) {
    let mut out = String::new();
    let mut group = None;
    let mut count = 0;
    for message in messages(english) {
        if have.contains(message.id) {
            continue;
        }
        if group != Some(message.group) {
            let _ = write!(out, "\n## {}\n\n", message.group);
            group = Some(message.group);
        }
        out.push_str(&message.text);
        count += 1;
    }
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENGLISH: &str = "\
### The catalogue.

## panels/cut.rs

cut-plane = Plane
# Starts with a space.
cut-pins = { \" \" }with pins
cut-mended =
    { $count ->
        [one] one piece
       *[other] { $count } pieces
    }

## panels/drain.rs

drain-hole = Hole
";

    #[test]
    fn only_what_the_translation_lacks_is_listed_under_its_heading() {
        let have = ids("cut-plane = Plane, translated\n");
        let (missing, count) = untranslated(ENGLISH, &have);
        assert_eq!(count, 3, "cut-pins, cut-mended and drain-hole");
        assert!(!missing.contains("cut-plane"), "{missing}");
        assert!(
            missing.contains("# Starts with a space.\ncut-pins"),
            "a comment goes with its message: {missing}"
        );
        assert!(
            missing.contains("       *[other] { $count } pieces\n    }\n"),
            "every line of a message goes with it: {missing}"
        );
        assert!(
            missing.find("## panels/cut.rs") < missing.find("## panels/drain.rs"),
            "the groups keep English's order: {missing}"
        );
    }

    #[test]
    fn a_full_translation_lists_nothing() {
        let have = ids(ENGLISH);
        assert_eq!(untranslated(ENGLISH, &have), (String::new(), 0));
    }
}
