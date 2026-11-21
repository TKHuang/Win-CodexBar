//! In-app Cursor sign-in via an isolated WebView2 window.
//!
//! Opens `https://cursor.com/dashboard` in an ephemeral InPrivate window so
//! the user can complete email, GitHub, Google, or Enterprise SSO sign-in
//! without installing extensions or copying cookies manually. Once the
//! `WorkosCursorSessionToken` cookie appears for `cursor.com`, the session
//! header is persisted to `ManualCookies` and Cursor is refreshed.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use codexbar::core::ProviderId;
use codexbar::settings::{ManualCookies, Settings};
use tauri::{Manager, PhysicalPosition, Url, WebviewUrl};

use super::{MAX_COOKIE_HEADER_LEN, validate_single_line_secret};

const CURSOR_LOGIN_LABEL: &str = "cursor-login";
const CURSOR_LOGIN_URL: &str = "https://cursor.com/dashboard";
const CURSOR_API_URL: &str = "https://cursor.com/api/usage-summary";
const CURSOR_SESSION_COOKIE_NAME: &str = "WorkosCursorSessionToken";
const LOGIN_WINDOW_WIDTH: f64 = 540.0;
const LOGIN_WINDOW_HEIGHT: f64 = 720.0;
const LOGIN_WINDOW_MIN_WIDTH: f64 = 420.0;
const LOGIN_WINDOW_MIN_HEIGHT: f64 = 560.0;
const LOGIN_POLL_INTERVAL: Duration = Duration::from_millis(500);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

static CURSOR_LOGIN_MUTATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static CURSOR_LOGIN_CANCEL: LazyLock<Mutex<Option<Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(None));

struct CancelSlotGuard {
    flag: Arc<AtomicBool>,
}

impl CancelSlotGuard {
    fn install() -> Self {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut slot) = CURSOR_LOGIN_CANCEL.lock() {
            *slot = Some(Arc::clone(&flag));
        }
        Self { flag }
    }

    fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl Drop for CancelSlotGuard {
    fn drop(&mut self) {
        if let Ok(mut slot) = CURSOR_LOGIN_CANCEL.lock() {
            *slot = None;
        }
    }
}

/// Build a single-line `Cookie` header when `WorkosCursorSessionToken` is
/// present and non-empty. Returns `None` until sign-in completes.
pub(crate) fn extract_cursor_cookie_header(cookies: &[(String, String)]) -> Option<String> {
    let mut has_session_token = false;
    let mut seen = HashSet::new();
    let mut pairs = Vec::new();

    for (raw_name, raw_value) in cookies {
        let name = raw_name.trim();
        let value = raw_value.trim();
        if name.is_empty()
            || value.is_empty()
            || name.contains(['\r', '\n', ';', '='])
            || value.contains(['\r', '\n', ';'])
        {
            continue;
        }
        if name.eq_ignore_ascii_case(CURSOR_SESSION_COOKIE_NAME) {
            has_session_token = true;
        }
        if seen.insert(name.to_ascii_lowercase()) {
            pairs.push(format!("{name}={value}"));
        }
    }

    if !has_session_token || pairs.is_empty() {
        return None;
    }

    let header = pairs.join("; ");
    validate_single_line_secret(&header, "Cookie header", MAX_COOKIE_HEADER_LEN).ok()?;
    Some(header)
}

fn open_cursor_login_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window(CURSOR_LOGIN_LABEL) {
        let _ = existing.close();
    }

    let url = Url::parse(CURSOR_LOGIN_URL)
        .map_err(|error| format!("Invalid Cursor login URL: {error}"))?;

    let win = tauri::WebviewWindowBuilder::new(app, CURSOR_LOGIN_LABEL, WebviewUrl::External(url))
        .title("Sign in to Cursor")
        .inner_size(LOGIN_WINDOW_WIDTH, LOGIN_WINDOW_HEIGHT)
        .min_inner_size(LOGIN_WINDOW_MIN_WIDTH, LOGIN_WINDOW_MIN_HEIGHT)
        .incognito(true)
        .resizable(true)
        .focused(true)
        .build()
        .map_err(|error| format!("Could not open Cursor sign-in window: {error}"))?;

    if let Ok(Some(monitor)) = win.primary_monitor() {
        let pos = monitor.position();
        let size = monitor.size();
        let scale = win.scale_factor().unwrap_or(1.0);
        let win_w = (LOGIN_WINDOW_WIDTH * scale) as i32;
        let win_h = (LOGIN_WINDOW_HEIGHT * scale) as i32;
        let x = pos.x + (size.width as i32 - win_w) / 2;
        let y = pos.y + (size.height as i32 - win_h) / 2;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    let _ = win.set_focus();
    Ok(win)
}

fn close_cursor_login_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window(CURSOR_LOGIN_LABEL) {
        let _ = win.close();
    }
}

