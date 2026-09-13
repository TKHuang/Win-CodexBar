# Security policy

## Supported versions

Only the latest release of Win-CodexBar is supported. Security fixes are made
against the current release line; older releases and the historical upstream
macOS project are not patched.

## Antivirus false positives

Win-CodexBar reads local credential stores so it can report your own usage:
provider CLI config, OAuth tokens, and — when you import them — browser cookies.
Several of those operations are indistinguishable from an infostealer to a
behavioural antivirus engine. Kaspersky in particular reports
`PDM:Trojan.Win32.Generic` and may terminate the process.

This is a false positive, and the project narrows the surface deliberately:

- **Browser cookie stores are read only when you click Import.** Background
  refreshes never open them; the rule is enforced by the `ScanTrigger` type in
  `rust/src/browser/cookies.rs`, and a refresh is refused before browser
  detection runs. See [docs/COOKIES.md](docs/COOKIES.md).
- **Chromium App-Bound Encryption is never bypassed.** Where ABE protects a
  cookie, CodexBar reports it and asks for a manual header instead.
- **Prefer CLI or OAuth sources.** Most providers can report usage from a local
  CLI login or OAuth token without any cookie access at all.

If your scanner flags a release, please report it as a false positive to the
vendor — for Kaspersky, submit the file at
[opentip.kaspersky.com](https://opentip.kaspersky.com). That is the only route
that fixes it for every user rather than just your machine.

Building from source flags more often than a release does: `cargo test` produces
unsigned, zero-reputation binaries in `target\debug\deps\` that exercise every
credential path in one process. Excluding your checkout's `target\` directory in
your scanner is the usual remedy on a development machine.

The project does **not** pack, obfuscate, or otherwise disguise its binaries to
avoid detection. Code signing (see [docs/CODE_SIGNING.md](docs/CODE_SIGNING.md))
is the supported path to publisher trust.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting: open the
[Security tab](https://github.com/nesszer/Win-CodexBar/security) and click
"Report a vulnerability".

Do **not** open a public GitHub issue for a vulnerability. Public issues and
the bug report template are for non-security problems only.

Win-CodexBar handles provider cookies, OAuth tokens, and API keys locally, so
reports touching that surface — credential extraction, storage, redaction, or
leakage — are taken seriously. Please keep report details private and do not
paste secrets, cookies, or tokens into any report.
