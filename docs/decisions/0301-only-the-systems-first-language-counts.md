# 0301. Only the system's first language counts

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

ADR 0300 took the first of the system's languages the window has. A system set to German
that also lists Russian lower down then showed the window in Russian, a language its user
did not choose for it, and the General page offered no word on why. A test print's STL was
also named by its translated title, so a file a user saved could carry spaces and Cyrillic
that some printers and file systems refuse.

## Decision

Only the system's or the browser's first language is read, by `sys_locale::get_locale`. If
the window is written in it, it is used; otherwise the window is English, and the General
page says under the language choice that the system's language, named by its tag, is not
translated yet. A POSIX `C` or `POSIX` locale is English. A test print's file name is a
fixed English one with no spaces, the same in every language; its title stays translated.

## Consequences

A user sees the language they put first, or English with the reason. A user who reads
a translated language that is not their system's first picks it once in Settings. The tag
(`de-DE`) is shown rather than a language name, since the window carries names only for
the languages it has.

## Alternatives considered

### Keep the first system language the window has

Right for a user who lists a second language they read, wrong for one who does not, and
silent about it either way; a one-time choice in Settings serves the first user as well.

### The option that won, and what it costs

A user whose first language is untranslated but whose second is gets English until they
choose, and the hint names a tag rather than a language.
