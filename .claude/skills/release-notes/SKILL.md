---
name: release-notes
description: Write short, warm, user-facing notes for a release and the command to publish them.
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
2. `git log --format='%B%n----' <previous>..<tag>`; a squash commit's body lists its PR's commits.
3. `gh pr view <n> --json body,author` for each `(#n)`, and `gh release view <tag> --json body`.
4. Where a line does not say what a user sees, read the diff or ADR. Never guess.

## Pick

Readers skim a card in the window, so choose before you write.

- **Three to five highlights**: what a user will notice first or has been asking for.
- The rest only if a user would miss it; a pile of small things becomes one line,
  e.g. "Plus plenty of smaller polish across supports and the slicer."
- Fixed: the three to five bugs people actually hit. Never tests, refactors, CI or bumps.
- Budget: about 200 words, at most 15 bullets, at most four areas. Over it, cut, don't squeeze.

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
