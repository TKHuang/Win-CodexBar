---
name: verify
description: Drive the real Win-CodexBar desktop app (Tauri 2 + WebView2 tray app) on this Windows machine and capture proof - launch a fresh debug build in proof mode, drive Settings, the tray-panel flyout and the float bar over CDP in the background, read native window facts and pixels, and restore the user's config afterwards. Use after any UI, tray-panel, settings, float-bar, theme or bridge change, or whenever a claim about app behavior needs evidence from the running app instead of unit tests.
---

# Verify Win-CodexBar

The app is one Tauri shell process (codexbar-desktop-tauri.exe in dev builds, codexbar.exe when installed)
hosting several WebView2 pages. Pick a page by its Tauri window label:

| Label | Window title | Surface | Reached by |
| --- | --- | --- | --- |
| main | CodexBar Desktop | Settings in proof mode settings[:tab[:provider]]; tray panel in proof mode trayPanel | launch |
| flyout | CodexBar | Tray-panel flyout: provider switcher, usage cards, menu rows | proof mode popOut, or invoke open_flyout_window |
| floatbar | CodexBar Float Bar | Always-on-top float bar | Settings > Menu > Show Floating Bar |

Drive page content over CDP with scripts/cdp.mjs. Read what CDP cannot see - bounds, topmost, layered, DWM
dark frame, real window pixels - with scripts/native-window.ps1. Never bring a window to the front, send
hotkeys or type with the OS keyboard: the user works on this desktop while you drive.

The feature map is features/README.md. Read it and the feature file before driving.

## Isolation: read this first

- One instance per desktop session. tauri-plugin-single-instance hands any second launch to the running
  process. launch.ps1 refuses to start while any codexbar* process runs (the installed app is
  %LOCALAPPDATA%\Programs\CodexBar\codexbar.exe). Ask the user to quit it from the tray menu; never kill a
  process you did not start. The sandbox may not see the user's process: run launch.ps1 outside it.
- No private data dir. The desktop app ignores CODEXBAR_CONFIG (only the CLI honours it) and always uses
  %APPDATA%\CodexBar and %LOCALAPPDATA%\CodexBar. launch.ps1 backs up the top-level files of
  %APPDATA%\CodexBar (settings.json, api_keys.json, manual_cookies.json, window_geometry.json, markers) and
  cleanup.ps1 restores any whose hash changed. Account folders, history and logs are not backed up: never
  drive account add/switch/remove, sign-in, cookie import or API-key edits.
- Real providers refresh with the user's credentials while the app runs, and the UI shows account emails and
  plans. Keep such captures local, or use the seed (fixtures/usage-seed.json) for evidence that leaves the
  machine.
- One CDP port per lane (default 9333); the single-instance rule still allows only one lane at a time.

## Launch

Build once per source change, from the repo root (PowerShell, outside the sandbox: the Cargo target lives
under %LOCALAPPDATA%):

    . .\scripts\worktree-env.ps1
    pnpm --dir apps/desktop-tauri run tauri:build:debug

The binary is "$env:CARGO_TARGET_DIR\debug\codexbar-desktop-tauri.exe", outside the worktree; an incremental
build takes about 2 minutes. scripts/dev.ps1 looks under the repo's target\ and will not find it; raw
cargo build --release can embed the dev URL.

Start a run (outside the sandbox: it starts a GUI app and writes %APPDATA%):

    powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/verify/scripts/launch.ps1 -ProofMode settings:general -Name <feature>

Parameters: -ProofMode, -SeedUsageJson <absolute path>, -CdpPort (9333), -Name (run dir suffix), -Exe (another
binary), -ReadyTimeoutSec (60). It prints the run directory on its last line, for example
.proof\verify\20261113-145449-settings-navigation, and writes run.json there (PID, port, exe, config hashes).
It returns once any labelled page has rendered #root - usually main - so wait for the page you need. A launch
that never renders tears itself down.

CODEXBAR_PROOF_MODE values (src-tauri/src/proof_harness.rs; tab ids checked by src-tauri/src/surface_target.rs):
trayPanel; popOut; settings; settings:<tab> with tab general, providers, notifications, menuBar, menu,
usageSpend, advanced, hooks or about; settings:providers:<providerId>. Proof mode shows the surface without
activating it and suppresses blur-dismiss. An invalid value is ignored and the app starts hidden.

CODEXBAR_SEED_USAGE_JSON: fixtures/usage-seed.json holds synthetic Codex and Claude snapshots. A seed stays
only while it covers every enabled provider and no forced refresh runs; opening the tray panel with Refresh
when the menu opens turned on forces one (features/tray-panel.md shows the workaround). Each seed snapshot
needs "errorState": "ready", or the backend marks it Unknown.

## Doctor

Run before the first drive, after anything surprising, and after every failed drive:

    powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/verify/scripts/doctor.ps1 -RunDir <runDir>

