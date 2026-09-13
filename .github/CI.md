# CI — Win-CodexBar

Win-CodexBar separates primary validation, reserve validation, and interaction policy:

- **CircleCI Windows** is the primary PR/push validation path. Its `pr-check`
  job/workflow in `.circleci/config.yml` runs the canonical local-check CI slice.
- **Blacksmith Windows** is manual reserve CI only. `.github/workflows/pr-check.yml`
  has `workflow_dispatch` only and uses the repo-proven 4-vCPU Windows runner.
  Use it for an independent Windows second opinion when CircleCI is degraded or a
  high-risk native change merits extra evidence.
- The lightweight **interaction guard** runs automatically on GitHub's standard
  `ubuntu-latest` runner for untrusted authors. This public repository gets that
  standard GitHub-hosted compute without consuming the Blacksmith allowance.

The primary and reserve Windows jobs deliberately execute the same
`scripts/local-check.ps1 -Slice ci` contract. Neither build job receives a GitHub
write credential; only the approval-gated release publisher receives a
`GH_TOKEN`, and it is scoped by the `release` GitHub Environment.

## CircleCI hosted PR check (primary PR/push gate)

### Workflow — `.circleci/config.yml`

The hosted PR/push validation gate now runs on **CircleCI** as the
`pr-check` job in the `pr-check` workflow (project `TKHuang/Win-CodexBar`),
not on Blacksmith. The CircleCI job delegates the whole check to
`scripts/local-check.ps1 -Slice ci`, so the exact commands it runs are:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
```

Auto-cancel of superseded non-default-branch work is a CircleCI **project setting**
("Auto-cancel redundant workflows", Project Settings -> Advanced), not GitHub
`concurrency` YAML. It is enabled for this project.

CircleCI trigger configuration also lives outside this repository. The verified
2026-09-05 setup has one enabled GitHub App trigger with explicit rules for PR
opened/reopened/synchronize events and default-branch pushes only. Tag pushes are
excluded. That exclusion is now load-bearing for a second reason: releases build
in GitHub Actions, so a CircleCI tag trigger would produce a duplicate — and
unsigned — release pipeline. Do not add overlapping subset triggers or tag rules.

Porting micro-review CI is enforced semantically in `.circleci/config.yml`: a PR
whose **base/target** branch is `port/upstream-*` does not create the `pr-check`
workflow. A `port/micro-*` head opened directly to `main` still receives the full
hosted Windows gate. This uses CircleCI's GitHub App PR base-ref pipeline value,
not a head-branch naming bypass.

### Interaction guard - `.github/workflows/interaction-guard.yml`

The interaction guard runs on GitHub's standard `ubuntu-latest`, not Blacksmith.
It keeps `contents: read`, `issues: write`, and `pull-requests: write` permissions
and skips maintainer-authored events before a runner starts. This preserves the
Blacksmith pool for manual Windows fallback.

## GitHub Actions budget mode

Both the interaction guard and manual Blacksmith reserve job honor
`vars.CI_BUDGET_MODE != 'off'`. `off` is the emergency stop for GitHub Actions
CI helpers; it does not disable the release workflow.

Set `CI_BUDGET_MODE` in **Settings -> Secrets and variables -> Actions ->
Variables**. Do not hard-code it in a workflow.

| Mode | Interaction guard | Blacksmith reserve | Release |
|------|-------------------|-------------------|---------|
| normal | runs when needed | manual only | tag-triggered |
| thin | runs when needed | manual only | tag-triggered |
| off | skip | skip | tag-triggered |

Blacksmith's allowance is now a reserve, not a recurring Win-CodexBar cost. The
interaction guard uses free standard GitHub-hosted public-repo compute; the
Blacksmith Windows workflow runs only when a maintainer deliberately dispatches
it. CircleCI credits/OSS allowance are budgeted independently.

## Release pipeline (GitHub Actions)

Configuration: `.github/workflows/release.yml`, triggered by a canonical
`vX.Y.Z` tag push, on GitHub's hosted `windows-latest` runner.

Releases moved here from CircleCI because SignPath's Trusted Build System
integration verifies build provenance through GitHub (`github-artifact-id`);
artifacts built elsewhere cannot be signed under that policy. See
[docs/CODE_SIGNING.md](../docs/CODE_SIGNING.md).

1. **`build`** (no publish credential; asserts `GH_TOKEN` is absent) checks out
   the tag with full history, runs `scripts/release-preflight.ps1`,
   `scripts/install-release-prerequisites.ps1`, then
   `scripts/circleci-release-build.ps1` — the same scripts CircleCI ran, so the
   six assets and `release-manifest.json` are produced identically.
2. It uploads the two installers, submits them to SignPath, replaces the
   unsigned binaries with the signed ones, recomputes every `.sha256` sidecar
   and the manifest hashes, and asserts `Get-AuthenticodeSignature` is `Valid`.
   With `SIGNPATH_API_TOKEN` unset the signing steps skip and the release
   publishes unsigned, exactly as before.
3. **`publish`** is gated on the `release` GitHub Environment. Configure
   required reviewers there — that is the manual hold the CircleCI
   `release-approval` job used to provide. It is the only job with
   `contents: write` and runs `scripts/publish-github-release.ps1`.

## CircleCI release pipeline (dormant)

Retained in `.circleci/config.yml` as a fallback. Nothing triggers it: the
CircleCI project excludes tag pushes and `release.yml` no longer calls the
CircleCI API. To use it, trigger the pipeline manually via the CircleCI API with
the tag — note that it produces **unsigned** artifacts.

Both jobs use CircleCI's hosted Windows executor (`circleci/windows@5.0`).

1. `release-build` runs only when the workflow tag filter matches exactly
   `^v[0-9]+\.[0-9]+\.[0-9]+$`; branch filters explicitly ignore every branch.
   The preflight rejects PR/branch environment markers as a second boundary.
2. The credential-free build validates the canonical remote, full tag SHA,
   tag-to-SHA identity, protected `main` ancestry, and every project version
   file. It provisions/asserts Node 24.x via the `OpenJS.NodeJS.LTS` winget
   package, the exact pnpm version pinned by `apps/desktop-tauri/package.json`,
   the Rust MSVC target, Git, and Inno Setup 6.
3. It uses a new temporary `WorkRoot`, runs `release-doctor.ps1`, then runs
   `windows-release-build.ps1` with the immutable SHA and `-SmokeInstall`.
   It never uploads. Six assets — `CodexBar-<version>-Setup.exe` and its
   `.sha256` sidecar, `CodexBar-<version>-portable.exe` and its `.sha256`
   sidecar, `CodexBarCLI-v<version>-windows-x64.zip` and its `.sha256`
   sidecar — plus `release-manifest.json` and build logs are
   persisted to the workspace and stored as CircleCI artifacts.
4. `release-approval` is a required manual CircleCI approval job.
5. `release-publish` attaches the workspace and is the only job with context
   `github-release-publisher`. Its `GH_TOKEN` is used by
   `scripts/publish-github-release.ps1` to create or update a **draft** release.
   Exact SHA-256 matches are skipped, mismatches fail, and missing assets are
   uploaded without clobbering. The script never publishes/finalizes a release.

### CircleCI project and context setup

Provider settings are not stored in `.circleci/config.yml`. Verified on 2026-09-05:

1. **Project Setup** has one enabled GitHub App trigger on the primary pipeline,
   limited to PR opened/reopened/synchronize plus default-branch pushes;
   overlapping subset/tag triggers were removed.
2. **Advanced -> Auto-cancel redundant workflows** is enabled.
3. Keep the restricted context named `github-release-publisher`, scoped only to
   this project (and the release job if organization policy supports expression
   restrictions). Add `GH_TOKEN` as a secret; never add it as a project variable
   or to the build job.
4. Use a fine-grained GitHub token for only this repository with **Contents: read
   and write**. Do not grant **Workflows** permission.
5. Protect the `v*` tag namespace with a GitHub ruleset/tag protection policy that
   permits only authorized release maintainers to create canonical `vX.Y.Z` tags.
   Protect `main` and require the `ci/circleci: pr-check` status check.
6. Configure CircleCI notifications and a spending/credit alert appropriate to the
   organization. Do not approve a release until artifacts and manifest are
   reviewed.

The GitHub token is intentionally not available to checkout, preflight,
prerequisite provisioning, release-doctor, build, smoke, or artifact steps.
CircleCI trigger cleanup and auto-cancel are already applied. GitHub `main`
protection and future context/tag-policy changes remain repository-admin scope.

## Cost, retry, and rollback behavior

CircleCI Windows is the recurring integration cost. The config avoids that compute
for PRs targeting `port/upstream-*`; behavioral micro PRs rely on focused local
evidence. A micro-named PR targeting `main` still runs the full hosted gate. The
full job is also used for ordinary PRs and `main` validation. CircleCI's medium
Windows executor is already its smallest managed Windows resource class, so the
main savings levers are clean trigger topology, compile-time filtering, cache reuse,
and auto-cancel.

Blacksmith Windows is manual reserve only. Keep the proven 4-vCPU Windows runner
for reliability. A 2-vCPU experiment was cancelled while queued before it could
prove equivalent availability. Do not make Blacksmith an automatic second copy of
CircleCI; if CircleCI is healthy, one hosted Windows gate is enough.

Reruns are safe: the build is tied to the immutable SHA from the tag and
produces a fresh temporary WorkRoot. If publication stops after some uploads,
rerun the publisher after approval; matching assets are skipped and a same-name
different-hash asset fails rather than being replaced. A final/non-draft release
is never modified by the publisher. Rollback or deletion of a draft/final
release is a deliberate GitHub administrator action, followed by a new tag/SHA
release if replacement is required.

The current smoke scope is `scripts/windows-smoke-install.ps1`: install the
generated Inno Setup installer, verify the expected application/version, then
uninstall it. It does not prove every tray/UI/provider path; those remain
separate Windows/CUA validation work.

## Local release checks

Run the same pure checks and parser-level validation locally:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\release-pipeline.tests.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\install-release-prerequisites.ps1 -AssertOnly
```

The hosted release flow is:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\release-preflight.ps1 -Tag vX.Y.Z -Sha <full-40-char-sha>
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\circleci-release-build.ps1 -Tag vX.Y.Z -Sha <full-40-char-sha>
```

Do not pass an upload switch to `windows-release-build.ps1`; publication is
owned exclusively by the approval-gated, hash-safe publisher.
