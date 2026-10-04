# 0183. The web build works offline, and the command line completes and remembers its flags

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

The browser build ran only online, isolated by `isolate.js` where a host sent no headers, and
named its source only on the page shown when it failed to start. Under the AGPL a program
served over a network offers its users its source. The command line had no completion, no
man page, no file of default flags and no way to see a shipped profile as TOML. clap refuses
a flag given twice, so defaults cannot simply be prepended to the arguments.

## Decision

- **Offline.** `sw.js` replaces `isolate.js`. It keeps every file of the build in a cache
  named for the build; `cargo xtask web` writes the build id — the version and a hash of the
  files — and the file list into it. A new build waits until every tab of the old one is
  closed; only the first worker takes the page at once. Every response is `no-cache`, since
  the files keep their names. A manifest and the icons make the page installable.
- **Source.** In a browser the title strip links to the tag the build was made from.
- **Completion and man pages.** `encrust completions <shell>` over `clap_complete`;
  `cargo xtask man` over `clap_mangen`, so `xtask` depends on `encrust-cli` for its
  definition.
- **`--config FILE`.** Flags written down in TOML, a table per subcommand. The arguments are
  parsed once; every key whose flag was neither typed nor set by a variable is appended as
  that flag, and the arguments are parsed again.
- **`profiles show <id>`** prints a printer, resin or support profile as TOML, or writes it
  with `-o`.

## Consequences

The window opens offline after one visit, and an update never mixes two builds in one page;
a user who keeps a tab open keeps the old build until it is closed. A written flag behaves
as a typed one: it is checked, completed and shown in errors the same way, but it also wins
over a plate file or a project, which the plan ranked above a config file. Reopen this if a
user needs a config value under a plate file's.

## Alternatives considered

### A prompt in the window when a new build is ready

Kinder than waiting for every tab to close, but it needs a message from the worker into the
window and a control in it, for something that resolves itself on the next start.

### File names carrying a content hash, cached for a year

The usual answer, but the module script and the worker script import each other by name
across threads, and a rename would ripple through `wasm-bindgen`'s output. The service worker
already serves them from a cache.

### `--config` as its own layer under the plate file

The order the plan wrote. It means threading a third source of every value through
`stage`, beside the flags and the plate file, for a precedence nobody has asked for yet.

### Completion and man pages by hand

No dependency, but a script per shell and a page per subcommand, each drifting from clap's
definition with every new flag.

### The option that won, and what it costs

Two dependencies, an `xtask` edge to the command line, and a service worker that must stay
correct about which build owns a page — a mistake there strands a user on an old build or
breaks the page offline. A config file's flags ranking above a plate file can surprise.
