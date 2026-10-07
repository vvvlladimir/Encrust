# 0198. A drainage check is asked for, and the view it needs is the user's

- **Status:** Accepted
- **Date:** 2026-10-07

## Context

ADR 0189 ran the drainage check after every hollow run and every cut, and ADR 0190 had the
turn from no trapped resin to some open the x-ray. Together that makes Hollow do three
things on one press: it cuts the cavity, it cuts the whole plate again at the layer height
to find the pockets, and it turns the model transparent.

What the user sees is a part that was solid and legible becoming a red-lit ghost without
having asked for anything but a wall. The check is a full slice of the plate (ADR 0072), so
on anything bigger than a test coupon it also lands as a wait after every click. ADR 0189
named that wait as the cost it expected to be reopened on.

The check itself is not the problem: the Drain panel already carries a `Check drainage`
button that starts the same scan.

## Decision

Nothing starts a drainage check but the `Check drainage` button. A hollow run, a hole, a
channel and a cleared cut mark the last check as no longer current — `DrainTool::stale` —
and a hollow run also drops the pockets it found, because the cavity they stood in is a new
mesh. Neither starts a scan.

Nothing turns the x-ray on but the view card and the View menu.

What a check found is stated where it is read: the Hollow and Drain panels both carry
`Resin is trapped in N place(s)` as a `notice` — a glyph, a heading and a line on the danger
wash — rather than as a line of coloured text under a subheading.

## Consequences

Hollow hollows. The plate is checked when the user asks, at a moment they chose, and the
model stays the way they were looking at it until they turn the x-ray on themselves.

The window no longer tells an unprompted user that their cavity traps resin, so a plate can
be sliced with resin sealed in it and nothing on screen will have said so. The CLI's report
and `--check-drainage` are unchanged and still say it, and the slice report is where that
belongs. Reopen if a file written with trapped resin in it turns up as a support case.

`HollowTool::take_hollowed` now means "a run has replaced the cavity", which is the only
thing left that invalidates a check on its own.

## Alternatives considered

### Keep the check and drop only the x-ray

The cheapest change: the panel would still say what is trapped, without the model turning
transparent underneath it. Rejected because the wait after every click is the other half of
what makes the tool feel out of the user's hands, and a red cavity nobody can see through
is the mark ADR 0190 was written to replace.

### Run the check, but only on the model that was hollowed

Cuts the wait roughly by the number of models on the plate. Rejected as the same surprise
on a plate holding one model, which is the case the complaint came from.

### The option that won, and what it costs

Drainage is now opt-in, and an opt-in check is one a hurried user skips. The notice is
louder than what it replaced, which buys back some of that only for a user who ran the
check at least once.
