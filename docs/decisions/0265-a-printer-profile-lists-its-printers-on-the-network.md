# 0265. A printer profile lists the printers of its model on the network

- **Status:** Accepted
- **Date:** 2026-10-14

## Context

ADR 0156 bound each printer profile to one machine on the network. The binding lived in
`preferences.json`, keyed by catalogue id. That fits one printer per model and fails a
farm: ten identical boards mean ten profiles, or rebinding before every send. A profile
handed to another desk also arrived without its printers, because the binding never left
the window that made it.

## Decision

- `PrinterProfile` gains `machines`, a list of `NetworkMachine`: a name the user gives, the
  address or host, and for an SDCP board its own id. The id still finds the board after it
  moves to another address. The list is TOML (`[[machines]]`), so exporting and importing
  a profile carries it. An empty list is left out of the file, and an empty address is
  refused (`ProfileError::NoAddress`). A Prusa machine's login stays in the window's
  settings (ADR 0153); the profile names only its host.
- The printer form's Network tab lists the profile's printers: name, address, what each
  is doing, and a button to drop it. Under the list are the boards found on the network,
  this model's first, each with a box to tick it onto the list. Two or more of this model
  get an "Add all N of this model" button. A ticked board is named after itself, numbered
  when the name is taken ("3D Printer 2"). A board added by its address learns its id
  once it answers.
- `Network` takes the list from the profile in hand. The Export tool shows one row naming
  the listed printer it sends to. With one printer listed the row is plain; with several
  it opens the others. Which one a profile last sent to is the only network choice left in
  `preferences.json` (`chosen`, by catalogue id, as an address). The boards the profile in
  hand lists are kept there too, so they are named before a scan answers.
- A scan also probes every listed address, so a listed board on another subnet is found.
- The ring in the top bar still lists every printer the window finds, whatever profile.
  Each is called by the name any profile gives it.

## Consequences

Ten printers of one model are one profile and one Network tab, and a profile carries its
printers to another desk. Switching profiles switches lists. Bindings made under 0156 are
not carried over: the old `bound` field is ignored, and each profile's printers are ticked
once.

Editing a shipped profile's list makes it the user's copy, as any other edit does. The
note that the copy differs from the shipped one then appears for a list of printers too.

## Alternatives considered

### Keeping the list in the preferences file

No change to the profile format, and no IP address inside a file that may be shared. It
lost because carrying the printers with the profile is the point: export and import must
bring them along.

### A farm as a profile kind of its own

A separate list of machines that several profiles point at. More general, but nobody has
asked for printers of different models in one list, and it is a second catalogue to keep
in step with the first.

### The option that won, and what it costs

An exported profile now carries the addresses of the user's network. A user sharing a
profile publicly has to clear the list first, and nothing warns them.
