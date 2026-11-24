# 0475. A job runs on the standard machine until it is written

- **Status:** Accepted
- **Date:** 2026-11-24

## Context

ADR 0471 says the document and the edits know no printer or resin, and then lets
`Edit::Orient` name both by catalogue id to prove its poses, the standard printer and stock
resin numbers where unset. Read alone, the two lines contradict each other. `encrust-mcp`
also refused a job without a printer and a resin, so an agent asked to cut or look at a model
had to make the user choose a machine first, or choose one for them, which its instructions
forbid. Slicing is the one step whose result is wrong on the wrong machine: the exposure and
the plate are the printer's and the resin's.

## Decision

- **The plate stores no machine.** An edit names a printer and a resin only to prove a pose,
  and only as catalogue ids; unset, it is proved on `STANDARD_PRINTER`
  (`elegoo-saturn-4-ultra`) with the stock resin numbers. This is what 0471 decided; this
  record states it as one rule.
- **A chat client's job may start on the standard machine.** `start_job` takes the printer
  and the resin together or neither; with neither, every check runs with no machine flag, as
  the command line does, and the window keeps its own machine.
- **Writing needs the machine the user prints on.** `write_file` on such a job is refused,
  telling the agent to ask the user for the printer and the resin and to pass both; it slices
  on them. A job started on a named machine writes on it, or on the one `write_file` names.
- `OrientAim` stays `core-edit`'s own enum (ADR 0470): the format's words live there, and
  `core-plate` takes no serde for them.

## Consequences

An agent can open, cut and prepare a model before the user knows the printer, and no file
reaches a printer on a machine nobody named. A job on the standard machine checks its fit on
no plate until it is written, so a model too big for the user's printer is found at
`write_file`. Reopen if a check on the standard machine leads users to a pose or supports their
own machine would reject.

## Alternatives considered

### Ask the agent in the instructions only

No refusal, so an agent that skips the question writes a file for a printer the user may not
own; the exposure is then a guess.

### Keep the printer and the resin required by `start_job`

The agent must ask before anything is done, even to look at a model, or pick one itself.

### The option that won, and what it costs

`write_file` grows two parameters, and a job's fit and exposure are only known on the user's
machine once it is written.