DOCTOR OK means: the recorded PID is alive and is the recorded exe; the binary is newer than HEAD and every
edited file under apps/desktop-tauri/src, src-tauri/src and rust/src; the CDP port's listening socket belongs to
our process tree (an msedgewebview2.exe child; its socket reports an empty TCP state, so the check matches
RemotePort 0); at least one page reports a window label. It also notes user config files changed since launch.
DOCTOR FAIL lines name the problem (exit 1). Doctor cannot see a wedged UI on a healthy process: clean up and
launch again instead of driving on.

## Drive

    node .agents/skills/verify/scripts/cdp.mjs 9333 list
    node .agents/skills/verify/scripts/cdp.mjs 9333 click main ".settings-sidebar__pane[data-pane-id='about']"
    node .agents/skills/verify/scripts/cdp.mjs 9333 wait main "document.querySelector('.settings-sidebar__pane[aria-current=page]')?.dataset.paneId === 'about'"
    node .agents/skills/verify/scripts/cdp.mjs 9333 eval main "document.title"
    node .agents/skills/verify/scripts/cdp.mjs 9333 invoke main get_settings_snapshot
    node .agents/skills/verify/scripts/cdp.mjs 9333 shot main <runDir>\main-cdp.png

- click dispatches Input.dispatchMouseEvent at the element's centre: a real pointer path through React, with no
  OS cursor movement and no focus change. The target is a CSS selector or js:<expression returning an element>
  (needed for Settings toggles, which are found by the text their aria-labelledby points at).
- wait polls an expression until it is truthy (default 15 s, optional last argument in ms) and prints it.
- invoke calls a Tauri command through window.__TAURI_INTERNALS__.invoke. Use it to read state
  (get_settings_snapshot, get_cached_providers, get_app_info) and, where a feature file says so, to stand in for a
  tray-icon gesture. Setting state through invoke is not proof of a user path.
- PowerShell quoting: wrap the expression in double quotes and use single quotes inside it.

Native facts and pixels (outside the sandbox):

    powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/verify/scripts/native-window.ps1 -RunDir <runDir>
    powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/verify/scripts/native-window.ps1 -RunDir <runDir> -Title "CodexBar Float Bar" -Out <runDir>\floatbar-native.png

The first form lists the process's visible top-level windows (title, physical bounds, topmost, layered,
toolWindow, darkFrame from DWMWA_USE_IMMERSIVE_DARK_MODE). The second captures one window with
PrintWindow(PW_RENDERFULLCONTENT): the window's own pixels even when other windows cover it, without raising it.
AGENTS.md prefers cua-driver screenshots for PRs; when %LOCALAPPDATA%\Programs\Cua\cua-driver\bin\cua-driver.exe
exists use it, otherwise attach the PrintWindow capture and say so.

## Evidence

Everything goes in the run directory under .proof\verify\ at the repo root (untracked; cleanup never deletes
it). Pipe each command through Tee-Object so the action and its result are both kept:

    node .agents/skills/verify/scripts/cdp.mjs 9333 click main "<target>" | Tee-Object -FilePath <runDir>\01-click.json

A proof has: the action (click output), the resulting state (wait/eval output), the persisted value when the
feature saves anything (get_settings_snapshot), native facts and a PrintWindow capture of the window that
changed, and cleanup.json showing which user files were restored. CDP shot images are layout evidence only;
they never prove theme, DWM frame, z-order or transparency. Exercise the path the user takes; the seed is the
only sanctioned mock and replaces provider data at the bridge boundary.

## Cleanup

    powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/verify/scripts/cleanup.ps1 -RunDir <runDir>

Stops only the PID in run.json (and WebView2 children it left behind), restores every top-level
%APPDATA%\CodexBar file whose hash changed, lists new files it left in place, deletes the scratch backup in
%TEMP%\codexbar-verify-<stamp>, and writes cleanup.json. Run it after every failed iteration as well; it is safe
to run twice. Afterwards check that the evidence files are still in the run directory. The user's own CodexBar
is not restarted: tell the user to start it again if you asked them to quit it.

## Helpers

| Script | Run with | Does |
| --- | --- | --- |
| scripts/launch.ps1 | powershell -NoProfile -ExecutionPolicy Bypass -File ... | refuses on a running CodexBar, backs up config, starts the build with proof mode and CDP, waits for a rendered page |
| scripts/doctor.ps1 | same, -RunDir | read-only health check, exit 0/1 |
| scripts/cdp.mjs | node ... <port> <command> | list, ready, eval, wait, click, invoke, shot |
| scripts/native-window.ps1 | same, -RunDir [-Title -Out] | native window facts and PrintWindow capture |
| scripts/cleanup.ps1 | same, -RunDir | teardown, config restore, evidence check |
| fixtures/usage-seed.json | -SeedUsageJson <absolute path> | synthetic Codex (61%/74%) and Claude (12%/37%) usage |

Requirements: Windows PowerShell 5.1+, Node 22+ (built-in WebSocket and fetch), the debug build above.
