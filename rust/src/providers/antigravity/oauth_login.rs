//! Antigravity Google OAuth 2.0 sign-in (authorization code + PKCE, loopback).
//!
//! Lets a user sign in from CodexBar itself, without the `agy` CLI or the
//! Antigravity desktop app. The result is the same `oauth_creds.json` the
//! remote quota fetcher already reads and refreshes, with the OAuth client
//! stored beside the tokens so refreshes never need Antigravity installed.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::remote::{
    CLIENT_ID_ENV, CLIENT_SECRET_ENV, OAuthClient, OAuthCredentials, TOKEN_URL, TokenClaims,
    credentials_path, default_client_artifacts, discover_client, non_empty, save_credentials,
};

const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SCOPES: &str = "https://www.googleapis.com/auth/cloud-platform \
https://www.googleapis.com/auth/userinfo.email \
https://www.googleapis.com/auth/userinfo.profile openid";
const CALLBACK_PATH: &str = "/oauth-callback";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const ACCEPT_TICK: Duration = Duration::from_millis(250);
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(2);
const TOKEN_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_REQUEST_HEAD_BYTES: usize = 8 * 1024;
const MISSING_CLIENT_ERROR: &str = "No Antigravity OAuth client was found. Enter a Google OAuth desktop Client ID and Client Secret, or install Antigravity.";

/// Where the OAuth client came from, in resolution priority order.
const SOURCE_OVERRIDE: &str = "override";
const SOURCE_CREDENTIALS: &str = "credentials";
const SOURCE_ENV: &str = "env";
const SOURCE_DESKTOP_APP: &str = "desktop_app";

/// Non-secret sign-in state for the settings UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityOAuthStatus {
    pub signed_in: bool,
    pub email: Option<String>,
    pub credentials_path: String,
    pub client_source: Option<String>,
    pub has_client_config: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityOAuthLoginResult {
    pub email: Option<String>,
    pub credentials_path: String,
}

// ── Pure helpers ────────────────────────────────────────────────────

/// RFC 7636 verifier and its S256 challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PkcePair {
    verifier: String,
    challenge: String,
}

impl PkcePair {
    fn generate() -> Self {
        Self::from_verifier(random_token(32))
    }

    fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

/// URL-safe random token built from v4 UUID bytes (16 random bytes each).
fn random_token(byte_len: usize) -> String {
    let bytes: Vec<u8> = std::iter::repeat_with(uuid::Uuid::new_v4)
        .flat_map(uuid::Uuid::into_bytes)
        .take(byte_len)
        .collect();
    URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_state() -> String {
    random_token(16)
}

fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    reqwest::Url::parse_with_params(
        AUTHORIZE_URL,
        [
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("response_type", "code"),
            ("scope", SCOPES),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
        ],
    )
    .expect("the static authorize URL and parameters always form a valid URL")
    .into()
}

/// Request target (path and query) of an HTTP request line, for `GET` only.
fn request_target(request_head: &str) -> Option<&str> {
    let mut parts = request_head.lines().next()?.split_whitespace();
    let (method, target) = (parts.next()?, parts.next()?);
    method.eq_ignore_ascii_case("GET").then_some(target)
}

fn parse_target(target: &str) -> Option<reqwest::Url> {
    reqwest::Url::parse(&format!("http://127.0.0.1{target}")).ok()
}

fn is_callback_request(request_head: &str) -> bool {
    request_target(request_head)
        .and_then(parse_target)
        .is_some_and(|url| url.path() == CALLBACK_PATH)
}

/// Reads the authorization code out of the loopback callback request.
fn parse_callback_request(request_head: &str, expected_state: &str) -> Result<String, String> {
    let url = request_target(request_head)
        .and_then(parse_target)
        .ok_or_else(|| "The sign-in callback was not a valid request.".to_string())?;
    let param = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    if let Some(error) = param("error") {
        return Err(format!("Google sign-in was not completed: {error}"));
    }
    if param("state").as_deref() != Some(expected_state) {
        return Err("The sign-in response did not match this request (state mismatch).".into());
    }
    param("code")
        .filter(|code| !code.is_empty())
        .ok_or_else(|| "The sign-in response did not include an authorization code.".to_string())
}

// ── Client resolution ───────────────────────────────────────────────

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn read_stored_credentials(home: &Path) -> Option<OAuthCredentials> {
    let contents = crate::secure_file::read_string(&credentials_path(home)).ok()?;
    serde_json::from_str(&contents).ok()
}

fn client_from_pair(id: Option<&str>, secret: Option<&str>) -> Option<OAuthClient> {
    Some(OAuthClient {
        client_id: non_empty(id)?.to_string(),
        client_secret: non_empty(secret)?.to_string(),
    })
}

fn resolve_client_at(
    home: &Path,
    override_id: Option<&str>,
    override_secret: Option<&str>,
) -> Result<(OAuthClient, &'static str), String> {
    match (non_empty(override_id), non_empty(override_secret)) {
        (Some(_), Some(_)) => {
            if let Some(client) = client_from_pair(override_id, override_secret) {
                return Ok((client, SOURCE_OVERRIDE));
            }
        }
        (None, None) => {}
        _ => {
            return Err("Enter both the OAuth Client ID and the Client Secret.".into());
        }
    }
    if let Some(client) = read_stored_credentials(home).and_then(|credentials| {
        client_from_pair(
            credentials.client_id.as_deref(),
            credentials.client_secret.as_deref(),
        )
    }) {
        return Ok((client, SOURCE_CREDENTIALS));
    }
    let env = |key: &str| std::env::var(key).ok();
    if let Some(client) = client_from_pair(
        env(CLIENT_ID_ENV).as_deref(),
        env(CLIENT_SECRET_ENV).as_deref(),
    ) {
        return Ok((client, SOURCE_ENV));
    }
    discover_client(&default_client_artifacts())
        .map(|client| (client, SOURCE_DESKTOP_APP))
        .ok_or_else(|| MISSING_CLIENT_ERROR.to_string())
}

/// Resolves the OAuth client: explicit pair, stored with the credentials,
/// environment, then the installed desktop app.
pub(super) fn resolve_client(
    override_id: Option<&str>,
    override_secret: Option<&str>,
) -> Result<OAuthClient, String> {
    resolve_client_at(&home_dir(), override_id, override_secret).map(|(client, _)| client)
}

// ── Status and sign-out ─────────────────────────────────────────────

pub fn status() -> AntigravityOAuthStatus {
    status_at(&home_dir())
}

fn status_at(home: &Path) -> AntigravityOAuthStatus {
    let credentials = read_stored_credentials(home);
    let signed_in = credentials.as_ref().is_some_and(|credentials| {
        non_empty(credentials.access_token.as_deref()).is_some()
            || non_empty(credentials.refresh_token.as_deref()).is_some()
    });
    let email = credentials
        .as_ref()
        .and_then(|credentials| TokenClaims::from_credentials(credentials).email);
    let client_source = resolve_client_at(home, None, None)
        .ok()
        .map(|(_, source)| source.to_string());
    AntigravityOAuthStatus {
        signed_in,
        email,
        credentials_path: credentials_path(home).display().to_string(),
        has_client_config: client_source.is_some(),
        client_source,
    }
}

/// Removes the stored credentials. Signing out twice is not an error.
pub fn sign_out() -> Result<(), String> {
    sign_out_at(&home_dir())
}

fn sign_out_at(home: &Path) -> Result<(), String> {
    match std::fs::remove_file(credentials_path(home)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Could not remove the Antigravity credentials: {}",
            error.kind()
        )),
    }
}

// ── Login flow ──────────────────────────────────────────────────────

