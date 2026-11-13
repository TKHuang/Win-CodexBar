# Tray panel

Left-clicking the tray icon opens the tray-panel flyout: a provider switcher (All providers plus one button per
enabled provider), one usage card per provider with quota bars, pace and reset times, and menu rows (Refresh,
Settings..., About CodexBar, Quit).

## Sub-features

- tray-open shows the flyout with a card per enabled provider.
- tray-switch selects one provider; only its card stays and its button is active.
- tray-usage shows used percent and reset time for each quota window.
- tray-refresh-on-open (Settings > General > Refresh when the menu opens) re-fetches every provider when the panel opens.

## How to get to it (user POV)

- Left-click the CodexBar tray icon.
- Tray context menu > Pop Out Dashboard.
- Global shortcut, when the user configured one in Settings > General.

## Driving it with cdp.mjs

Preconditions:

- launch.ps1 -ProofMode settings:general -SeedUsageJson <abs path to fixtures\usage-seed.json> -Name tray-panel.
- doctor.ps1 reports DOCTOR OK. The seed covers every provider in get_settings_snapshot().enabledProviders
  (codex and claude on this machine); add a snapshot per extra enabled provider first.

- **Keep the seed.** Turn off Refresh when the menu opens, or opening the panel replaces the seed with real
  data. Run node .agents/skills/verify/scripts/cdp.mjs 9333 click main "js:[...document.querySelectorAll('input.toggle')].find(i => (document.getElementById(i.getAttribute('aria-labelledby'))?.textContent ?? '').trim() === 'Refresh when the menu opens')".
  Then node .agents/skills/verify/scripts/cdp.mjs 9333 wait main "window.__TAURI_INTERNALS__.invoke('get_settings_snapshot').then(s => s.refreshAllProvidersOnMenuOpen === false)".
- **Open the panel.** Stand-in for the tray click (CDP cannot click the notification area). Run
  node .agents/skills/verify/scripts/cdp.mjs 9333 invoke main open_flyout_window. A page labelled flyout appears (index.html?window=flyout, window title
  "CodexBar").
- **Read the cards.** Run node .agents/skills/verify/scripts/cdp.mjs 9333 wait flyout "[...document.querySelectorAll('.menu-card')].map(c => c.innerText.replace(/\s+/g,' ')).filter(t => /61% used|12% used/.test(t)).length === 2".
  The Codex card shows Session 61% used and Weekly 74% used; Claude shows 12% and 37%; resets read 26347d / 26353d.
- **Switch provider.** Run node .agents/skills/verify/scripts/cdp.mjs 9333 click flyout ".provider-grid__item[aria-label='Claude']". Then wait for
  "document.querySelector('.provider-grid__item--active')?.getAttribute('aria-label') === 'Claude' && document.querySelectorAll('.menu-card').length === 1".
- **Proof.** native-window.ps1 -RunDir <runDir> -Title "CodexBar" -Out <runDir>\flyout-native.png, then
  cleanup.ps1; cleanup.json lists settings.json as restored.
- **Without a seed.** launch.ps1 -ProofMode popOut opens the flyout at launch with real data (cards show
  account emails; keep that capture local).

## Gotchas

- A seed is pinned only while it covers all enabled providers and no forced refresh runs. With Refresh when
  the menu opens on (the default here), mounting the panel forces a refresh and real data replaces the seed;
  a reset time missing from the real data can then keep the seed's 2099 value.
- Seed snapshots need "errorState": "ready"; without it the backend marks them Unknown (the float bar then
  shows Usage unavailable). Cards ignore errorState, so the panel alone will not reveal the mistake.
- launch.ps1 returns when main renders; with -ProofMode popOut the flyout page appears a moment later, so
  start with a wait on flyout.
- The flyout is always drawn in the light palette (TrayPanel sets data-theme="light") while its DWM frame
  reports dark; do not report that as a theme bug from this map alone.
- The tray icon itself, its pixels and the context menu are not reachable from CDP or this harness.
