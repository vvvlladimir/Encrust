# 0140. A profile carries the machine name its firmware matches

- **Status:** Accepted
- **Date:** 2026-09-29

## Context

Both sliced-file formats we write name the machine the file is for: `.goo` twice, in its
machine-name and machine-type fields, and `.ctb` twice, in the slicer-info and print-
parameters blocks. Elegoo's firmware and the SDCP handshake read that string.

We wrote `PrinterProfile::name`, which is the short label the picker shows: `Saturn 4
Ultra`. A vendor file for the same machine carries `ELEGOO Saturn 4 Ultra`. The
manufacturer is on the profile but not in the string, and gluing the two together would
produce `Elegoo Saturn 4 Ultra`, which matches neither the case nor, on other vendors, the
form the firmware expects.

## Decision

`PrinterProfile::machine_name: Option<String>` holds the exact string a sliced file must
carry, and `PrinterProfile::machine_name()` returns it or falls back to `name`. Both format
writers call that accessor. A profile that says nothing keeps behaving as it did.

Only `elegoo-saturn-4-ultra.toml` sets it, to `ELEGOO Saturn 4 Ultra`, because that is the
one string read out of a file the machine is known to accept. The other shipped profiles
are left alone rather than guessed at.

## Consequences

The picker, the fit report and the window keep showing the short name while the file carries
the long one, which is what both need. Four shipped profiles still write a name no vendor
file was checked against; each is one line of TOML away from correct once someone slices on
that machine and reads the string back.

The signal to revisit is a firmware that matches on something other than a string — a model
id or a capability word — which would make this field the wrong shape rather than the wrong
value.

## Alternatives considered

### Put the vendor string in `name`

One line, no new field. It lost because `name` is what the picker, the fit line and the
window title show, and `ELEGOO Saturn 4 Ultra` in a list of five machines is noise where
`Saturn 4 Ultra` is not.

### Compose it as `"{manufacturer} {name}"`

No new field either, and right for exactly the machines whose manufacturer string happens to
be cased the way their firmware wants. `Elegoo` is not `ELEGOO`, so it is wrong for the one
machine there is evidence about.

### The option that won, and what it costs

A profile field that matters only when it is set, and is unset almost everywhere, so the
failure it prevents stays live for four of the five shipped machines and for every profile a
user writes. It also adds a second name to a profile, which someone will fill in with the
display name and wonder why nothing changed.