/// Opens the browser for Google sign-in, waits for the loopback callback,
/// exchanges the code, and stores the credentials.
pub async fn login_with_browser(
    override_id: Option<&str>,
    override_secret: Option<&str>,
    cancel: Arc<AtomicBool>,
) -> Result<AntigravityOAuthLoginResult, String> {
    let home = home_dir();
    let (client, source) = resolve_client_at(&home, override_id, override_secret)?;
    tracing::debug!(source, "Antigravity OAuth client resolved");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|error| format!("Could not start the sign-in listener: {}", error.kind()))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Could not start the sign-in listener: {}", error.kind()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    let pkce = PkcePair::generate();
    let state = generate_state();
    let auth_url = build_authorize_url(&client.client_id, &redirect_uri, &state, &pkce.challenge);

    open::that_detached(&auth_url)
        .map_err(|error| format!("Could not open the browser for sign-in: {error}"))?;

    let code = await_callback(&listener, &state, &cancel, LOGIN_TIMEOUT).await?;
    let http = reqwest::Client::builder()
        .timeout(TOKEN_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("Could not start the token request: {error}"))?;
    let credentials = exchange_code_for_credentials(
        &http,
        TOKEN_URL,
        &client,
        &code,
        &redirect_uri,
        &pkce.verifier,
    )
    .await?;

    let path = credentials_path(&home);
    persist_credentials(&path, &credentials)
        .map_err(|error| format!("Could not save the credentials: {}", error.kind()))?;
    Ok(AntigravityOAuthLoginResult {
        email: credentials.email,
        credentials_path: path.display().to_string(),
    })
}

/// `secure_file` does not create parent directories, and a first sign-in
/// happens before `~/.codexbar/antigravity` exists.
fn persist_credentials(path: &Path, credentials: &OAuthCredentials) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    save_credentials(path, credentials)
}

enum CallbackOutcome {
    /// Not the OAuth callback (favicon, probe); keep listening.
    Stray,
    Done(Result<String, String>),
}

async fn await_callback(
    listener: &TcpListener,
    expected_state: &str,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("Sign-in was cancelled.".into());
        }
        if Instant::now() >= deadline {
            return Err("Sign-in timed out. Try again.".into());
        }
        let Ok(accepted) = tokio::time::timeout(ACCEPT_TICK, listener.accept()).await else {
            continue;
        };
        let (mut stream, _) =
            accepted.map_err(|error| format!("Sign-in listener failed: {}", error.kind()))?;
        match handle_connection(&mut stream, expected_state).await {
            CallbackOutcome::Stray => {}
            CallbackOutcome::Done(result) => return result,
        }
    }
}

async fn handle_connection(stream: &mut TcpStream, expected_state: &str) -> CallbackOutcome {
    let Some(head) = read_request_head(stream).await else {
        return CallbackOutcome::Stray;
    };
    if !is_callback_request(&head) {
        respond(stream, "404 Not Found", "Not found.").await;
        return CallbackOutcome::Stray;
    }
    let result = parse_callback_request(&head, expected_state);
    match &result {
        Ok(_) => {
            respond(
                stream,
                "200 OK",
                "Signed in. You can close this tab and return to CodexBar.",
            )
            .await;
        }
        Err(_) => {
            respond(
                stream,
                "400 Bad Request",
                "Sign-in failed. Return to CodexBar for details.",
            )
            .await;
        }
    }
    CallbackOutcome::Done(result)
}

/// Reads up to the end of the request line; silent speculative connections
/// time out and count as stray.
async fn read_request_head(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    let read_line = async {
        while !buffer.contains(&b'\n') && buffer.len() < MAX_REQUEST_HEAD_BYTES {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            }
        }
    };
    tokio::time::timeout(REQUEST_READ_TIMEOUT, read_line)
        .await
        .ok()?;
    (!buffer.is_empty()).then(|| String::from_utf8_lossy(&buffer).into_owned())
}

async fn respond(stream: &mut TcpStream, status_line: &str, message: &str) {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>CodexBar</title></head>\
         <body style=\"font-family:sans-serif;margin:3em\"><p>{message}</p></body></html>"
    );
    let response = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(error) = stream.write_all(response.as_bytes()).await {
        tracing::debug!(
            "Antigravity OAuth callback response failed: {}",
            error.kind()
        );
    }
    if let Err(error) = stream.shutdown().await {
        tracing::debug!(
            "Antigravity OAuth callback shutdown failed: {}",
            error.kind()
        );
    }
}

