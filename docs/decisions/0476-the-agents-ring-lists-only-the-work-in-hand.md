# 0476. The agents' ring lists only the work in hand

- **Status:** Proposed
- **Date:** 2026-11-25

## Context

ADR 0465 has the window list every agent from a ring beside the print chip: there while an
agent works or waits for its chat, each agent with its latest three jobs, up to three ended
agents, and the last day's jobs no agent names as *Earlier jobs*. In use, each chat client
starts a server that waits for its chat whether or not it touches a model, so the ring
stood over rows of *Agent · waiting — No model yet*, and the history crowded out the one
model an agent was working on. A click on *Follow agents* also closed the list, which
closed on any click inside it.

## Decision

- **The ring lists a live agent only while it has a model in hand:** the job its running
  tool names, else the latest job its file names, or *Opening a model…* while it starts
  one. Each agent has one row, that job.
- **No history:** an ended or silent agent, an agent with no job and a job no agent names
  are not listed; with none listed the ring is gone.
- **The list closes on a click outside it or a row pressed,** so the switch flips in place.
- The rest of ADR 0465 — the agents' files, following, `show_in_window` — stands.

## Consequences

The ring says what is being worked on now and nothing else. Work done by an ended agent is
no longer reachable from the ring; its job folders stay under the cache, and a chat client
still shows a revision with `show_in_window`. Reopen if users ask for a job of an ended
agent back.

## Alternatives considered

### Show the ring only while a tool runs

Literal to "working" — but a tool runs for a second or two between chat turns and the
folder is read every 2 s, so the ring would blink and rarely be there to press.

### The option that won, and what it costs

An agent waiting for its chat with a model still shows it, which may be finished work; the
earlier jobs of a long chat are not listed.
