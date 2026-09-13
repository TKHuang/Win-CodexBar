# Code signing policy

Free code signing of Win-CodexBar releases via SignPath.io, certificate by SignPath Foundation.

> **Status: SignPath Foundation approved — awaiting repository secrets.** The release workflow (`.github/workflows/release.yml`) contains the SignPath signing steps. Signing activates once all four secrets (`SIGNPATH_API_TOKEN`, `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_SIGNING_POLICY_SLUG`) are present on the repo; the signing step is skipped while `SIGNPATH_API_TOKEN` is empty, and the release publishes unsigned with SHA-256 `.sha256` sidecars. See `.signpath/SETUP.md` for the onboarding checklist.

## Project identity

- **Project name:** Win-CodexBar
- **Homepage:** https://github.com/nesszer/Win-CodexBar
- **Source code:** https://github.com/nesszer/Win-CodexBar
- **Releases:** https://github.com/nesszer/Win-CodexBar/releases
- **License:** MIT

## Roles

| Role |
|------|
| Author: Finesssee |
| Reviewer: Finesssee |
| Approver: Finesssee (@Finesssee) |

## Build system

- Releases build on **GitHub Actions**, `.github/workflows/release.yml`, triggered by a canonical `vX.Y.Z` tag push. SignPath verifies build provenance through its GitHub Trusted Build System integration, so the signed artifacts must be produced by that workflow.
- The workflow has two jobs. `build` runs the preflight, provisions pinned prerequisites, and runs `scripts/circleci-release-build.ps1` (which drives `scripts/windows-release-build.ps1`) to produce the installer (`CodexBar-<version>-Setup.exe`), the portable build, the console CLI zip, SHA-256 sidecars, and `release-manifest.json`. It holds **no** publish credential.
- `build` then submits the two installers to SignPath, replaces the unsigned binaries with the signed ones, recomputes every SHA-256 sidecar and the manifest, and asserts `Get-AuthenticodeSignature` returns `Valid`.
- `publish` is gated on the `release` GitHub Environment — configure required reviewers there for the manual approval hold. It is the only job with `contents: write`, and runs `scripts/publish-github-release.ps1` to create or update a **draft** release. That script never finalizes a release.
- Each release-signing request is also approved manually in SignPath by the approver listed above.
- Ordinary PR/`main` CI still runs on CircleCI (`.circleci/config.yml`). Its `release` workflow is dormant — the CircleCI project excludes tag triggers, and `release.yml` no longer calls the CircleCI API.

## Privacy

See [docs/PRIVACY.md](PRIVACY.md) for the project's privacy policy.

## Notes

*The notes below apply once signing is active:*

- Certificates are issued in the SignPath Foundation's name; signed binaries show "SignPath Foundation" as the publisher.
- Every release-signing request requires manual approval per release; no unattended signing is performed.
