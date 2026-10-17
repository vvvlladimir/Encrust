# 0300. The window speaks the user's language through Fluent catalogues

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

Every string of the window was an English literal at its call site, some 1,300 of them
over 120 files, with plurals built by hand (`if n == 1`, `"model(s)"`). The first language
to add is Russian, which has three plural forms (1 слой, 2 слоя, 5 слоёв), so a catalogue
that only maps keys to strings cannot carry it. The window also runs in a browser
(ADR 0181), where nothing can be read from disk at start, and on threads, where a job
writes the status line. egui draws every frame, so a language switched in the settings
needs no restart if every string is looked up as it is drawn. IBM Plex, compiled in
(`ui::fonts`), already covers Cyrillic.

## Decision

The window's text is a set of Fluent catalogues, `assets/i18n/<code>.ftl`, one a
language, compiled in with `include_str!`. `encrust-app` reads them through
`fluent-bundle` 0.16 (with `unic-langid` 0.9 for the language tags) in `i18n.rs`: a
`concurrent::FluentBundle` a language, parsed on first use, with isolation marks off, and
a `tr!("key", name = value)` macro whose key is always a literal. A message missing from a
translation falls back to English, and a key missing from English reads as itself.

The language in use is a global the frame sets from `General::language` in
`preferences.json`: a language the user chose, or `None` for the system's, which
`sys-locale` 0.3 reads once — `navigator.languages` in a browser, through its `js`
feature. The first of the system's languages the window has wins, and English otherwise.
The General page offers the system's choice and every language, each named in itself.

English is the source: a test checks that every key the code names is in `en.ftl`, that
every message there is used, and that a translation names only messages and arguments
English has. `cargo xtask i18n <code>` prints what a translation still lacks, as Fluent
to paste and translate. `fluent-syntax` is a dev-dependency of `encrust-app` for that test.

## Consequences

A translator edits one file and never Rust; plurals, cases and word order are theirs to
choose per language. A new language is a file and one variant of `Language`. Every new
string in the window goes through `tr!`, which `.claude/rules/code-style.md` now says.

What stays English, and why, is listed in `docs/design/i18n.md`: the command line, the
error text of the core crates under the window's own context line, names a project or a
printer stores, and the bug report, which is read by whoever fixes the bug. Numbers keep the
point as their decimal mark, because the number fields parse only that.

Reopen if a core error has to read in the user's language, which needs the window to word
each variant itself, or if a translation needs a number formatted by the locale.

## Alternatives considered

### A key-value catalogue crate, YAML compiled in

The simplest call site and compile-time embedding. Rejected: it has no plural rules, so
Russian would go back to hand-built plurals in the code.

### Catalogues keyed by the English text (`.po`)

No ids to invent, and a format translators know. Rejected: changing an English sentence
orphans every translation of it, plurals are positional, and arguments are unnamed.

### Fluent, and what it costs

Three dependencies, a thousand ids to keep apart, and no compile-time check that a key
exists, which the test stands in for. A sentence built from fragments, such as a tool's
scope set into its tooltip, still constrains the cases a translation may use.
