use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};

use super::*;

/// Every message in `source` and the variables it reads, its variants' included.
fn messages(source: &str) -> BTreeMap<String, BTreeSet<String>> {
    let resource = fluent_syntax::parser::parse(source).expect("a catalogue parses");
    resource
        .body
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Message(message) => Some(message),
            _ => None,
        })
        .map(|message| {
            let mut variables = BTreeSet::new();
            if let Some(value) = &message.value {
                pattern_variables(value, &mut variables);
            }
            (message.id.name.to_owned(), variables)
        })
        .collect()
}

fn pattern_variables(pattern: &Pattern<&str>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            expression_variables(expression, out);
        }
    }
}

fn expression_variables(expression: &Expression<&str>, out: &mut BTreeSet<String>) {
    match expression {
        Expression::Select { selector, variants } => {
            inline_variables(selector, out);
            for variant in variants {
                pattern_variables(&variant.value, out);
            }
        }
        Expression::Inline(inline) => inline_variables(inline, out),
    }
}

fn inline_variables(inline: &InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match inline {
        InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_owned());
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            for argument in &arguments.positional {
                inline_variables(argument, out);
            }
        }
        InlineExpression::Placeable { expression } => expression_variables(expression, out),
        _ => {}
    }
}

/// Every key a `tr!` in this crate's sources names, this module's own examples aside.
fn keys_in_code() -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("the sources are readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && !path.ends_with("i18n.rs")
                && !path.ends_with("i18n/tests.rs")
            {
                let code = std::fs::read_to_string(&path).expect("a source is UTF-8");
                keys.extend(keys_in(&code));
            }
        }
    }
    keys
}

fn keys_in(code: &str) -> Vec<String> {
    code.split("tr!(")
        .skip(1)
        .filter_map(|after| {
            let quoted = after.trim_start().strip_prefix('"')?;
            Some(quoted[..quoted.find('"')?].to_owned())
        })
        .collect()
}

#[test]
fn every_catalogue_parses_and_names_each_message_once() {
    for language in Language::ALL {
        assert!(
            FluentResource::try_new(language.source().to_owned()).is_ok(),
            "{} does not parse",
            language.code()
        );
        let mut bundle = FluentBundle::new_concurrent(vec![language.id()]);
        let resource =
            FluentResource::try_new(language.source().to_owned()).expect("checked just above");
        assert!(
            bundle.add_resource(resource).is_ok(),
            "{} repeats a message",
            language.code()
        );
    }
}

#[test]
fn a_translation_names_only_messages_and_arguments_english_has() {
    let english = messages(EN);
    for language in Language::ALL {
        for (key, variables) in messages(language.source()) {
            let Some(given) = english.get(&key) else {
                panic!("{} has {key}, which English does not", language.code());
            };
            assert!(
                variables.is_subset(given),
                "{}'s {key} reads {variables:?}, English gives only {given:?}",
                language.code()
            );
        }
    }
}

#[test]
fn every_key_the_code_names_is_in_english_and_every_message_is_used() {
    let english: BTreeSet<String> = messages(EN).into_keys().collect();
    let used = keys_in_code();
    let missing: Vec<_> = used.difference(&english).collect();
    assert!(
        missing.is_empty(),
        "named in code, missing from en.ftl: {missing:?}"
    );
    let unused: Vec<_> = english.difference(&used).collect();
    assert!(unused.is_empty(), "in en.ftl, named by no code: {unused:?}");
}

#[test]
fn a_tag_names_a_language_by_its_primary_subtag() {
    assert_eq!(language_of("ru-RU"), Some(Language::Russian));
    assert_eq!(language_of("ru_RU.UTF-8"), Some(Language::Russian));
    assert_eq!(language_of("EN-gb"), Some(Language::English));
    assert_eq!(
        language_of("C.UTF-8"),
        Some(Language::English),
        "POSIX default"
    );
    assert_eq!(language_of("de-DE"), None, "not translated");
    assert_eq!(language_of(""), None, "a system that names none");
}

#[test]
fn an_unknown_key_reads_as_itself() {
    assert_eq!(
        message(Language::Russian, "no-such-key", None),
        "no-such-key"
    );
}
