//! The drawn dependency graph against the one the manifests declare.
//!
//! `docs/architecture.md` says which crate may depend on which, and
//! `.claude/rules/architecture.md` says anything it does not draw is forbidden. This reads
//! both and answers with the difference, so the drawing cannot drift away from the code.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// The repository to read, for running the check from somewhere else.
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

/// An edge of the graph: who depends on what.
type Edges = BTreeSet<(String, String)>;

/// Every workspace crate and the crates it declares, dev-dependencies apart: a test may
/// reach for a fixture the crate itself must not.
fn declared(crates: &Path) -> Result<(BTreeSet<String>, Edges)> {
    let mut members = BTreeSet::new();
    let mut manifests = Vec::new();
    for entry in fs::read_dir(crates).with_context(|| format!("reading {}", crates.display()))? {
        let path = entry?.path().join("Cargo.toml");
        if !path.is_file() {
            continue;
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let manifest: toml::Table = text
            .parse()
            .with_context(|| format!("parsing {}", path.display()))?;
        let name = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .with_context(|| format!("{} names no package", path.display()))?
            .to_owned();
        members.insert(name.clone());
        manifests.push((name, manifest));
    }

    let mut edges = Edges::new();
    for (name, manifest) in &manifests {
        for dependency in dependencies(manifest) {
            if members.contains(&dependency) {
                edges.insert((name.clone(), dependency));
            }
        }
    }
    Ok((members, edges))
}

/// Every name under `[dependencies]` or `[build-dependencies]`, at the top level and under
/// a `[target.'cfg(…)']` table.
fn dependencies(manifest: &toml::Table) -> Vec<String> {
    let mut names = Vec::new();
    let mut tables = vec![manifest];
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        tables.extend(targets.values().filter_map(toml::Value::as_table));
    }
    for table in tables {
        for key in ["dependencies", "build-dependencies"] {
            if let Some(declared) = table.get(key).and_then(toml::Value::as_table) {
                names.extend(declared.keys().cloned());
            }
        }
    }
    names
}

/// The edges `docs/architecture.md` draws, read out of the one fenced block under
/// "The allowed dependency graph".
fn drawn(doc: &str, members: &BTreeSet<String>) -> Result<Edges> {
    let block = doc
        .split("## The allowed dependency graph")
        .nth(1)
        .and_then(|rest| rest.split("```").nth(1))
        .context("docs/architecture.md draws no dependency graph")?;

    // A rule spans the lines after its arrow, which are the ones that are indented.
    let mut rules: Vec<String> = Vec::new();
    for line in block.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match rules.last_mut() {
            Some(rule) if line.starts_with(' ') => {
                rule.push(' ');
                rule.push_str(line.trim());
            }
            _ => rules.push(line.trim().to_owned()),
        }
    }

    let mut edges = Edges::new();
    for rule in rules {
        let Some((left, right)) = rule.split_once("──>") else {
            bail!("a line of the graph has no arrow: {rule}");
        };
        let sources: Vec<String> = names(left, members);
        if sources.is_empty() {
            bail!("a line of the graph names no crate of this workspace: {rule}");
        }
        for source in sources {
            for target in names(right, members) {
                edges.insert((source.clone(), target));
            }
        }
    }
    Ok(edges)
}

/// The workspace crates a side of a rule names. `every core-*` stands for the family, an
/// aside in brackets or after the name is ignored, and anything that is not a crate of
/// this workspace — a third-party dependency — is not the graph's business.
fn names(side: &str, members: &BTreeSet<String>) -> Vec<String> {
    let mut found = Vec::new();
    for part in side.split(',') {
        let part = part.trim().trim_matches('`');
        if let Some(family) = part
            .strip_prefix("every ")
            .and_then(|f| f.strip_suffix('*'))
        {
            found.extend(
                members
                    .iter()
                    .filter(|member| member.starts_with(family))
                    .cloned(),
            );
            continue;
        }
        let word = part.split_whitespace().next().unwrap_or_default();
        if members.contains(word) {
            found.push(word.to_owned());
        }
    }
    found
}

/// Reports every edge one side has and the other does not, and fails if there is any.
pub fn run(args: &Args) -> Result<()> {
    let (members, declared) = declared(&args.root.join("crates"))?;
    let doc = args.root.join("docs/architecture.md");
    let drawn = drawn(
        &fs::read_to_string(&doc).with_context(|| format!("reading {}", doc.display()))?,
        &members,
    )?;

    let undrawn: Edges = declared.difference(&drawn).cloned().collect();
    let stale: Edges = drawn.difference(&declared).cloned().collect();
    report("declared but not drawn", &undrawn);
    report("drawn but not declared", &stale);
    if undrawn.is_empty() && stale.is_empty() {
        println!(
            "{} crates, {} edges: the graph and the manifests agree",
            members.len(),
            declared.len()
        );
        return Ok(());
    }
    bail!(
        "{} edge(s) declared but not drawn, {} drawn but not declared",
        undrawn.len(),
        stale.len()
    );
}

fn report(what: &str, edges: &Edges) {
    if edges.is_empty() {
        return;
    }
    println!("{what}:");
    let mut by_source: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (source, target) in edges {
        by_source.entry(source).or_default().push(target);
    }
    for (source, targets) in by_source {
        println!("  {source} ──> {}", targets.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn members() -> BTreeSet<String> {
        [
            "core-geometry",
            "core-slicer",
            "format-goo",
            "format-sl1",
            "encrust-cli",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    #[test]
    fn a_family_stands_for_every_crate_in_it() {
        assert_eq!(
            names("every format-*, core-slicer", &members()),
            ["format-goo", "format-sl1", "core-slicer"]
        );
    }

    #[test]
    fn an_aside_beside_a_name_is_not_part_of_it() {
        assert_eq!(
            names(
                "format-goo in a test only, ureq (with TLS at the desk)",
                &members()
            ),
            ["format-goo"],
            "a third-party crate is not the graph's business"
        );
    }

    #[test]
    fn a_rule_spanning_several_lines_is_one_rule() {
        let doc = "## The allowed dependency graph\n\n```\nencrust-cli ──> core-geometry,\n               core-slicer\nformat-goo, format-sl1 ──> core-geometry\n```\n";
        let edges = drawn(doc, &members()).expect("the block parses");
        assert_eq!(
            edges,
            [
                ("encrust-cli", "core-geometry"),
                ("encrust-cli", "core-slicer"),
                ("format-goo", "core-geometry"),
                ("format-sl1", "core-geometry"),
            ]
            .into_iter()
            .map(|(source, target)| (source.to_owned(), target.to_owned()))
            .collect()
        );
    }

    #[test]
    fn a_line_with_no_arrow_is_refused() {
        let doc = "## The allowed dependency graph\n\n```\ncore-slicer core-geometry\n```\n";
        assert!(drawn(doc, &members()).is_err());
    }

    #[test]
    fn a_graph_that_is_not_there_is_refused() {
        assert!(drawn("# Architecture\n", &members()).is_err());
    }
}
