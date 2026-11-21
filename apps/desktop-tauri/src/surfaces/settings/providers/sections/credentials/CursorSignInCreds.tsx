import { useCallback, useEffect, useState } from "react";
import type { LocaleKey } from "../../../../../i18n/keys";
import type { CookieInfoBridge } from "../../../../../types/bridge";
import {
  cursorCancelLogin,
  cursorLoginWithBrowser,
  getManualCookies,
  removeManualCookie,
} from "../../../../../lib/tauri";

interface Props {
  t: (key: LocaleKey) => string;
  onChanged?: () => void;
}

/**
 * In-app Cursor sign-in section. Opens an isolated WebView2 window to
 * `cursor.com/dashboard` so users can sign in directly (including Enterprise
 * SSO) without copying browser cookies by hand.
 */
export function CursorSignInCreds({ t, onChanged }: Props) {
  const [saved, setSaved] = useState<CookieInfoBridge | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [signingIn, setSigningIn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async (signal?: { stale: boolean }) => {
    try {
      const cookies = await getManualCookies();
      if (signal?.stale) return;
      setSaved(cookies.find((c) => c.providerId === "cursor") ?? null);
    } catch (cause: unknown) {
      if (signal?.stale) return;
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      if (!signal?.stale) setLoaded(true);
    }
  }, []);

  useEffect(() => {
    const signal = { stale: false };
    void reload(signal);
    return () => {
      signal.stale = true;
    };
  }, [reload]);

  const handleSignIn = async () => {
    setSigningIn(true);
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await cursorLoginWithBrowser();
      await reload();
      setStatusMessage(t("CursorSignInSuccess"));
      onChanged?.();
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSigningIn(false);
      setBusy(false);
    }
  };

  const handleCancel = () => {
    void cursorCancelLogin();
  };

  const handleSignOut = async () => {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await removeManualCookie("cursor");
      setSaved(null);
      onChanged?.();
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  const statusLabel = saved
    ? t("CredsStatusAuthenticated")
    : t("CredsStatusNotSignedIn");

  return (
    <section className="provider-detail-section">
      <h4>{t("CursorSignInTitle")}</h4>
      {loaded && (
        <dl className="provider-detail-grid">
          <div style={{ display: "contents" }}>
            <dt>{t("CursorSignInTitle")}</dt>
            <dd>
              {statusLabel}
              {saved?.savedAt ? ` · ${saved.savedAt}` : ""}
            </dd>
          </div>
        </dl>
      )}
      <div className="provider-detail-helper">
        {signingIn ? t("CursorSignInWaiting") : t("CursorSignInHelp")}
      </div>
      {statusMessage && (
        <div className="settings-status settings-status--ok">{statusMessage}</div>
      )}
      {error && (
        <div className="settings-status settings-status--error">{error}</div>
      )}
      <div className="provider-detail-actions">
        {!signingIn ? (
          <button
            type="button"
            className="credential-btn credential-btn--primary"
            disabled={busy}
            onClick={() => void handleSignIn()}
          >
            {saved ? t("CursorSwitchAccountButton") : t("CursorSignInButton")}
          </button>
        ) : (
          <button
            type="button"
            className="credential-btn credential-btn--secondary"
            onClick={handleCancel}
          >
            {t("CursorSignInCancelButton")}
          </button>
        )}
        {saved && !signingIn && (
          <button
            type="button"
            className="credential-btn credential-btn--danger"
            disabled={busy}
            onClick={() => void handleSignOut()}
          >
            {t("CursorSignOutButton")}
          </button>
        )}
      </div>
    </section>
  );
}
