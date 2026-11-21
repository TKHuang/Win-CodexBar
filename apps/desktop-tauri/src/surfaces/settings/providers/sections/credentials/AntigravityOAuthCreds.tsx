import { useCallback, useEffect, useState } from "react";
import type { LocaleKey } from "../../../../../i18n/keys";
import type { AntigravityOAuthStatus } from "../../../../../types/bridge";
import {
  antigravityOAuthCancelLogin,
  antigravityOAuthLogin,
  antigravityOAuthSignOut,
  getAntigravityOAuthStatus,
  openPath,
} from "../../../../../lib/tauri";

interface Props {
  t: (key: LocaleKey) => string;
  onChanged?: () => void;
}

/**
 * Direct Antigravity Google OAuth 2.0 (PKCE + loopback) sign-in section.
 * Works without installing the Antigravity desktop app or `agy` CLI: users can
 * either use a discovered OAuth client or enter a Google OAuth desktop Client
 * ID + Client Secret directly here.
 */
export function AntigravityOAuthCreds({ t, onChanged }: Props) {
  const [status, setStatus] = useState<AntigravityOAuthStatus | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [signingIn, setSigningIn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async (signal?: { stale: boolean }) => {
    try {
      const next = await getAntigravityOAuthStatus();
      if (signal?.stale) return;
      setStatus(next);
    } catch (cause: unknown) {
      if (signal?.stale) return;
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, []);

  useEffect(() => {
    const signal = { stale: false };
    void reload(signal);
    return () => {
      signal.stale = true;
    };
  }, [reload]);

  if (!status && !error) return null;

  const needsClientInput = status != null && !status.hasClientConfig;
  const hasOverridePair =
    clientId.trim().length > 0 && clientSecret.trim().length > 0;
  const canStartSignIn =
    !busy && (status?.hasClientConfig === true || hasOverridePair);

  const handleSignIn = async () => {
    setSigningIn(true);
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await antigravityOAuthLogin(
        clientId.trim() || undefined,
        clientSecret.trim() || undefined,
      );
      setClientSecret("");
      await reload();
      setStatusMessage(t("AntigravitySignInSuccess"));
      onChanged?.();
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSigningIn(false);
      setBusy(false);
    }
  };

  const handleCancel = () => {
    void antigravityOAuthCancelLogin();
  };

  const handleSignOut = async () => {
    setBusy(true);
    setError(null);
    setStatusMessage(null);
    try {
      await antigravityOAuthSignOut();
      await reload();
      onChanged?.();
    } catch (cause: unknown) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };

  const handleOpenFolder = () => {
    if (!status?.credentialsPath) return;
    void openPath(status.credentialsPath).catch((cause: unknown) =>
      setError(cause instanceof Error ? cause.message : String(cause)),
    );
  };

  const statusLabel = status?.signedIn
    ? status.email
      ? `${t("CredsStatusAuthenticated")} (${status.email})`
      : t("CredsStatusAuthenticated")
    : t("CredsStatusNotSignedIn");

  return (
    <section className="provider-detail-section">
      <h4>{t("AntigravitySignInTitle")}</h4>
      {status && (
        <dl className="provider-detail-grid">
          <div style={{ display: "contents" }}>
            <dt>{t("AntigravitySignInTitle")}</dt>
            <dd>{statusLabel}</dd>
          </div>
          {status.credentialsPath && (
            <div style={{ display: "contents" }}>
              <dt>{t("CredsGeminiCliHelperPrefix")}</dt>
              <dd className="provider-detail-grid__mono">
                {status.credentialsPath}
              </dd>
            </div>
          )}
        </dl>
      )}
      <div className="provider-detail-helper">
        {signingIn
          ? t("AntigravitySignInWaiting")
          : needsClientInput
            ? t("AntigravitySignInClientMissingHelp")
            : t("AntigravitySignInHelp")}
      </div>
      {needsClientInput && (
        <>
          <label className="provider-detail-field">
            <span className="provider-detail-field__label">
              {t("AntigravityClientIdLabel")}
            </span>
            <input
              className="provider-detail-field__input"
              type="text"
              value={clientId}
              autoComplete="off"
              spellCheck={false}
              disabled={busy}
              aria-label={t("AntigravityClientIdLabel")}
              onChange={(event) => setClientId(event.target.value)}
            />
          </label>
          <label className="provider-detail-field">
            <span className="provider-detail-field__label">
              {t("AntigravityClientSecretLabel")}
            </span>
            <input
              className="provider-detail-field__input"
              type="password"
              value={clientSecret}
              autoComplete="off"
              spellCheck={false}
              disabled={busy}
              aria-label={t("AntigravityClientSecretLabel")}
              onChange={(event) => setClientSecret(event.target.value)}
            />
          </label>
        </>
      )}
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
            disabled={!canStartSignIn}
            onClick={() => void handleSignIn()}
          >
            {t("AntigravitySignInButton")}
          </button>
        ) : (
          <button
            type="button"
            className="credential-btn credential-btn--secondary"
            onClick={handleCancel}
          >
            {t("AntigravitySignInCancelButton")}
          </button>
        )}
        {status?.signedIn && !signingIn && (
          <>
            <button
              type="button"
              className="credential-btn credential-btn--danger"
              disabled={busy}
              onClick={() => void handleSignOut()}
            >
              {t("AntigravitySignOutButton")}
            </button>
            {status.credentialsPath && (
              <button
                type="button"
                className="btn btn--ghost"
                disabled={busy}
                onClick={handleOpenFolder}
              >
                {t("CredsOpenFolderAction")}
              </button>
            )}
          </>
        )}
      </div>
    </section>
  );
}
