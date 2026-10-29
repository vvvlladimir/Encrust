# 0381. A dependabot pull request regenerates what the lockfile drives

- **Status:** Accepted
- **Date:** 2026-10-29

## Context

Two committed files are generated from `Cargo.lock`: `workspace-hack/Cargo.toml` (ADR-0272) and
`THIRD-PARTY-LICENSES.md` (ADR-0214). CI fails when either is stale. Dependabot edits only the
lockfile, so almost every cargo bump failed both the Linux job and the licence check, about ten
runner-minutes spent to learn that.

A workflow run started by dependabot gets a read-only `GITHUB_TOKEN` and only the secrets stored
as Dependabot secrets. A commit pushed with `GITHUB_TOKEN` starts no new run, so CI would never
check the regenerated tree.

## Decision

- On a dependabot cargo pull request, `ci.yml` first runs `cargo hakari generate` and
  `cargo xtask licenses` on the branch. When either file changes, a second job commits them to
  the branch, and the Linux job of that run is skipped.
- The push uses a token from a GitHub App with `contents: write`, whose client id and private
  key are the secrets `SYNC_APP_CLIENT_ID` and `SYNC_APP_PRIVATE_KEY`, stored for Dependabot as
  well as for Actions. Its push starts the run that checks the tree.
- The same App replaces the personal token `notify-site.yml` used to reach the site repository:
  the App is installed there too, and each job narrows its token to the repository it needs.
- The job that runs cargo holds no secret and hands the two files over as an artifact; the job
  that holds the token checks out afresh and runs nothing from the bumped crates.
- `audit.yml`'s licence check skips dependabot's own commit: the run after the App's push, or the
  sync job finding nothing to change, covers it.
- Security updates are grouped like version updates, into one pull request.

## Consequences

A dependabot pull request is green when the bump is, and costs one short sync run plus one full
run. A commit by the App stops dependabot rebasing the branch on its own; `@dependabot recreate`
starts over and syncs again. The App's key replaces a personal token that expires, but reaches
both repositories, so the App holds `contents: write` and nothing more. Reopen this when Cargo's
`feature-unification` replaces the workspace-hack (ADR-0272) and the licences are the only file
left.

## Alternatives considered

### `pull_request_target`

Gives the token and secrets without an App, but runs with them on a branch whose dependencies
are the change under review.

### A personal access token

Simpler to set up, but tied to one account, expiring within a year, and pushing as that person.

### Check only, regenerate on main after the merge

No token at all, but main would sit out of step with its lockfile between the merge and the fix.

### The option that won, and what it costs

An App to create and a key to keep, two extra jobs in `ci.yml`, and a bot's commit in each bump.
