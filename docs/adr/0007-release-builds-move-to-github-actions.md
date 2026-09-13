# ADR 0007: Release builds move to GitHub Actions for code signing

Date: 2026-09-13
Status: Accepted; supersedes the release-pipeline decision of ADR 0004

## Context

ADR 0004 put release packaging and publication on CircleCI hosted Windows, with
`.github/workflows/release.yml` reduced to a job that POSTs to the CircleCI API
on a `vX.Y.Z` tag. That split has held up well; nothing about it was failing.

What changed is code signing. Releases ship unsigned, which is the single
largest contributor to endpoint security products distrusting the installer —
an unsigned, zero-reputation binary that reads local credential stores is
exactly the shape of an infostealer, and Kaspersky reports
`PDM:Trojan.Win32.Generic` against it. SignPath Foundation approved the project
for free signing.

SignPath's Trusted Build System integration establishes provenance by verifying
that the artifact was produced by a specific build on a supported CI provider.
The GitHub integration does this by consuming the workflow's own
`github-artifact-id`. Artifacts built elsewhere cannot be signed under that
policy, so signing could not be "wired into" release.yml as it stood: no
artifact existed in that workflow to submit.

`.github/workflows/signpath-test.yml` already proved the whole sequence —
preflight, build, upload, sign, replace, publish — on GitHub's hosted Windows
runner behind a manual `workflow_dispatch`.

## Decision

Move the release build to GitHub Actions. `release.yml` now builds on tag push
instead of calling the CircleCI API.

The pipeline keeps ADR 0004's central property — the job that compiles the
binaries never holds a publish credential — by splitting into two jobs:

- **`build`** asserts `GH_TOKEN` is absent, then runs the same scripts CircleCI
  ran: `release-preflight.ps1`, `install-release-prerequisites.ps1`, and
  `circleci-release-build.ps1`. The six assets and `release-manifest.json` are
  produced identically. It submits the two installers to SignPath, replaces the
  unsigned binaries, recomputes every `.sha256` sidecar and the manifest hashes,
  and asserts `Get-AuthenticodeSignature` returns `Valid`.
- **`publish`** is gated on the `release` GitHub Environment, is the only job
  with `contents: write`, and runs the unchanged `publish-github-release.ps1`.

Recomputing the hashes is not incidental: signing changes the bytes, and
`publish-github-release.ps1` treats a digest mismatch as a hard failure.

Signing degrades rather than blocks. With `SIGNPATH_API_TOKEN` unset the signing
steps skip and the release publishes unsigned with sidecars, exactly as before,
so this could land before SignPath onboarding completed.

## Trust boundaries

- **GitHub Actions build:** packaging boundary; no GitHub write secret, no
  release API calls, immutable source resolved from the tag SHA.
- **SignPath:** verifies build provenance via `github-artifact-id` and applies
  its own manual per-release approval policy.
- **`release` environment:** required reviewers reproduce ADR 0004's CircleCI
  `release-approval` hold. **An environment without reviewers configured is not
  a gate** — the job proceeds unattended.
- **`publish` job:** sole release write capability; draft-only, hash-safe,
  idempotent publisher, unchanged from ADR 0004.
- **GitHub administrators:** protect `main` and the `v*` tag namespace and
  manually finalize releases.

## Consequences

- Release compute moves from CircleCI hosted Windows to GitHub hosted Windows.
  CircleCI remains the PR/`main` gate per ADR 0005, so the change is confined to
  the release lane. `CONTEXT.md` and `.github/CI.md` record the new topology.
- The CircleCI `release` workflow in `.circleci/config.yml` becomes a dormant
  fallback. Nothing triggers it: the project's trigger configuration already
  excluded tag pushes, and release.yml no longer calls the API. That exclusion
  is now load-bearing for a second reason — a CircleCI tag trigger would
  produce a duplicate, unsigned release — and `.github/CI.md` says so.
- The approval hold moves from a CircleCI job to a GitHub Environment, so it
  must be configured in repository settings rather than in version-controlled
  config. This is the one boundary from ADR 0004 that is no longer enforced by
  a file in the repository.
- ADR 0004's compiled-output caching analysis no longer applies to releases;
  the GitHub Actions build does not carry caches between runs. Its reasoning
  still describes the dormant CircleCI pipeline.
- SignPath onboarding (four repository secrets, artifact configuration slug
  `codexbar-installer`, Trusted Build System link) remains manual administrator
  setup, as does the `release` environment.
