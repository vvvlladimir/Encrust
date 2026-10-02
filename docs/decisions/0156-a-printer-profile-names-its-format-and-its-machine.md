# 0156. A printer profile names its format, and is bound to its machine

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

Two things sat beside the Slice button that are properties of the machine, not of the press:
the container to write, as a list of eight radio buttons, and the whole setup of where the
file goes — a network scan, a field for an address, and four fields for a Prusa login. A user
with one printer walked that menu on every run to reach the same two answers.

Both are already decided by the machine. `PrinterProfile::output` has named the container
since ADR 0140, and `set_printer` has applied it; the radio list could only disagree with it,
and the preferences file remembered the disagreement in `format` and reapplied it over the
profile on the next run. Which machine on the network a user sends to is just as fixed: a
profile is one printer, and that printer is one board or one host.

What a machine speaks is not the same as how it is reached. ADR 0153 put the host and the
credentials in the window's own settings, because a profile is shipped, shared and copied,
and a password is none of those things.

## Decision

`PrinterProfile` gains `connection`: `none`, `sdcp` or `prusa-link` — what the machine takes
a file over, beside `output`, which says what it takes. Both are edited in the printer's form
in Settings, `output` over all eight containers the window writes.

Which machine on the network is a binding held by the window, keyed by catalogue id:
`Network::bound` maps a printer profile to a `Destination`. Choosing one for the profile in
hand binds it; `apply_printer` reads the binding back, so switching profiles switches
machines. The binding and the boards it names go to `preferences.json`, and `format` comes
out of it. A board is written there only when a profile is bound to it, and a scan merges
over the boards already listed rather than replacing them, so a machine that is switched off
is still the machine this profile prints on.

Beside the Slice button there remains the one choice that is neither: a file or that
printer's machine, and the way into the settings. This replaces the format list and the
setup fields of ADR 0138's caret menu; everything else it decided stands, the second press
that starts a print included.

## Consequences

A user who has set a printer up once presses one button. The container cannot disagree with
the machine any more, because there is only one place that states it, and the profile carries
it to anyone it is shared with.

A machine's connection now travels in the profile, so a shipped profile can be wrong about
it — the two Elegoo Ultras ship as `sdcp` and the other three as `none`. Being wrong is
visible and one click to fix, and no credential ever enters a profile.

A one-off export in another container is no longer two clicks: it is an extension typed into
the save dialog, which has had the last word on the format since ADR 0047, or an edit to the
profile. The signal to reopen this is a user who regularly writes two containers for one
machine.

A board nobody chose is still dropped between runs, as ADR 0153 had it. A bound board is not,
which means its address can go stale; a scan corrects it, and until then a send fails naming
the address it tried.

## Alternatives considered

### The host and credentials in the profile TOML

It would put everything about a machine in one file, and the binding would need no second
store. It lost on ADR 0153's grounds, which have not changed: profiles are shipped and shared,
the user directory is a place a user copies files into, and a password written there leaks the
first time a profile is sent to someone.

### The destination left beside the Slice button, unbound

The smallest change: keep the menu, only drop the format radios. It lost because the menu is
where the clicking was. A destination that is not bound to anything has to be chosen again
whenever the window forgets it, which is every run and every profile switch.

### The option that won, and what it costs

The destination is now two screens from the press that uses it: a user who sends to a
different machine today opens Settings to say so. That is the right default for one printer
and the wrong one for a workshop with several, and the quick list beside the button is there
for exactly that case — it switches between what is already set up, and nothing else.
