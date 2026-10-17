# 0297. Read the `.pws` with its own layer codec, named by the extension

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

The `.pws` of the original Photon and Photon S is the version-1 Anycubic container, so a
`.pws` reached `AnycubicReader` through the content sniffer. Its layers are not the
four-bit runs of the other extensions but one-bit passes, as many as the header's
anti-alias level, summed into grey. Read as four-bit runs, a real file's first layer covered
four times the panel and the file was refused. Nothing in the bytes tells the two codecs
apart; only the extension does.

## Decision

`format-anycubic` has a `PwsReader` beside `AnycubicReader`, sharing the container walk and
decoding a layer as the passes the header asks for, 1 to 16; outside that the file is
refused with `FormatError::AntiAliasLevels`. Passes are summed as runs, never as a buffer
of the panel. `core-pipeline` hands a `.pws` to it by name. Encrust reads `.pws` and does not
write it. The codec is in `docs/formats/anycubic.md`.

## Consequences

A `.pws` opens, converts and gives up its resin like any other file. A `.pws` renamed to
something else is sniffed as the Anycubic container and refused as before. Writing one is
its own work, and would start from this decoder.

## Alternatives considered

### Guessing the codec from the bytes

A layer that decodes to exactly the panel under one codec and not the other would tell
them apart most of the time, but not always, and a wrong guess is a print of noise.

### The option that won, and what it costs

A second reader type and a second family in the pipeline for one extension, and a
`.pws` under a wrong name does not open.
