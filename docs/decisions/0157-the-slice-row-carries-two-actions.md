# 0157. The Slice row carries two actions, not a destination

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

`0138` hung the destination off a caret beside *Slice*, and named its own cost: a user who
never opens that caret does not learn the window can reach their printer. A destination is
also a mode, so the one button in the window means *write a file* on one day and *send*
on another, and the label is the only thing that says which.

Since `0156` a printer profile already states what it is reached over and which machine on
the network it is bound to, so the press has nothing left to choose. What it cannot say is
whether that machine is there: a board that answered a scan last week may be off.

## Decision

The row carries *Slice to .ext*, which always writes a file through the save dialog. Where
the profile states a connection and is bound to a machine, *Send* stands beside it, with a
dot saying whether that machine answered the last scan. The dot is a button: pressing it
scans again. One scan runs by itself when a bound machine is first in hand, so the dot is
answered before it is asked about.

A profile set to `USB only` has one button, and nothing in the window mentions the network.

## Consequences

Both actions are visible at all times, which is what `0138` gave up. Slicing still has one
entry: the two buttons differ only in where the written file goes, so the blocker, the
estimate and the progress bar stay written once. `Slice all plates` no longer disappears,
because the file action no longer competes with a destination.

The dot reports the last answer, not live state — holding a control socket open per
machine for a line of text is still not worth it. It therefore goes stale, and says so on
hover rather than claiming the machine is unreachable.

The format blocker now guards only *Send*: an `.sl1`-only Prusa machine greys that button
out and leaves the file button alone, which is the honest division.

## Alternatives considered

### One button that changes meaning when a machine is bound

The *Send G-code* pattern of the FDM slicers. It lost because writing a file for a stick is
what a bound machine's owner still does when the network is down, and the mode takes it
away for a press.

### Keeping the caret and only adding reachability to it

The smallest change. It lost on the same ground `0138` already conceded: nothing about the
feature is visible until the caret is opened.

### The option that won, and what it costs

Two buttons on one row is less space for the label, and the row now depends on the profile
being bound before *Send* exists at all — a user with a printer on the network and no
binding sees no way to send from the main window, only from its settings.
