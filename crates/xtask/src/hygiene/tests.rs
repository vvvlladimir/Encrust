use super::scan::{Kind, measure};
use super::*;

/// A function `name` of `body` code lines, its signature and closing brace two more.
fn function(name: &str, body: usize) -> String {
    let lines = (0..body)
        .map(|at| format!("    let x{at} = {at};\n"))
        .collect::<Vec<_>>()
        .concat();
    format!("pub(crate) fn {name}(\n    a: u8,\n) -> u8 {{\n{lines}    a\n}}\n")
}

fn kinds(text: &str) -> Vec<(Kind, String, usize)> {
    measure("crates/a/src/b.rs", text)
        .into_iter()
        .map(|found| (found.kind, found.name, found.lines))
        .collect()
}

#[test]
fn function_is_reported_only_past_its_limit() {
    // Signature three lines, closing brace one, `a` one: a body of n lines is n + 5.
    assert!(kinds(&function("short", 75)).is_empty());
    assert_eq!(
        kinds(&function("long", 76)),
        vec![(Kind::Fn, "long".to_owned(), 81)]
    );
}

#[test]
fn blank_and_comment_lines_do_not_count() {
    let padded = function("padded", 75).replace(";\n", ";\n\n    // note\n");
    assert!(kinds(&padded).is_empty());
}

#[test]
fn inline_test_module_is_measured_apart_from_the_file() {
    let body = (0..200)
        .map(|at| format!("    fn t{at}() {{}}\n"))
        .collect::<Vec<_>>()
        .concat();
    let text = format!("fn a() {{}}\n\n#[cfg(test)]\nmod tests {{\n{body}}}\n");
    assert_eq!(kinds(&text), vec![(Kind::Tests, String::new(), 203)]);
}

#[test]
fn declaration_and_one_line_body_are_not_measured() {
    let text =
        "trait T {\n    fn declared(\n        a: u8,\n    );\n    fn short() -> u8 { 1 }\n}\n";
    assert!(kinds(text).is_empty());
}

fn tally(entries: &[(&str, &[usize])]) -> Tally {
    entries
        .iter()
        .map(|(name, lines)| {
            let key = (Kind::Fn, "crates/a/src/b.rs".to_owned(), (*name).to_owned());
            (key, lines.to_vec())
        })
        .collect()
}

#[test]
fn new_or_grown_entry_fails_and_shrunk_is_reported() {
    let baseline = tally(&[("kept", &[100]), ("grew", &[90]), ("split", &[120])]);
    let current = tally(&[("kept", &[100]), ("grew", &[95]), ("fresh", &[81])]);
    let verdict = compare(&baseline, &current);
    assert_eq!(
        verdict.grown.len(),
        2,
        "grew and fresh: {:?}",
        verdict.grown
    );
    assert_eq!(verdict.shrunk.len(), 1, "split: {:?}", verdict.shrunk);
}

#[test]
fn functions_of_one_name_pair_longest_with_longest() {
    let baseline = tally(&[("new", &[120, 90])]);
    let current = tally(&[("new", &[110, 85])]);
    let verdict = compare(&baseline, &current);
    assert!(verdict.grown.is_empty());
    assert_eq!(verdict.shrunk.len(), 2);
}

#[test]
fn an_attribute_between_cfg_test_and_the_module_still_sets_it_apart() {
    let body = (0..700)
        .map(|at| format!("    fn t{at}() {{}}\n"))
        .collect::<Vec<_>>()
        .concat();
    let text = format!("#[cfg(test)]\n#[expect(clippy::expect_used)]\nmod tests {{\n{body}}}\n");
    let found = kinds(&text);
    assert!(
        found.iter().all(|(kind, _, _)| *kind == Kind::Tests),
        "the tests are not counted as the file's code: {found:?}"
    );
}

#[test]
fn block_comment_lines_do_not_count() {
    let comment = (0..700).map(|_| " * words\n").collect::<String>();
    let text = format!("/*\n{comment} */\nfn a() {{}}\n");
    assert!(kinds(&text).is_empty());
}
