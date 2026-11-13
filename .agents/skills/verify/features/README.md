# Win-CodexBar verification map

This directory is the maintained source for verifying what a user sees in the Win-CodexBar desktop app. Read
the index and SKILL.md first, then follow the matching feature file as the recipe.

## Baseline preconditions

- A fresh debug build of the current source (SKILL.md, Launch). doctor.ps1 fails on a stale binary.
- No CodexBar process is running. The installed app is codexbar.exe under
  %LOCALAPPDATA%\Programs\CodexBar; ask the user to quit it from the tray menu, never kill it.
- Each journey starts with launch.ps1 and ends with cleanup.ps1. Cleanup restores the user's
  %APPDATA%\CodexBar top-level files, so journeys may toggle settings through the UI.
- Synthetic usage comes from fixtures/usage-seed.json (Codex 61%/74%, Claude 12%/37%, resets in 2099).

## Driving conventions

- Pick pages by Tauri window label (main, flyout, floatbar), never by CDP list order.
- Click with cdp.mjs click (real pointer events, no focus change). Use invoke only to read state or, where a
  feature file says so, to stand in for a tray-icon gesture that CDP cannot reach.
- Prefer data-* attributes, aria-current, aria-selected and aria-label. Settings toggles have no attribute of
  their own: find them through the text of the element named by aria-labelledby (English UI strings).
- Never drive account add/switch/remove, sign-in, cookie import or API-key edits: those write folders and
  credential files that cleanup does not restore.

## Proof and skip reporting

- Save every action and assertion as <step>.json in the run directory (cdp.mjs output piped through
  Tee-Object), plus native-window.ps1 facts and a PrintWindow capture of the window that changed.
- A setting change is proven by the UI state and by get_settings_snapshot, then by cleanup.json listing
  settings.json as restored.
- Real provider data shows account emails and plan names. Use the seed for screenshots that leave this
  machine, or redact them.
- Report an unreachable path with the command tried and the missing precondition. A tray-icon click proven
  through open_flyout_window is reported as such, not as a tray click.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph on the user-visible behavior, then four H2
sections in this order: Sub-features, How to get to it (user POV), Driving it with cdp.mjs, Gotchas.

## Features

- [Settings navigation](./settings-navigation.md) covers the Settings sidebar panes and provider detail panes.
- [Tray panel](./tray-panel.md) covers the tray-panel flyout: provider switcher, usage cards and menu rows.
- [Float bar](./float-bar.md) covers turning on the always-on-top float bar and what it shows.

Not mapped yet: tray icon pixels and the tray context menu, Usage & Spend, Notifications, Hooks, provider
credential sections, and the codexbar CLI (docs/CLI.md; it honours CODEXBAR_CONFIG, so it can run against a
scratch settings file).
