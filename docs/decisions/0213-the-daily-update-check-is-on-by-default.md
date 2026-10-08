# 0213. The daily update check is on by default

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

[ADR 0172](0172-the-window-offers-a-signed-update-and-never-applies-one.md) left the daily
look at the release feed off until the user found the switch in Settings › Updates, to keep
the promise that the window makes no call the user did not ask for.

What that costs is now visible: a fix reaches only the users who turned the switch on or who
come back to the release page by themselves. The builds are not code-signed yet, so an
install that stays behind also stays on the older warning path, and every security fix in the
readers of files somebody else wrote ([ADR 0161](0161-the-readers-are-fuzzed-outside-the-workspace.md))
sits in a release the user never hears about.

The call itself is one request a day for `latest.json`, carrying this build's version and
nothing else, and it downloads nothing: the install is still a click.

## Decision

`UpdatePrefs::check` defaults to **true**, in a fresh profile and in a preferences file
written before the field existed. The switch in Settings › Updates stays where it is and
turns the daily look off; what the look costs is stated beside it. Everything else in ADR
0172 holds: nothing is downloaded, verified or swapped until the user asks.

## Consequences

A user two versions behind sees the badge the day after a release is published without
having found a setting first. The README states the daily look and where it is turned off,
because a window that calls out by itself must say so where the user is told to install it.

The window now makes one request the user did not ask for, which is the promise ADR 0172
kept and this one spends. A browser build makes the same request and can never act on it —
it has no TLS stack and no binary to replace — so its look fails quietly once a day.

Revisit if the feed's host becomes a cost or a privacy complaint arrives: the next step is a
question asked once on first start, not a switch nobody finds.

## Alternatives considered

### Asking on first start instead

A plain question, answered once, and no call before the answer. Rejected for now: it is one
more thing in the way of a first slice, and it is the setting over again for anyone who
clicks it away.

### Leaving it off and advertising the switch

No promise spent. Rejected: a switch in Settings reaches the users who already look after
their install, which is not the ones a fix has to reach.

### On by default, and what it costs

The window contacts GitHub once a day without being asked, so a user on a metered or
isolated network has to turn it off rather than never having been reached, and GitHub sees
the address of every install that has not.
