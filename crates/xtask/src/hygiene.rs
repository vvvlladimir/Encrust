//! Size limits held by a ratchet (`.claude/rules/code-style.md`): a function, a file or an
//! inline test module past its limit fails unless the baseline already lists it at that size.

mod scan;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use clap::Args as ClapArgs;

use scan::{Finding, Kind};

/// The debt the ratchet holds: what was over its limit when the rule came, and may only shrink.
const BASELINE: &str = "crates/xtask/hygiene-baseline.txt";

#[derive(ClapArgs)]
pub struct Args {
    /// The repository to read, for running the check from somewhere else.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Checks only these files, as the hook after an edit does; what shrank is not reported.
    #[arg(long = "file", conflicts_with = "bless")]
    files: Vec<PathBuf>,
    /// Rewrites the baseline to what shrank or went; refuses anything new or grown.
    #[arg(long)]
    bless: bool,
}

/// Measures past their limits by kind, file and function name, longest first: a file may hold
/// two functions of one name.
type Tally = BTreeMap<(Kind, String, String), Vec<usize>>;

/// What the tree did against the baseline.
#[derive(Debug, Default)]
struct Verdict {
    grown: Vec<String>,
    shrunk: Vec<String>,
}

pub fn run(args: &Args) -> Result<()> {
    let root = fs::canonicalize(&args.root)
        .with_context(|| format!("resolving {}", args.root.display()))?;
    let baseline_path = root.join(BASELINE);
    let scoped = !args.files.is_empty();
    let paths = if scoped {
        args.files
            .iter()
            .filter_map(|file| relative(&root, file))
            .collect()
    } else {
        sources(&root)?
    };
    let current = tally(&root, &paths)?;

    if !baseline_path.exists() {
        if args.bless {
            return write_baseline(&baseline_path, &current);
        }
        bail!("no {BASELINE}: write it once with `cargo xtask hygiene --bless`");
    }
    let mut baseline = read_baseline(&baseline_path)?;
    if scoped {
        baseline.retain(|(_, path, _), _| paths.contains(path));
    }
    let verdict = compare(&baseline, &current);
    report(&verdict, scoped);
    if !verdict.grown.is_empty() {
        bail!(
            "{} over its limit: split it, see .claude/rules/code-style.md",
            verdict.grown.len()
        );
    }
    if args.bless {
        return write_baseline(&baseline_path, &current);
    }
    if !scoped && !verdict.shrunk.is_empty() {
        bail!("the baseline can tighten: run `cargo xtask hygiene --bless`");
    }
    Ok(())
}

/// Every source the limits hold: `crates/*/src`, test files apart.
fn sources(root: &Path) -> Result<Vec<String>> {
    let mut found = Vec::new();
    let mut pending = vec![root.join("crates")];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Some(rel) = relative(root, &path) {
                found.push(rel);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// `path` relative to `root` with `/` between parts, if the limits hold it.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let absolute = fs::canonicalize(path).ok()?;
    let rel = absolute.strip_prefix(root).ok()?;
    let rel: Vec<_> = rel.iter().map(|part| part.to_string_lossy()).collect();
    let rel = rel.join("/");
    let held = rel.starts_with("crates/")
        && rel.contains("/src/")
        && absolute
            .extension()
            .is_some_and(|extension| extension == "rs")
        && !rel.ends_with("/tests.rs")
        && !rel.contains("/tests/");
    held.then_some(rel)
}

fn tally(root: &Path, paths: &[String]) -> Result<Tally> {
    let mut tally = Tally::new();
    for path in paths {
        let text =
            fs::read_to_string(root.join(path)).with_context(|| format!("reading {path}"))?;
        for Finding {
            kind,
            path,
            name,
            lines,
        } in scan::measure(path, &text)
        {
            tally.entry((kind, path, name)).or_default().push(lines);
        }
    }
    longest_first(&mut tally);
    Ok(tally)
}

fn longest_first(tally: &mut Tally) {
    for lines in tally.values_mut() {
        lines.sort_unstable_by(|a, b| b.cmp(a));
    }
}

/// Pairs each measure with the baseline's of the same key, longest with longest.
fn compare(baseline: &Tally, current: &Tally) -> Verdict {
    let mut verdict = Verdict::default();
    let keys: std::collections::BTreeSet<_> = baseline.keys().chain(current.keys()).collect();
    for key in keys {
        let was = baseline.get(key).map_or(&[][..], Vec::as_slice);
        let now = current.get(key).map_or(&[][..], Vec::as_slice);
        let what = describe(key);
        let limit = key.0.limit();
        for (at, &lines) in now.iter().enumerate() {
            match was.get(at) {
                None => verdict
                    .grown
                    .push(format!("{what}: {lines} lines, limit {limit}")),
                Some(&old) if lines > old => verdict
                    .grown
                    .push(format!("{what}: {lines} lines, was {old}, limit {limit}")),
                Some(&old) if lines < old => {
                    verdict.shrunk.push(format!("{what}: {old} → {lines}"));
                }
                Some(_) => {}
            }
        }
        for &old in was.iter().skip(now.len()) {
            verdict
                .shrunk
                .push(format!("{what}: {old} → within {limit}"));
        }
    }
    verdict
}

fn describe((kind, path, name): &(Kind, String, String)) -> String {
    if name.is_empty() {
        format!("{} {path}", kind.name())
    } else {
        format!("{} {path}::{name}", kind.name())
    }
}

fn report(verdict: &Verdict, scoped: bool) {
    for line in &verdict.grown {
        eprintln!("over: {line}");
    }
    if !scoped {
        for line in &verdict.shrunk {
            println!("shrank: {line}");
        }
    }
}

fn read_baseline(path: &Path) -> Result<Tally> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut tally = Tally::new();
    for (number, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [kind, path, name, lines] = parts[..] else {
            bail!("{BASELINE}:{}: expected `kind path name lines`", number + 1);
        };
        let kind = Kind::parse(kind)
            .with_context(|| format!("{BASELINE}:{}: no kind `{kind}`", number + 1))?;
        let lines = lines
            .parse()
            .with_context(|| format!("{BASELINE}:{}: `{lines}` is no count", number + 1))?;
        let name = if name == "-" { "" } else { name };
        let key = (kind, path.to_owned(), name.to_owned());
        tally.entry(key).or_default().push(lines);
    }
    longest_first(&mut tally);
    Ok(tally)
}

fn write_baseline(path: &Path, tally: &Tally) -> Result<()> {
    let header = "# Written by `cargo xtask hygiene --bless`. Each line may only shrink or go: a split\n\
         # that brings one under its limit is blessed, and nothing new is ever added here.\n";
    let entries: Vec<String> = tally
        .iter()
        .flat_map(|((kind, file, name), counts)| {
            let name = if name.is_empty() { "-" } else { name };
            counts
                .iter()
                .map(move |lines| format!("{} {file} {name} {lines}\n", kind.name()))
        })
        .collect();
    fs::write(path, header.to_owned() + &entries.concat())
        .with_context(|| format!("writing {}", path.display()))?;
    println!(
        "{} entries in {BASELINE}",
        tally.values().map(Vec::len).sum::<usize>()
    );
    Ok(())
}
