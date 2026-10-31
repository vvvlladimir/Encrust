# 0387. Name a sliced file by a template the user writes, dated with `jiff`

- **Status:** Accepted
- **Date:** 2026-10-31

## Context

The save dialog opened on the model's name alone, and a file sent to a printer was called
the same. Users keeping many prints apart want the machine, the resin, the layer height or
the date in the name without typing it each time. The window and `encrust batch` must name
alike (rule 9 of `architecture.md`), and a date in the user's own zone needs the system's
time zone, which the standard library does not read.

## Decision

A `General` preference holds a template; `core_engine::file_stem` fills it from
`NameFields`. Placeholders are `{model}`, without the extension of the file it came from,
`{ext}`, that extension, `{printer}`, `{resin}`, `{layer}` (whole µm), `{plate}` (from
one), `{date}` (`2026-10-31`) and `{time}` (`09-05`, no colon). The default is `{model}`, so
`bracket.stl` saves as `bracket.goo`, not `bracket.stl.goo`. The output's extension is
never part of the template.

The filled name is made safe after filling, so a literal and a value alike: `/ \ : * ? " <
> |` and control characters become `_` rather than vanish, so two names stay apart and no
folder is made; trailing dots and spaces go; a Windows device name gets `_`; the stem stops
at 200 bytes between characters; an empty one is `model`. A `{tag}` nothing answers stays
as written and the setting names it, rather than failing.

The window fills the template for the save dialog and for the file sent to a printer, and
shows the name it makes now under the box. `encrust batch --name` takes the same template,
one time for the whole run, and refuses one that gives two models one file. `slice -o`
already names its file in full and takes no template.

`jiff` 0.2 reads the local time: `core-engine` uses only its `civil::DateTime`, the front ends
`Zoned::now`, the browser through its `js` feature.

## Consequences

A name a user would type is made once. A template without `{model}` can still name two
saves alike; the dialog shows the name before it is taken, and a batch refuses. A new
placeholder is one variant of `Tag`, which the compiler then asks a name and a value of,
and one word in the help string.

## Alternatives considered

### Square brackets, or conditionals in the template

Brackets read as well, but braces are what most naming templates use; conditionals and
string functions are a language to maintain for little gain.

### UTC computed by hand, or no date

No dependency, but a UTC time an evening print carries is the next day's date, and a date
is the most asked-for field.

### The option that won, `jiff`, and what it costs

One dependency more and its time-zone database reader at the desk; in a browser it rides
on `js-sys`, which is already there.
