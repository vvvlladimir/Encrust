# 0152. The file goes up as one PUT and nothing else

- **Status:** Accepted
- **Date:** 2026-10-01

## Context

PrusaLink offers two ways to take a file. The v1 API, documented in Prusa's own OpenAPI
specification, is one `PUT /api/v1/files/{storage}/{path}` whose body is the file. The
older OctoPrint-compatible API is a multipart `POST /api/files/{storage}`. A machine says
which it has in `capabilities.upload-by-put` on `GET /api/version`; the flag arrived in
PrusaLink 0.7 and, on the SL1, in firmware 1.8.0. A shipping client reads the flag and
implements both arms.

Two things constrain the PUT. The API asks for a `Content-Length`, and `ureq` derives that
from the body it is handed: a `File` gives a length, a `Read` gives a chunked transfer,
and a manually set header is removed. So a reader wrapped to count bytes as they go would
change how the request is framed, into a framing PrusaLink does not accept.

Supporting the POST arm as well means a streaming multipart body. Hand-rolling one into
memory breaks rule 7 — an `.sl1` of a tall model is hundreds of megabytes — and `ureq`'s
own `multipart` feature pulls `mime_guess` in with it.

## Decision

`net-prusalink` sends the file as one PUT whose body is the opened `File`, and implements
the v1 API only. A machine whose `/api/version` does not report `upload-by-put` is refused
by name: the error says which firmware version is wanted.

The transfer therefore reports no progress. `upload` takes a `cancel` closure, asks it once
before the PUT begins, and cannot ask again.

## Consequences

The upload is about forty lines and has no dependency beyond the `ureq` the workspace
already carries. `Content-Length` is right by construction rather than by a header we set
and hope survives.

The window animates its progress bar for a Prusa machine instead of filling it, and its
Cancel button only bites before the transfer starts. `SendJob` carries a flag for whether
the protocol counts bytes, so the label says "Sending" rather than sitting on "Connecting"
for the whole transfer.

A user on SL1 firmware 1.7 or older cannot send at all, and is told to update. That is the
real cost. Reopen this if that turns out to be a common machine rather than a stale one: the
POST arm is then worth `mime_guess`, and `Form::part(Part::file(...))` streams the file
with a length already.

Reopen the progress half if `ureq` grows a body that carries both a length and a reader.
Nothing else here changes if it does.

## Alternatives considered

### Read the whole file into a `Vec<u8>` and send that

`Content-Length` comes out right, a wrapper can count bytes, and nothing about the request
framing is in question. It lost to rule 7: peak memory would grow with the layer count,
which is the one thing every stage of this workspace is written not to do.

### Write the PUT over a raw `TcpStream`

Full control: length, progress, and a cancel that lands mid-transfer. It lost because it
means hand-rolling request framing, chunked response reading and connection handling that
`ureq` already has, for a printer on a LAN — and because the same argument would apply to
every other call in the crate, which would then not use `ureq` either.

### Implement both arms behind the capability flag

Every machine in the field can be sent to, which is the whole point of the feature. It lost
on what the POST arm costs today: either a multipart body in memory or a new dependency,
for firmware that the vendor has itself moved off. The refusal names the version, so a user
who hits it knows exactly what to do.

### The option that won, and what it costs

A send with no progress is a worse send. A 400 MB `.sl1` over Wi-Fi takes minutes, and for
all of them the window can only say that it is sending and cannot be stopped. On a `.goo`
over SDCP the same window fills a bar a megabyte at a time, so the two destinations behave
visibly differently for no reason the user can see.
