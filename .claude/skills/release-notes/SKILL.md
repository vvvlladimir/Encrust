---
name: release-notes
description: Write short complete, warm, user-facing notes for a release and the command to publish them.
argument-hint: "[tag]"
disable-model-invocation: true
context: fork
agent: general-purpose
---

Write the notes of Encrust release `$ARGUMENTS` for people who print with it, into
`local-docs/release-notes/<tag>.md`. Change nothing else; the user publishes.

## Gather

1. Tag: the one given, else the newest draft in `gh release list --json tagName,isDraft`, else
   the newest release. `git fetch --tags` first. The previous tag is the release before it.
2. `git log --format='%B%n----' <previous>..<tag>`; a squash commit's body lists its PR's
   commits — read every line of it, a PR title hides most of what it ships.
3. `gh pr view <n> --json body,author` for each `(#n)`, and `gh release view <tag> --json body`.
4. `git diff --word-diff=plain <previous> <tag> -- docs/features.md docs/cli.md`: every added
   or changed sentence is something a user sees. This is the inventory; the log only adds to it.
5. Where a line does not say what a user sees, read the diff or ADR. Never guess.

## Cover everything

The notes are the release's whole changelog, not a teaser.

- Every user-visible change goes in: each inventory item and each `feat`/`fix` commit is a
  bullet, or is named in a "Plus …" line of small polish. Nothing seen by a user is left out.
- Leave out only what no user sees: tests, refactors, CI, release bumps, internal tooling.
- Lead with what a user notices first; as many areas as the release has.
- Length follows the release: about 200 words is a guide for a small one, never a reason to cut.
- Before handing back, tick the inventory and the commit list off against the notes; name
  what was left out and why.

## Voice

Write like a maker who loves this community, telling friends what's new over a print.

- "You" and "we", active voice, contractions. Lead with what they can now do, not how.
- Plain words for someone who never saw the code: no crate, type, ADR, hash or algorithm.
- One idea per bullet, one line, ideally under 15 words.
- Warm, never hype: no "excited to announce", "revolutionary", "seamless"; one `!` at most.
- Open with one or two sentences on the release's heart. Close with one line of thanks:
  to outside contributors by `@login` (from `author`, never the maintainer), bug reporters,
  everyone who prints with it,
  and an invite to share prints and problems in the project's issues.

## Format

- `### <Area>` per highlight area, then `## ⚡ Faster`, `## 🐛 Fixed`, and
  `## Before you update` only when a user must act first. The window tints a group by its
  heading: an area green, Faster orange, Fixed and Before you update grey.
- `-` bullets, a **bold lead** of two to four words, `(#n)` may stay.
- The window parses it line by line (`crates/encrust-app/src/updates/feed.rs`): no tables,
  `---`, nested lists, images, bare URLs or "Full changelog" line.

## Hand back

The path, the notes, what you left out and why, and this one command with the tag filled in. It
also puts the notes into the release's `latest.json`, which the window shows, and stamps it with
the time it runs as the release's date, so it runs only once the build has attached that file
and just before the draft is published; each `&&` keeps a failed step from uploading a stale one.

```sh
d=local-docs/release-notes; t=<tag>; rm -f $d/latest.json &&
gh release edit $t --notes-file $d/$t.md &&
gh release download $t --pattern latest.json --dir $d &&
jq --rawfile notes $d/$t.md '.notes = $notes | .published = (now | todate)' \
  $d/latest.json > $d/feed.json &&
mv $d/feed.json $d/latest.json && gh release upload $t $d/latest.json --clobber
```
