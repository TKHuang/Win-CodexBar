# Float bar

The float bar is an always-on-top, transparent strip that shows one pill per provider with its usage and
reset time, colored ok / warn / crit by the quota warning thresholds. The user turns it on in Settings > Menu.

## Sub-features

- floatbar-enable turns the float bar on and persists floatBarEnabled.
- floatbar-pills shows a pill per provider with percent, reset and tone.
- floatbar-native keeps the window topmost and layered (transparent).

## How to get to it (user POV)

- Settings > Menu > Floating Bar > Show Floating Bar.

## Driving it with cdp.mjs

Preconditions:

- launch.ps1 -ProofMode settings:menu -SeedUsageJson <abs path to fixtures\usage-seed.json> -Name float-bar.
- get_settings_snapshot().floatBarEnabled is false at launch (cleanup restores it either way).

- **Turn it on.** Run node .agents/skills/verify/scripts/cdp.mjs 9333 click main "js:[...document.querySelectorAll('input.toggle')].find(i => (document.getElementById(i.getAttribute('aria-labelledby'))?.textContent ?? '').trim() === 'Show Floating Bar')".
  Then node .agents/skills/verify/scripts/cdp.mjs 9333 wait main "window.__TAURI_INTERNALS__.invoke('get_settings_snapshot').then(s => s.floatBarEnabled === true)".
- **Read the pills.** Run node .agents/skills/verify/scripts/cdp.mjs 9333 wait floatbar "[...document.querySelectorAll('.floatbar__pill')].map(p => p.getAttribute('title'))" 20000.
  With the seed: "Codex: 74% used" with class floatbar__pill--warn and "Claude: 37% used" with
  floatbar__pill--ok, both "Resets in 26353d ...". No pill contains .floatbar__warning.
- **Native facts.** Run native-window.ps1 -RunDir <runDir> -Title "CodexBar Float Bar" -Out <runDir>\floatbar-native.png.
  The window reports topmost true and layered true (about 480x48 physical px at the bottom right).
- **Proof.** Save the pill JSON, the facts JSON and the capture, then cleanup.ps1; cleanup.json lists
  settings.json and window_geometry.json as restored.

## Gotchas

- The float bar page is labelled floatbar with URL ?window=floatbar&orientation=horizontal.
- A pill with "!" and title "<Provider>: Usage unavailable" means the snapshot's errorState is not ready - for
  seeds, a fixture missing "errorState": "ready".
- The pills show the weekly lane as used percent by default; style, orientation and lane options live in the
  same Settings section and were not driven by this map.
- The PrintWindow capture of a layered window has no desktop behind it; judge transparency from the layered
  flag, not from the capture background.
