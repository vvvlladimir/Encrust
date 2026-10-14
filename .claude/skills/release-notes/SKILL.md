---
name: release-notes
description: Write user-facing notes for a release and the command to publish them.
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
3. `gh pr view <n> --json body` for each `(#n)`, and `gh release view <tag> --json body`.
4. Where a line does not say what a user sees, read the diff or ADR. Never guess.

## Write

- Plain words for someone who never saw the code: what they can now do, what no longer goes
  wrong. No crate, type, ADR, hash or algorithm names.
- One or two opening sentences, then `### <Area>` per large area, then `## ⚡ Faster` and
  `## 🐛 Fixed`. Short `-` bullets, a **bold lead** where one opens a topic, `(#n)` may stay.
- Skip tests, refactors, CI and dependency bumps a user would not notice.
- The window parses it line by line (`crates/encrust-app/src/updates/feed.rs`): no tables, `---`,
  nested lists, images, bare URLs or "Full changelog" line.

## Hand back

The path, the notes, what you left out and why, and this one command with the tag filled in. It
also puts the notes into the release's `latest.json`, which the window shows, so it runs only
once the build has attached that file; each `&&` keeps a failed step from uploading a stale one.

```sh
d=local-docs/release-notes; t=<tag>; rm -f $d/latest.json &&
gh release edit $t --notes-file $d/$t.md &&
gh release download $t --pattern latest.json --dir $d &&
jq --rawfile notes $d/$t.md '.notes = $notes' $d/latest.json > $d/feed.json &&
mv $d/feed.json $d/latest.json && gh release upload $t $d/latest.json --clobber
```
