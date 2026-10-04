# PrusaLink, the Prusa HTTP API

How an `.sl1` reaches an Original Prusa machine over the local network, and how a print is
started on it. `net-prusalink` implements this; nothing else in the workspace speaks it.

## Where the knowledge comes from

| Source | What it settles |
|---|---|
| [`prusa3d/Prusa-Link-Web` `spec/openapi.yaml`](https://github.com/prusa3d/Prusa-Link-Web/blob/master/spec/openapi.yaml) | The API itself: every path, header, status code and error object |
| An open-source client of the same API | What a shipping client actually sends, and the two firmware bugs it works around |
| [Prusa's own SL1 API key article](https://help.prusa3d.com/article/sl1-api-key_133977) | Which login a machine offers, and where the user finds it |

The SL1 and SL1S are the machines this matters for: they are the resin printers that print
`.sl1`. A shipping client treats them as PrusaLink with one extra check, and so does this.

## Authentication

HTTP digest, or a single API key in an `X-Api-Key` header. Digest is the default the
firmware generates and the one Prusa recommends; the key is what a machine falls back to
when its digest login has been turned off, and SL1 firmware has since dropped it. A machine
offers one or the other, never both, so the user says which they have.

The username is `maker` on every machine that ships with PrusaLink. The password is under
*Main Menu → Settings → Network → Login Credentials* on an SL1.

Digest costs a round trip: the nonce only exists in the `401` that carries it. That refusal
is collected on the version call below rather than on the upload, so a whole stack is never
sent twice. Every request after counts up in `nc`, because a server is entitled to refuse a
count it has already seen. `qop=auth` is answered when offered and the RFC 2069 form when
not; `auth-int` is not implemented and no shipping firmware asks for it.

## The four calls

### `GET /api/version`

Answers with `api`, `version`, `text`, `firmware` and an optional `capabilities` object.
Three things are read from it:

- `text` must start with `PrusaLink`, `Prusa SLA` or `OctoPrint`. Anything else answering
  on port 80 is named back to the user rather than uploaded to.
- `capabilities.upload-by-put` says whether the file may go up as a PUT. An absent
  capability means no — the specification says that of every capability it does not list.
- `firmware` is what the printer list shows.

### `PUT /api/v1/files/{storage}/{path}`

The body is the file itself; `201 Created` is the answer. `storage` is `local` on an SL1,
`usb` or `sdcard` on a machine that has one. `path` is the file name, percent-escaped one
segment at a time.

Headers: `Content-Type: application/octet-stream`, `Content-Length`, and `Overwrite: ?1` —
RFC 8941 booleans, `?0` and `?1`.

`Print-After-Upload` is **not sent at all**, not even as `?0`. PrusaLink has read any value
in that header as true, so sending false started the print; a shipping client works around the
same bug the same way. Starting a print is the separate call below, which is what ADR 0138
asks for anyway.

A shipping client sends `Content-Type: text/x.gcode` here whatever the file is. The
specification's own default is `application/octet-stream` and PrusaLink keys on the path,
so that is what goes.

### `POST /api/v1/files/{storage}/{path}`

Starts a file already on the storage. The body is ignored; `204 No Content` is the answer.

### `GET /api/v1/status`

What `encrust printer status` reads. `printer.state` is the one required field — `IDLE`,
`BUSY`, `PRINTING`, `PAUSED`, `FINISHED`, `STOPPED`, `ERROR`, `ATTENTION` or `READY` — and
`job`, while there is one, carries `progress` in percent and `time_remaining` in seconds.
The file being printed is not named here; that is `GET /api/v1/job`'s, which is not read.

## What a refusal means

The body of an error is the API's `Error` object — `code`, `title`, `text`, `url` — and the
`title` and `text` are what the user is shown. When a machine answers a bare status code,
these stand in:

| Status | What it means |
|---|---|
| 401 | the credentials were rejected |
| 403 | the credentials are not allowed to do this |
| 404 | the storage or the file is not there |
| 409 | it is printing, or a file of that name is already there |
| 415 | it does not take files of this type |
| 507 | there is no room left on the storage |

## What this is not

There is no discovery. PrusaLink announces itself over mDNS as `_http._tcp`, and this
workspace carries no resolver; a machine is typed in, with its credentials, and remembered
(ADR 0153).

There is no progress inside the transfer. The file is one PUT and `ureq` derives
`Content-Length` from the body it is given, so a reader wrapped to count bytes would go up
chunked instead — which PrusaLink does not take. The window animates its bar rather than
filling it, and a cancel only lands before the PUT begins (ADR 0152).

A page in a browser cannot call any of this. The machine answers no CORS preflight, and a
digest login or an API key is a header that needs one, so the web build only downloads
(ADR 0182).

Older firmware — PrusaLink before 0.7, SL1 before 1.8.0 — has no `upload-by-put` and takes
only the OctoPrint-style multipart `POST /api/files/{storage}`. That is refused here by
name rather than implemented; ADR 0152 says why.
