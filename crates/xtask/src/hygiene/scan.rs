//! What one source file measures: its functions, its code outside the inline test module, and
//! that module, each reported once past its limit.

use std::ops::Range;

/// What a measure is of, with the code lines past which it is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Fn,
    File,
    Tests,
}

impl Kind {
    /// The most lines this kind may hold: a function's and a file's code lines, a test module's
    /// lines of any sort.
    pub fn limit(self) -> usize {
        match self {
            Self::Fn => 80,
            Self::File => 600,
            Self::Tests => 150,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Fn => "fn",
            Self::File => "file",
            Self::Tests => "tests",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        [Self::Fn, Self::File, Self::Tests]
            .into_iter()
            .find(|kind| kind.name() == name)
    }
}

/// One measure past its limit: its kind, its file, the function's name or nothing, its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub kind: Kind,
    pub path: String,
    pub name: String,
    pub lines: usize,
}

/// Every measure of `text`, the file at `path`, past its limit.
pub fn measure(path: &str, text: &str) -> Vec<Finding> {
    let lines: Vec<&str> = text.lines().collect();
    let tests = test_module(&lines);
    let outside = |at: usize| tests.as_ref().is_none_or(|range| !range.contains(&at));
    let mut found = Vec::new();
    let mut push = |kind: Kind, name: &str, count: usize| {
        if count > kind.limit() {
            found.push(Finding {
                kind,
                path: path.to_owned(),
                name: name.to_owned(),
                lines: count,
            });
        }
    };

    for at in (0..lines.len()).filter(|&at| outside(at)) {
        let Some(name) = fn_name(lines[at]) else {
            continue;
        };
        if let Some(end) = body_end(&lines, at) {
            push(Kind::Fn, name, code_lines(&lines[at..=end]));
        }
    }
    let code = (0..lines.len())
        .filter(|&at| outside(at) && is_code(lines[at]))
        .count();
    push(Kind::File, "", code);
    if let Some(range) = tests {
        push(Kind::Tests, "", range.len());
    }
    found
}

/// The lines of an inline `#[cfg(test)] mod tests { … }`, attribute to closing brace.
fn test_module(lines: &[&str]) -> Option<Range<usize>> {
    let start = lines.windows(2).position(|pair| {
        pair[0].trim() == "#[cfg(test)]" && pair[1].trim_start().starts_with("mod tests {")
    })?;
    let close = closing(lines[start + 1]);
    let end = lines[start + 2..]
        .iter()
        .position(|line| *line == close)
        .map_or(lines.len(), |at| start + 2 + at + 1);
    Some(start..end)
}

/// The name of the function `line` opens, if it opens one.
fn fn_name(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    if let Some(scoped) = rest.strip_prefix("pub(") {
        rest = scoped.split_once(')')?.1.trim_start();
    } else if let Some(public) = rest.strip_prefix("pub ") {
        rest = public;
    }
    for qualifier in ["const ", "async ", "unsafe ", "extern \"C\" "] {
        rest = rest.strip_prefix(qualifier).unwrap_or(rest);
    }
    let name = rest.strip_prefix("fn ")?;
    let end = name.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
    Some(&name[..end])
}

/// The line closing the body of the function opened at `start`; `None` for a declaration or a
/// body on one line. Relies on `rustfmt` closing a body at the indent it was opened at.
fn body_end(lines: &[&str], start: usize) -> Option<usize> {
    let open = lines[start..].iter().position(|line| {
        let line = line.trim_end();
        line.ends_with('{') || line.ends_with(';') || line.ends_with('}')
    })? + start;
    if !lines[open].trim_end().ends_with('{') {
        return None;
    }
    let close = closing(lines[start]);
    let end = lines[open + 1..].iter().position(|line| *line == close)?;
    Some(open + 1 + end)
}

fn closing(opening: &str) -> String {
    let indent = opening.len() - opening.trim_start().len();
    format!("{}}}", " ".repeat(indent))
}

fn code_lines(lines: &[&str]) -> usize {
    lines.iter().filter(|line| is_code(line)).count()
}

fn is_code(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !line.starts_with("//")
}