fn persist_cursor_session_cookie(
    app: &tauri::AppHandle,
    cookie_header: &str,
) -> Result<(), String> {
    validate_single_line_secret(cookie_header, "Cookie header", MAX_COOKIE_HEADER_LEN)?;

    let mut cookies = ManualCookies::load();
    cookies.set(ProviderId::Cursor.cli_name(), cookie_header);
    cookies.save().map_err(|error| error.to_string())?;

    // If Cursor's cookie source was explicitly set to Off, switch it to Manual
    // so the newly captured session takes effect immediately.
    let mut settings = Settings::load();
    if settings.cookie_source(ProviderId::Cursor) == "off" {
        settings.set_cookie_source(ProviderId::Cursor, "manual");
        if let Err(error) = settings.save() {
            tracing::warn!(%error, "failed to update Cursor cookie source after sign-in");
        } else {
            crate::events::emit_settings_changed(app);
        }
    }

    Ok(())
}

/// Open an isolated WebView2 window to `cursor.com/dashboard`, wait until the
/// user completes sign-in, save the resulting session cookie, and refresh.
#[tauri::command]
pub async fn cursor_login_with_browser(app: tauri::AppHandle) -> Result<(), String> {
    let _mutation = CURSOR_LOGIN_MUTATION
        .try_lock()
        .map_err(|_| "Cursor sign-in is already in progress.".to_string())?;
    let cancel_guard = CancelSlotGuard::install();

    let _window = open_cursor_login_window(&app)?;
    let api_url =
        Url::parse(CURSOR_API_URL).map_err(|error| format!("Invalid Cursor API URL: {error}"))?;
    let deadline = Instant::now() + LOGIN_TIMEOUT;

    let captured_header = loop {
        if cancel_guard.is_cancelled() {
            close_cursor_login_window(&app);
            return Err("Cursor sign-in was cancelled.".to_string());
        }
        if Instant::now() >= deadline {
            close_cursor_login_window(&app);
            return Err("Cursor sign-in timed out. Try again.".to_string());
        }

        let Some(win) = app.get_webview_window(CURSOR_LOGIN_LABEL) else {
            return Err("Cursor sign-in window was closed before sign-in completed.".to_string());
        };

        // WebView2's `cookies_for_url` deadlocks if invoked synchronously on
        // the UI thread; keep it on a blocking worker thread.
        let poll_url = api_url.clone();
        let cookies = tokio::task::spawn_blocking(move || win.cookies_for_url(poll_url))
            .await
            .map_err(|error| format!("Cursor sign-in cookie poll failed: {error}"))?;

        if let Ok(list) = cookies {
            let pairs: Vec<(String, String)> = list
                .into_iter()
                .map(|cookie| (cookie.name().to_string(), cookie.value().to_string()))
                .collect();
            if let Some(header) = extract_cursor_cookie_header(&pairs) {
                break header;
            }
        }

        tokio::time::sleep(LOGIN_POLL_INTERVAL).await;
    };

    close_cursor_login_window(&app);
    persist_cursor_session_cookie(&app, &captured_header)?;
    crate::auto_resume::clear(&app, ProviderId::Cursor);

    let refresh_app = app.clone();
    let _ = super::refresh_providers(refresh_app).await;
    Ok(())
}

/// Cancel an active Cursor sign-in window, if one is open.
#[tauri::command]
pub fn cursor_cancel_login(app: tauri::AppHandle) {
    if let Ok(slot) = CURSOR_LOGIN_CANCEL.lock()
        && let Some(flag) = slot.as_ref()
    {
        flag.store(true, Ordering::SeqCst);
    }
    close_cursor_login_window(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_requires_workos_session_token() {
        let pairs = vec![
            ("__cf_bm".to_string(), "abc123".to_string()),
            ("theme".to_string(), "dark".to_string()),
        ];
        assert!(extract_cursor_cookie_header(&pairs).is_none());
    }

    #[test]
    fn extract_ignores_blank_session_token() {
        let pairs = vec![("WorkosCursorSessionToken".to_string(), "   ".to_string())];
        assert!(extract_cursor_cookie_header(&pairs).is_none());
    }

    #[test]
    fn extract_builds_header_with_session_token_and_deduplicates() {
        let pairs = vec![
            (
                "WorkosCursorSessionToken".to_string(),
                "user_123%3A%3Ajwt-token".to_string(),
            ),
            ("__cf_bm".to_string(), "cf-val".to_string()),
            (
                "workoscursorsessiontoken".to_string(),
                "duplicate-ignored".to_string(),
            ),
            ("bad\r\nname".to_string(), "x".to_string()),
        ];
        assert_eq!(
            extract_cursor_cookie_header(&pairs).as_deref(),
            Some("WorkosCursorSessionToken=user_123%3A%3Ajwt-token; __cf_bm=cf-val")
        );
    }
}
