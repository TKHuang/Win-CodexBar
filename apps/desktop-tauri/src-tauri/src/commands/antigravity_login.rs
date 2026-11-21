//! Tauri commands for Antigravity Google OAuth 2.0 (PKCE + loopback) sign-in.
//!
//! Wraps `codexbar::providers::antigravity::oauth_login` with concurrency
//! protection, cancellation, and post-sign-in provider refresh so users can
//! sign in directly from Settings without installing the Antigravity app or
//! `agy` CLI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use codexbar::core::ProviderId;
use codexbar::providers::antigravity::oauth_login::{
    self, AntigravityOAuthLoginResult, AntigravityOAuthStatus,
};

static ANTIGRAVITY_LOGIN_MUTATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static ANTIGRAVITY_LOGIN_CANCEL: LazyLock<Mutex<Option<Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(None));

struct CancelSlotGuard {
    flag: Arc<AtomicBool>,
}

impl CancelSlotGuard {
    fn install() -> Self {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut slot) = ANTIGRAVITY_LOGIN_CANCEL.lock() {
            *slot = Some(Arc::clone(&flag));
        }
        Self { flag }
    }

    fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }
}

impl Drop for CancelSlotGuard {
    fn drop(&mut self) {
        if let Ok(mut slot) = ANTIGRAVITY_LOGIN_CANCEL.lock() {
            *slot = None;
        }
    }
}

/// Inspect current Antigravity OAuth sign-in state and whether an OAuth client
/// configuration was discovered automatically.
#[tauri::command]
pub async fn antigravity_oauth_status() -> Result<AntigravityOAuthStatus, String> {
    tokio::task::spawn_blocking(oauth_login::status)
        .await
        .map_err(|error| format!("Could not read Antigravity sign-in status: {error}"))
}

/// Start an Antigravity Google OAuth 2.0 PKCE loopback login in the system
/// browser, persist `~/.codexbar/antigravity/oauth_creds.json`, and refresh.
#[tauri::command]
pub async fn antigravity_oauth_login(
    app: tauri::AppHandle,
    client_id: Option<String>,
    client_secret: Option<String>,
) -> Result<AntigravityOAuthLoginResult, String> {
    let _mutation = ANTIGRAVITY_LOGIN_MUTATION
        .try_lock()
        .map_err(|_| "Antigravity sign-in is already in progress.".to_string())?;
    let cancel_guard = CancelSlotGuard::install();

    let result = oauth_login::login_with_browser(
        client_id.as_deref(),
        client_secret.as_deref(),
        cancel_guard.flag(),
    )
    .await?;

    crate::auto_resume::clear(&app, ProviderId::Antigravity);
    let _ = super::refresh_providers(app).await;
    Ok(result)
}

/// Cancel an in-flight Antigravity Google OAuth sign-in, if one is running.
#[tauri::command]
pub fn antigravity_oauth_cancel_login() {
    if let Ok(slot) = ANTIGRAVITY_LOGIN_CANCEL.lock()
        && let Some(flag) = slot.as_ref()
    {
        flag.store(true, Ordering::SeqCst);
    }
}

/// Remove stored Antigravity OAuth credentials and refresh provider state.
#[tauri::command]
pub async fn antigravity_oauth_sign_out(app: tauri::AppHandle) -> Result<(), String> {
    tokio::task::spawn_blocking(oauth_login::sign_out)
        .await
        .map_err(|error| format!("Could not remove Antigravity credentials: {error}"))??;
    crate::auto_resume::clear(&app, ProviderId::Antigravity);
    let _ = super::refresh_providers(app).await;
    Ok(())
}
