# Browser Cookie Extraction (Windows)

Win-CodexBar can extract browser cookies for providers that use web authentication (Claude, Cursor, Kimi, and others). This is the Windows rewrite of upstream cookie/Keychain concepts: **DPAPI + browser profiles**, not macOS Keychain prompts.

## Supported Browsers

| Browser | Encryption | Status |
|---------|-----------|--------|
| Chrome | DPAPI + AES-256-GCM; modern profiles may use Chromium ABE (`v20`) | ⚠️ Automatic only when the needed cookies are not App-Bound |
| Edge | DPAPI + AES-256-GCM; modern profiles may use Chromium ABE (`v20`) | ⚠️ Automatic only when the needed cookies are not App-Bound |
| Brave | DPAPI + AES-256-GCM; modern profiles may use Chromium ABE (`v20`) | ⚠️ Automatic only when the needed cookies are not App-Bound |
| Firefox | Unencrypted SQLite | ✅ Automatic |

Chromium App-Bound Encryption (ABE) binds protected cookie keys to the browser installation. Win-CodexBar does not bypass that protection. If the selected Chromium profile stores the provider cookies as App-Bound `v20` values, automatic import cannot decrypt them with the normal user DPAPI key. Use a manual Cookie header or Firefox instead. Chromium browser choices remain available because older or unmigrated profiles can still contain readable DPAPI/AES-GCM cookies.

## Import-only: refreshes never read your browser

**A cookie store is opened only when you click Import.** Background provider
refreshes never touch it.

Reading Chromium's DPAPI-wrapped `Local State` key and decrypting its `Cookies`
database is, step for step, what an infostealer does. CodexBar used to do it as
a fallback on every provider poll (every 5 minutes by default), which makes
endpoint security products classify the app as credential theft — Kaspersky's
behavioural engine reports `PDM:Trojan.Win32.Generic` and terminates the
process.

The rule is enforced in the type system: every entry point into
`rust/src/browser/cookies.rs` takes a `ScanTrigger`, and only
`ScanTrigger::UserImport` is permitted to proceed. A background refresh is
refused before browser detection even runs, so it leaves no trace on the cookie
stores at all. New call sites cannot forget — the code does not compile without
choosing a trigger.

Providers are unaffected once you have imported: the shell passes your stored
cookie into each fetch. Only the implicit scan-every-browser fallback is gone.
If a provider reports "no cookies", import them once (below) and it works again.

## How It Works

1. You pick a browser and click **Import** for a provider
2. CodexBar reads that browser's cookie database from its standard location
3. For Chromium-based browsers, CodexBar can decrypt legacy DPAPI/AES-GCM cookies using the current user's credentials; App-Bound `v20` cookies are intentionally not bypassed
4. Only cookies for the provider's domains are extracted (e.g., `claude.ai`, `cursor.com`)
5. The imported cookie is stored as a manual-cookie override and reused by every later refresh

## Setting Up Cookie Import

1. Open **Settings** → **Providers** tab
2. Select the provider you want to configure
3. In the provider detail pane, find the **Browser Cookies** section
4. Choose your browser from the dropdown and click **Import**

## Manual Cookies

If automatic extraction fails (for example, Chromium App-Bound Encryption is active, the browser database cannot be read, or CodexBar is running in WSL):

1. Open your browser and navigate to the provider's website (e.g., `claude.ai`)
2. Open DevTools (F12) → **Network** tab
3. Refresh the page and click any request to the provider
4. Copy the `Cookie` header value from **Request Headers**
5. In CodexBar Settings → provider detail → **Browser Cookies**, paste the value

## Troubleshooting

- **"Chromium App-Bound Encryption"**: Modern Chrome, Edge, Brave, and other Chromium-based profiles can protect cookies with ABE. Closing the browser does not remove ABE; use a manual Cookie header or Firefox for the same login
- **"Cookie decryption failed"**: Close the browser and retry if the cookie database itself is locked
- **Empty cookies**: Make sure you're logged into the provider's web interface in that browser
- **WSL**: Chromium DPAPI cookies cannot be decrypted from WSL. Use manual cookies or CLI-based auth instead
- **"Browser cookies are only read when you import them"**: expected — a background refresh asked for a cookie that was never imported. Import it once for that provider, or prefer a CLI/OAuth usage source where the provider offers one
- **Antivirus flags CodexBar**: see [SECURITY.md](../SECURITY.md#antivirus-false-positives)

## Related

- [CONFIGURATION.md](./CONFIGURATION.md) — where manual cookies and settings live on disk
- [PROVIDERS.md](./PROVIDERS.md) — web vs cli vs oauth sources
- [WSL.md](./WSL.md) — why automatic Chromium decrypt fails in WSL
