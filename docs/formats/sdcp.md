# SDCP, the network protocol

How a sliced file reaches a printer over the local network, and how a print is started on
it. `net-sdcp` implements this; nothing else in the workspace speaks it.

Two versions are in the field and both are here. Version 3 is the published one: the board
is the server, the client opens a WebSocket to it and posts the file. Version 1, the
generation before it, inverts the roles — the board connects out to a broker the client runs
and fetches the file from a server the client runs — but the envelope, the commands, the
acknowledgements and the topics are the same, which is why it is the same crate (ADR 0154).

## Where the knowledge comes from

| Source | What it settles |
|---|---|
| [`cbd-tech/SDCP-Smart-Device-Control-Protocol-V3.0.0`](https://github.com/cbd-tech/SDCP-Smart-Device-Control-Protocol-V3.0.0) | The protocol itself: ports, topics, every command and acknowledgement, the upload form |
| [`WalkerFrederick/sdcp-centauri-carbon`](https://github.com/WalkerFrederick/sdcp-centauri-carbon) | What one shipping board actually does with it |
| [`vvuk/cassini`](https://github.com/vvuk/cassini) | Version 1: every packet of it, recorded off a Saturn 3 Ultra |

CBD Technology is the mainboard vendor behind the protocol, so this is what the vendor tools
call *network sending*. Elegoo ships version 3 from the Saturn 4 Ultra and Mars 5 onwards,
and version 1 on the Saturn 3 Ultra and Mars 4 Ultra.

## Version 3: the board is the server

Discovery is UDP, control is a WebSocket, and the file itself goes over plain HTTP. All
three are unencrypted and unauthenticated: anything on the segment can drive the printer.
Discovery is shared with version 1; the two sections after it are not.

### Discovery, UDP port 3000

Broadcast the seven bytes `M99999`. Every board on the segment replies with one datagram.
Version 3 puts the fields straight under `Data`:

```json
{ "Id": "<32-character brand UUID>",
  "Data": { "Name": "...", "MachineName": "...", "BrandName": "ELEGOO",
            "MainboardIP": "192.168.1.42", "MainboardID": "000000000001d354",
            "ProtocolVersion": "V3.0.0", "FirmwareVersion": "V1.2.3" } }
```

Version 1 nests the same fields under `Data.Attributes`, with a `Resolution`, a
`Capabilities` list and a whole `Data.Status` block beside them:

```json
{ "Id": "0a69ee780fbd40d7bfb95b312250bf46",
  "Data": { "Attributes": { "Name": "Saturn3Ultra", "MachineName": "ELEGOO Saturn 3 Ultra",
                            "ProtocolVersion": "V1.0.0", "MainboardID": "ABCD1234ABCD1234",
                            "Capabilities": ["FILE_TRANSFER", "PRINT_CONTROL"] },
            "Status": { "CurrentStatus": 0, "PrintInfo": {}, "FileTransferInfo": {} } } }
```

**That nesting is what decides the transport**, not the version string: a board whose
`ProtocolVersion` we have never seen is still placed by the shape of its own reply.

There is no count to wait for, so a scan is a window rather than a request: collect
replies for a couple of seconds and take what came. The same probe sent to one address
reaches a printer a broadcast does not, which is how another subnet is reached.

`MainboardID` names both topics below and identifies the board across a rescan; `Id` is
echoed back in every request.

### Control, version 3: WebSocket `ws://${MainboardIP}:3030/websocket`

JSON in both directions, each message carrying the topic it belongs to:

```
sdcp/request/${MainboardID}      client -> board, commands
sdcp/response/${MainboardID}     board -> client, one per command, matched by RequestID
sdcp/status/${MainboardID}       board -> client, unprompted, whenever anything changes
sdcp/attributes/${MainboardID}   board -> client, what the machine is and can do
sdcp/error/${MainboardID}        board -> client, unprompted
sdcp/notice/${MainboardID}       board -> client, unprompted
```

A request is an envelope of `Cmd`, a `Data` payload, a `RequestID` to match the answer by,
the board ID, a timestamp and `From: 0` for a client. The commands used here are `0`
(report status), `1` (report attributes) and `128` (start printing, taking a filename and
a start layer). The rest of the command table is in the specification.

Every response carries an `Ack`, and only `0` is success. The other seven say why not:
busy, file not found, MD5 mismatch, unreadable, resolution mismatch, unknown format,
wrong machine. `net-sdcp` spells each of them out rather than reporting a number.

Four details the shape of the client follows from:

1. **Reports arrive whenever the board feels like it**, on the same socket a response
   comes back on. A read loop therefore files status and attributes as they pass and
   keeps reading until the `RequestID` it is waiting for turns up.
2. **Command `0` is answered twice.** The response is a bare `Ack`; the state itself follows
   later on the status topic, so a refresh waits for that report and fails if it never
   comes rather than reading the empty state as idle.
3. **`CurrentStatus` is an array** of the states that are live at once. Firmware in the
   field sends a bare number instead, so both are accepted. `PrintInfo.Status` says what a
   print is doing inside that: homing, lowering, exposing, lifting, pausing, paused,
   stopping, stopped, complete, and `10`, checking a file it has just taken.
4. **A board checks a file after it lands and is busy until it has.** A start sent then is
   answered `Ack 1`, so a start waits out status `10` and asks again on busy, for up to a
   minute; a board printing something else is refused at once.

### Upload, version 3: `POST http://${MainboardIP}:3030/uploadFile/upload`

`multipart/form-data`, one packet of 1 MB per request, every packet of one transfer
sharing a `Uuid`:

| Field | What it holds |
|---|---|
| `S-File-MD5` | MD5 of the whole file, hex, computed before the first packet |
| `Check` | `1` to have the board verify the file when it lands |
| `Offset` | Where this packet starts. The board rejects one that does not continue the file it is holding |
| `Uuid` | Groups the packets of one transfer |
| `TotalSize` | Bytes in the whole file |
| `File` | The packet, with the remote filename in its content disposition |

The reply is JSON with a `success` flag; a failure lists `messages`, where a field named
`common_field` carries a numeric code: `-1` and `-2` are offset complaints, `-3` is the
board failing to open the file.

The file is not printed by being uploaded. Starting it is command `128`, naming the file
that was sent.

## Version 1: the client is the server

### Control, MQTT on a broker the client runs

Bind a listening socket, then send `M66666 <port>` to UDP 3000 of the board. It opens an
MQTT 3.1.1 connection back to the address the datagram came from, on that port, with its
`MainboardID` as the client identifier — a board connecting under any other identifier is
not the board that was asked.

It subscribes to `/sdcp/request/${MainboardID}` and publishes on
`/sdcp/status/`, `/sdcp/attributes/` and `/sdcp/response/`, each suffixed the same way.
**These topics carry a leading slash**; the version 3 spelling of the same topics, the one
that goes in the body, does not.

The envelope is the version 3 envelope with one field dropped and one level added:

- **No `Topic` field in the body.** The topic it was published to is the topic, so a request
  does not carry one and a report is sorted by where it arrived.
- **An unprompted report sits inside `Data`.** Version 3 sends `{"Status": {...}}` at the top
  level; version 1 sends `{"Data": {"Status": {...}, "MainboardID": ..., "TimeStamp": ...}}`.
  A response is the same shape in both.

What a board reports on `/sdcp/attributes/` is its status again, not its attributes. So
version 1 says what it is in its discovery reply and nowhere else, and nothing asks it.

Four commands are sent before anything else, in this order, because the vendor client sends them and
a board that has not had them reports far less often: `0`, `1`, then `512` with
`{"TimePeriod": 5000}`. `64` looks like a disconnect and is not used.

### Upload, version 1: command `256` and a server of our own

Put the file up on an HTTP server of the client's own, under a name nobody can guess, then
send command `256`:

```json
{ "Check": 0, "CleanCache": 1, "Compress": 0,
  "FileSize": 3541068, "Filename": "model.goo",
  "MD5": "205abc8fab0762ad2b0ee1f6b63b1750",
  "URL": "http://${ipaddr}:58883/f60c0718c8144b0db48b7149d4d85390.goo" }
```

`${ipaddr}` is sent **literally**. The board substitutes the address it is connected to,
which saves the client having to work out which of its own addresses the board can see.

The board then fetches the file — a HEAD, then a GET, either possibly more than once — and
reports progress in `Status.FileTransferInfo`: `DownloadOffset` of `FileTotalSize`, with
`Status` `0` while it runs, `2` once the file is there and `3` when it gave up. That is the
only progress there is, so the server keeps the route up until one of those two arrives.

Starting the print is command `128`, the same as version 3.

### Where our broker differs from `cassini`

`cassini` puts a packet identifier in the body of a QoS 0 publish, which the specification
does not allow and which prefixes the JSON with two stray bytes. Ours does not.

### What this costs the user

A board pulling a file needs to reach a listening socket on the machine running the slicer,
on a port the operating system picked. A host firewall that blocks incoming connections
blocks the transfer, and there is nothing in the protocol to fall back to. ADR 0155 covers
why both servers are written here rather than taken as dependencies.
