//! The window's text in the user's language. Every string a user reads is a Fluent message
//! in `assets/i18n/<code>.ftl`, compiled in; see docs/decisions/0300 and docs/design/i18n.md.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use serde::{Deserialize, Serialize};
use unic_langid::{LanguageIdentifier, langid};

/// A language the window is written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ru")]
    Russian,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::Russian];

    /// The language's own name for itself, which is never translated: a user lost in a
    /// language they cannot read finds theirs by it.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Russian => "Русский",
        }
    }

    /// The primary subtag of BCP 47, as a system names its languages.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Russian => "ru",
        }
    }

    fn id(self) -> LanguageIdentifier {
        match self {
            Self::English => langid!("en"),
            Self::Russian => langid!("ru"),
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::English => EN,
            Self::Russian => RU,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::English => 0,
            Self::Russian => 1,
        }
    }
}

const EN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/i18n/en.ftl"
));
const RU: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/i18n/ru.ftl"
));

/// The language every frame is written in, as `Language::index`.
static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Writes the window in `choice`, or in the system's language for `None`.
pub fn choose(choice: Option<Language>) {
    let language = choice.unwrap_or_else(system);
    CURRENT.store(language.index() as u8, Ordering::Relaxed);
}

/// The language the window is written in now.
pub fn current() -> Language {
    let index = usize::from(CURRENT.load(Ordering::Relaxed));
    Language::ALL
        .into_iter()
        .find(|language| language.index() == index)
        .unwrap_or_default()
}

/// The system's or the browser's first language if the window is written in it, English
/// otherwise. Read once: a system changes language by logging in again.
pub fn system() -> Language {
    system_tag().and_then(language_of).unwrap_or_default()
}

/// The system's first language as its tag, such as `de-DE`, when the window is not written
/// in it and so shows in English.
pub fn system_untranslated() -> Option<&'static str> {
    system_tag().filter(|tag| language_of(tag).is_none())
}

fn system_tag() -> Option<&'static str> {
    static TAG: OnceLock<Option<String>> = OnceLock::new();
    TAG.get_or_init(sys_locale::get_locale).as_deref()
}

/// The language `tag` names by its primary subtag — a BCP 47 tag such as `ru-RU`, or a
/// POSIX one such as `ru_RU.UTF-8`, whose `C` and `POSIX` are English.
fn language_of(tag: &str) -> Option<Language> {
    let primary = tag.split(['-', '_', '.', '@']).next()?.to_ascii_lowercase();
    if primary == "c" || primary == "posix" {
        return Some(Language::English);
    }
    Language::ALL
        .into_iter()
        .find(|language| language.code() == primary)
}

/// The message `key` in the current language; `tr!` is the way to call it.
pub fn text(key: &str, args: Option<&FluentArgs<'_>>) -> String {
    message(current(), key, args)
}

/// The message `key` in `language`, falling back to English for a message not translated
/// yet, and to the key itself for one that does not exist.
pub fn message(language: Language, key: &str, args: Option<&FluentArgs<'_>>) -> String {
    let catalogues = catalogues();
    for bundle in [
        &catalogues[language.index()],
        &catalogues[Language::English.index()],
    ] {
        if let Some(pattern) = bundle.get_message(key).and_then(|found| found.value()) {
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args, &mut errors);
            if !errors.is_empty() {
                tracing::warn!(key, ?errors, "a message did not format cleanly");
            }
            return text.into_owned();
        }
    }
    tracing::warn!(key, "no such message");
    key.to_owned()
}

/// Every language's messages, parsed on first use.
fn catalogues() -> &'static [FluentBundle<FluentResource>] {
    static CATALOGUES: OnceLock<Vec<FluentBundle<FluentResource>>> = OnceLock::new();
    CATALOGUES.get_or_init(|| Language::ALL.into_iter().map(catalogue).collect())
}

fn catalogue(language: Language) -> FluentBundle<FluentResource> {
    let resource = FluentResource::try_new(language.source().to_owned()).unwrap_or_else(
        |(resource, errors)| {
            tracing::warn!(
                language = language.code(),
                ?errors,
                "a catalogue did not parse"
            );
            resource
        },
    );
    let mut bundle = FluentBundle::new_concurrent(vec![language.id()]);
    // The marks Fluent puts around an argument are glyphs the window's faces do not have.
    bundle.set_use_isolating(false);
    if let Err(errors) = bundle.add_resource(resource) {
        tracing::warn!(
            language = language.code(),
            ?errors,
            "a catalogue repeats a message"
        );
    }
    bundle
}

/// The message `key` in the current language, with named arguments:
/// `tr!("layers-count", count = layers.len())`. The key is always a literal, so a test can
/// find every one the code uses.
macro_rules! tr {
    ($key:literal) => {
        $crate::i18n::text($key, None)
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $value);)+
        $crate::i18n::text($key, Some(&args))
    }};
}
pub(crate) use tr;

#[cfg(test)]
mod tests {
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
}