/// Exchanges the authorization code for tokens. The OAuth client is stored
/// with the credentials so refreshes work without Antigravity installed.
async fn exchange_code_for_credentials(
    http: &reqwest::Client,
    token_url: &str,
    client: &OAuthClient,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<OAuthCredentials, String> {
    let response = http
        .post(token_url)
        .form(&[
            ("code", code),
            ("client_id", client.client_id.as_str()),
            ("client_secret", client.client_secret.as_str()),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|error| format!("Could not reach Google to finish sign-in: {error}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|_| format!("Google returned an unreadable token response (HTTP {status})."))?;
    if !status.is_success() {
        let reason = body
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown_error");
        return Err(format!(
            "Google rejected the sign-in (HTTP {status}): {reason}"
        ));
    }
    let text = |name: &str| {
        body.get(name)
            .and_then(Value::as_str)
            .and_then(|value| non_empty(Some(value)))
            .map(str::to_owned)
    };
    let access_token =
        text("access_token").ok_or_else(|| "Google did not return an access token.".to_string())?;
    let expiry_date = body
        .get("expires_in")
        .and_then(Value::as_f64)
        .map(|seconds| chrono::Utc::now().timestamp_millis() as f64 + seconds * 1000.0);

    let mut credentials = OAuthCredentials {
        access_token: Some(access_token),
        refresh_token: text("refresh_token"),
        expiry_date,
        id_token: text("id_token"),
        client_id: Some(client.client_id.clone()),
        client_secret: Some(client.client_secret.clone()),
        ..OAuthCredentials::default()
    };
    credentials.email = TokenClaims::from_credentials(&credentials).email;
    Ok(credentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_client() -> OAuthClient {
        OAuthClient {
            client_id: "123-test.apps.googleusercontent.com".into(),
            client_secret: "GOCSPX-fixturefixturefixturefixt".into(),
        }
    }

    fn jwt(claims: &Value) -> String {
        let part = |value: &Value| URL_SAFE_NO_PAD.encode(value.to_string());
        format!(
            "{}.{}.sig",
            part(&serde_json::json!({"alg": "none"})),
            part(claims)
        )
    }

    fn callback(query: &str) -> String {
        format!("GET /oauth-callback?{query} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
    }

    #[test]
    fn pkce_challenge_is_the_s256_of_the_verifier() {
        // RFC 7636 appendix B.
        let pair = PkcePair::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
        assert_eq!(
            pair.challenge,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );

        let generated = PkcePair::generate();
        assert_eq!(generated.verifier.len(), 43);
        assert!(
            generated
                .verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
        assert_eq!(
            generated,
            PkcePair::from_verifier(generated.verifier.clone())
        );
        assert_ne!(generated.verifier, PkcePair::generate().verifier);
        assert_ne!(generate_state(), generate_state());
    }

    #[test]
    fn authorize_url_carries_every_encoded_parameter() {
        let redirect = "http://127.0.0.1:5123/oauth-callback";
        let url = build_authorize_url("id 1&x", redirect, "st/ate", "chal");
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert!(!url.contains(' '));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5123%2Foauth-callback"));

        let parsed = reqwest::Url::parse(&url).expect("valid url");
        let pairs: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs["client_id"], "id 1&x");
        assert_eq!(pairs["redirect_uri"], redirect);
        assert_eq!(pairs["response_type"], "code");
        assert_eq!(pairs["scope"], SCOPES);
        assert!(pairs["scope"].contains("auth/cloud-platform"));
        assert_eq!(pairs["access_type"], "offline");
        assert_eq!(pairs["prompt"], "consent");
        assert_eq!(pairs["state"], "st/ate");
        assert_eq!(pairs["code_challenge"], "chal");
        assert_eq!(pairs["code_challenge_method"], "S256");
    }

    #[test]
    fn callback_parser_accepts_the_matching_state() {
        assert_eq!(
            parse_callback_request(&callback("code=abc&state=xyz"), "xyz"),
            Ok("abc".to_string())
        );
        assert_eq!(
            parse_callback_request(&callback("state=xyz&code=4%2F0Ab%20c"), "xyz"),
            Ok("4/0Ab c".to_string())
        );
    }

    #[test]
    fn callback_parser_rejects_bad_responses() {
        let mismatch = parse_callback_request(&callback("code=abc&state=other"), "xyz");
        assert!(mismatch.expect_err("state mismatch").contains("state"));
        let missing_state = parse_callback_request(&callback("code=abc"), "xyz");
        assert!(missing_state.is_err());

        let denied = parse_callback_request(&callback("error=access_denied&state=xyz"), "xyz");
        assert!(denied.expect_err("error param").contains("access_denied"));

        let no_code = parse_callback_request(&callback("state=xyz"), "xyz");
        assert!(no_code.expect_err("missing code").contains("code"));
        assert!(parse_callback_request(&callback("code=&state=xyz"), "xyz").is_err());

        let post = "POST /oauth-callback?code=a&state=xyz HTTP/1.1";
        assert!(parse_callback_request(post, "xyz").is_err());
        assert!(parse_callback_request("", "xyz").is_err());
    }

    #[test]
    fn only_the_callback_path_counts_as_the_callback() {
        assert!(is_callback_request(&callback("code=a&state=b")));
        assert!(!is_callback_request("GET /favicon.ico HTTP/1.1\r\n\r\n"));
        assert!(!is_callback_request("GET / HTTP/1.1\r\n\r\n"));
    }

    #[test]
    fn client_resolution_requires_a_complete_override_pair() {
        let home = tempfile::tempdir().expect("tempdir");
        let (client, source) =
            resolve_client_at(home.path(), Some(" id "), Some(" secret ")).expect("override");
        assert_eq!(source, "override");
        assert_eq!(client.client_id, "id");
        assert_eq!(client.client_secret, "secret");

        for (id, secret) in [
            (Some("id"), None),
            (None, Some("secret")),
            (Some("id"), Some("  ")),
        ] {
            let error = resolve_client_at(home.path(), id, secret)
                .err()
                .expect("partial pair");
            assert!(error.contains("both"), "{error}");
        }
    }

    #[test]
    fn stored_client_wins_over_discovery_and_sign_out_is_idempotent() {
        let home = tempfile::tempdir().expect("tempdir");
        let path = credentials_path(home.path());
        let credentials = OAuthCredentials {
            access_token: Some("access".into()),
            client_id: Some("stored-id".into()),
            client_secret: Some("stored-secret".into()),
            ..OAuthCredentials::default()
        };
        persist_credentials(&path, &credentials).expect("save");

        let (client, source) = resolve_client_at(home.path(), None, None).expect("stored client");
        assert_eq!(
            (client.client_id.as_str(), source),
            ("stored-id", "credentials")
        );
        let overridden =
            resolve_client_at(home.path(), Some("o-id"), Some("o-secret")).expect("override");
        assert_eq!(overridden.1, "override");

        let status = status_at(home.path());
        assert!(status.signed_in);
        assert!(status.has_client_config);
        assert_eq!(status.client_source.as_deref(), Some("credentials"));

        sign_out_at(home.path()).expect("sign out");
        sign_out_at(home.path()).expect("sign out is idempotent");
        assert!(!status_at(home.path()).signed_in);
    }

    #[tokio::test]
    async fn token_exchange_persists_credentials_that_status_reads_back() {
        let id_token = jwt(&serde_json::json!({"email": "user@example.com"}));
        let mut server = mockito::Server::new_async().await;
        let token = server
            .mock("POST", "/token")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("code".into(), "auth-code".into()),
                mockito::Matcher::UrlEncoded("grant_type".into(), "authorization_code".into()),
                mockito::Matcher::UrlEncoded("code_verifier".into(), "verifier".into()),
                mockito::Matcher::UrlEncoded(
                    "redirect_uri".into(),
                    "http://127.0.0.1:9/oauth-callback".into(),
                ),
                mockito::Matcher::UrlEncoded("client_id".into(), fixture_client().client_id),
                mockito::Matcher::UrlEncoded(
                    "client_secret".into(),
                    fixture_client().client_secret,
                ),
            ]))
            .with_status(200)
            .with_body(
                serde_json::json!({
                    "access_token": "access-1",
                    "refresh_token": "refresh-1",
                    "expires_in": 3600,
                    "id_token": id_token,
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;
        let http = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("test client");

        let credentials = exchange_code_for_credentials(
            &http,
            &format!("{}/token", server.url()),
            &fixture_client(),
            "auth-code",
            "http://127.0.0.1:9/oauth-callback",
            "verifier",
        )
        .await
        .expect("exchange");
        token.assert_async().await;

        assert_eq!(credentials.access_token.as_deref(), Some("access-1"));
        assert_eq!(credentials.refresh_token.as_deref(), Some("refresh-1"));
        assert_eq!(credentials.email.as_deref(), Some("user@example.com"));
        assert_eq!(credentials.client_id, Some(fixture_client().client_id));
        let expiry = credentials.expiry_date.expect("expiry");
        assert!(expiry > chrono::Utc::now().timestamp_millis() as f64);

        let home = tempfile::tempdir().expect("tempdir");
        let path = credentials_path(home.path());
        persist_credentials(&path, &credentials).expect("save");
        #[cfg(windows)]
        assert!(matches!(
            crate::secure_file::status(&path),
            crate::secure_file::SecureFileStatus::Protected(_)
        ));
        let stored = read_stored_credentials(home.path()).expect("round trip");
        assert!(stored == credentials);

        let status = status_at(home.path());
        assert!(status.signed_in);
        assert_eq!(status.email.as_deref(), Some("user@example.com"));
        assert_eq!(status.client_source.as_deref(), Some("credentials"));
        assert!(status.credentials_path.ends_with("oauth_creds.json"));
    }

    #[tokio::test]
    async fn token_exchange_reports_google_errors_without_the_body() {
        let mut server = mockito::Server::new_async().await;
        let _token = server
            .mock("POST", "/token")
            .with_status(400)
            .with_body(r#"{"error":"invalid_grant","error_description":"secret detail"}"#)
            .create_async()
            .await;
        let http = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("test client");
        let error = exchange_code_for_credentials(
            &http,
            &format!("{}/token", server.url()),
            &fixture_client(),
            "code",
            "http://127.0.0.1:9/oauth-callback",
            "verifier",
        )
        .await
        .err()
        .expect("rejected");
        assert!(error.contains("invalid_grant"));
        assert!(!error.contains("secret detail"));
    }

    async fn send(port: u16, request: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        stream.write_all(request.as_bytes()).await.expect("write");
        let mut response = String::new();
        stream.read_to_string(&mut response).await.expect("read");
        response
    }

    #[tokio::test]
    async fn listener_ignores_stray_requests_until_the_callback_arrives() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let client = tokio::spawn(async move {
            let stray = send(port, "GET /favicon.ico HTTP/1.1\r\n\r\n").await;
            let done = send(port, &callback("code=the-code&state=s1")).await;
            (stray, done)
        });
        let cancel = AtomicBool::new(false);
        let code = await_callback(&listener, "s1", &cancel, Duration::from_secs(10)).await;
        assert_eq!(code, Ok("the-code".to_string()));

        let (stray, done) = client.await.expect("client task");
        assert!(stray.starts_with("HTTP/1.1 404"));
        assert!(done.starts_with("HTTP/1.1 200"));
        assert!(done.contains("Connection: close"));
    }

    #[tokio::test]
    async fn listener_answers_a_state_mismatch_with_an_error_page() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let client = tokio::spawn(async move { send(port, &callback("code=c&state=evil")).await });
        let cancel = AtomicBool::new(false);
        let result = await_callback(&listener, "s1", &cancel, Duration::from_secs(10)).await;
        assert!(result.expect_err("mismatch").contains("state"));
        assert!(
            client
                .await
                .expect("client task")
                .starts_with("HTTP/1.1 400")
        );
    }

    #[tokio::test]
    async fn listener_stops_on_cancel_and_on_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let cancelled = AtomicBool::new(true);
        let error = await_callback(&listener, "s", &cancelled, Duration::from_secs(10))
            .await
            .expect_err("cancelled");
        assert!(error.contains("cancelled"));

        let idle = AtomicBool::new(false);
        let error = await_callback(&listener, "s", &idle, Duration::from_millis(300))
            .await
            .expect_err("timeout");
        assert!(error.contains("timed out"));
    }
}
