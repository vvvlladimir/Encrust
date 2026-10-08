# 0212. A bug report is handed to the user, never sent

- **Status:** Accepted
- **Date:** 2026-10-08

## Context

The window is in alpha and the people running it hit bugs, but a report is only useful with
the build, the printer and resin profiles and the values the tool panels were set to beside
it. Until now all of that had to be copied into the issue form by hand, and mostly it was
not.

Two things constrain how it gets there. The README promises that nothing leaves the
user's computer — no account, no cloud, no analytics — and the browser build is served from
a German domain, so anything arriving at a server of ours would make us the controller of
whatever it carried under the DSGVO: a legal basis, a privacy notice, processor agreements,
a retention and deletion route, spam defence, and a store of other people's files to
moderate. A sliced-file bug also tempts the user to attach the model, which is often not
theirs to share.

GitHub's issue forms can be prefilled through the query string, but a URL much past eight
kilobytes comes back as `414 URI Too Long`, so the whole report cannot travel in one.

## Decision

`encrust_app::report` composes one markdown file out of the window's own state and nothing
else, and `panels::report` shows it whole before any of it moves. Three buttons hand it
over: the clipboard, a file called `encrust-report.md`, and the `bug.yml` form prefilled
with the short fields that fit under 6000 characters of URL — opening the form copies the
report as well, because the report itself does not fit in one.

The report carries no model, no file name and no printer address. A path quoted in the
failure on the status strip is cut to its extension, `.../*.stl`, an address or host name
in it is cut away altogether — the ones the window knows a machine by, anything shaped like
an address, and a URL that does not point at this project, which keeps only its scheme —
and the plate is stated as counts. Three switches leave out the profiles, the tool values
and those counts.

There is no endpoint, no account, no key, no rate limit and no retention policy, because
nothing is received.

## Consequences

The README's claim holds for reports too, and the browser build needs no legal basis, no
processor agreement and no deletion route for them: the report never leaves the machine the
user is sitting at. Spam, abuse and identity stay GitHub's to handle, as they already are
for every issue.

A report only arrives if the user walks it to the form, so we get fewer of them than a one
-click upload would bring, and none at all from someone who will not open an account. The
sheet is also written inside the window, so a panic that takes the window down leaves no
report. That is the signal to revisit: a class of bug nobody can reproduce because the
sheet could never be opened for it. The answer then is a crash file written beside the
preferences and still handed over by the user, not a send.

## Alternatives considered

### A form of our own behind a Worker

One click, no GitHub account, and the report could carry the model. It also makes this
project a data controller and a host of other people's files, with everything the Context
lists to set up and keep running, and a spam surface to defend. Too much for a pre-alpha
with nobody to run it.

### An error reporter such as Sentry or a self-hosted GlitchTip

It would catch exactly the crashes this does not, automatically. It also sends by default,
which is the opposite of what the README promises, and needs a server or an account in the
loop for a window that otherwise talks to nothing but the printer on the local network.

### `mailto:` with the report in the body

No attachment, a body every mail client truncates at its own limit, and an address
published for spam.

### The option that won, and what it costs

The user does the carrying, including a paste out of the clipboard that the sheet has to
explain. In exchange a report cannot be misdirected, cannot be collected by accident, and
cannot contain something the user did not read first.
