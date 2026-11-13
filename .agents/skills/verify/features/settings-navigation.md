# Settings navigation

Settings opens on a sidebar of panes (General, Usage & Spend, Notifications, Menu Bar, Menu, Advanced, Hooks,
About) followed by a Providers list. Choosing a pane or a provider shows its content on the right and marks
the choice in the sidebar.

## Sub-features

- settings-pane selects a pane; the sidebar marks it current and the content changes.
- settings-provider selects a provider; its detail pane (header, Usage, Visible usage items, Connection) opens and no pane stays current.
- settings-proof-target opens Settings directly on a tab or provider pane from proof mode.

## How to get to it (user POV)

- Tray menu > Settings..., or Ctrl+, in the tray panel, or the Settings... row of the tray panel.
- Inside Settings, click a pane or a provider in the sidebar.

## Driving it with cdp.mjs

Preconditions:

- launch.ps1 -ProofMode settings:general -Name settings-navigation. Settings renders on the main window (label
  main, window title "CodexBar Desktop"), not on a separate settings window.
- doctor.ps1 reports DOCTOR OK and pages: main.

- **Choose a pane.** Click About. Run node .agents/skills/verify/scripts/cdp.mjs 9333 click main ".settings-sidebar__pane[data-pane-id='about']". Then
  node .agents/skills/verify/scripts/cdp.mjs 9333 wait main "document.querySelector('.settings-sidebar__pane[aria-current=page]')?.dataset.paneId === 'about'".
  The About content shows the groups CodexBar, Updates and Links.
- **Choose a provider.** Click Codex in the Providers list. Run node .agents/skills/verify/scripts/cdp.mjs 9333 click main
  ".providers-sidebar__row[data-provider-id='codex']". Then wait for
  "document.querySelector('.providers-sidebar__row[data-provider-id=codex]')?.getAttribute('aria-selected') === 'true'".
  No .settings-sidebar__pane has aria-current, and the content headings start with Usage, Visible usage items,
  Connection.
- **Proof target.** launch.ps1 -ProofMode settings:about (or settings:providers:codex) opens on that pane
  without any click; assert the same aria-current / aria-selected state.
- **Proof.** native-window.ps1 -RunDir <runDir> -Title "CodexBar Desktop" -Out <runDir>\settings-native.png.
  Navigation must not change any user file: doctor.ps1 prints no "user config changed" note and cleanup.json
  lists no restored files.

## Gotchas

- Pane ids are the settings tab ids minus providers; a provider row's id is its CLI name (codex, claude, ...).
- Provider rows are li[role=option] in a listbox; their aria-label also carries the service status
  ("Codex - Provider service status: Partial outage"), so match on data-provider-id, not the label.
- The provider pane shows the real account email and plan. Do not share that capture; seed data does not
  replace the account line on a provider pane.
- Settings in proof mode is on main; a Settings window opened from the tray menu in normal mode is a different
  path and was not driven by this map.
