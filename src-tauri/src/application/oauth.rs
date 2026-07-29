use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use getrandom::fill as fill_random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::infrastructure::oauth::{listen_for_oauth_callback, LoopbackCallback};

pub const DEFAULT_OAUTH_TIMEOUT_MS: u64 = 120_000;
const PKCE_VERIFIER_BYTES: usize = 32;
const STATE_BYTES: usize = 32;
const MAX_AUTHORIZATION_URL_BYTES: usize = 8 * 1024;
const MAX_OAUTH_TEXT_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct OAuthFlowId(Uuid);

impl OAuthFlowId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for OAuthFlowId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OAuthFlowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for OAuthFlowId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StartOAuthAuthorizationRequest {
    pub flow_id: OAuthFlowId,
    pub authorization_endpoint: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub redirect_path: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthAuthorizationResult {
    pub flow_id: OAuthFlowId,
    pub redirect_uri: String,
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CancelOAuthAuthorizationResult {
    pub flow_id: OAuthFlowId,
    pub cancelled: bool,
}

pub trait BrowserLauncher: Send + Sync + 'static {
    fn open(&self, url: &Url) -> Result<(), OAuthError>;
}

pub struct SystemBrowserLauncher;

impl BrowserLauncher for SystemBrowserLauncher {
    fn open(&self, url: &Url) -> Result<(), OAuthError> {
        std::process::Command::new("xdg-open")
            .arg(url.as_str())
            .spawn()
            .map(|_| ())
            .map_err(|_| OAuthError::BrowserOpenFailed)
    }
}

pub struct OAuthCoordinator {
    state: Mutex<OAuthState>,
    browser: Arc<dyn BrowserLauncher>,
}

impl OAuthCoordinator {
    pub fn new(browser: Arc<dyn BrowserLauncher>) -> Self {
        Self {
            state: Mutex::new(OAuthState::default()),
            browser,
        }
    }

    pub async fn start(
        &self,
        request: StartOAuthAuthorizationRequest,
    ) -> Result<OAuthAuthorizationResult, OAuthError> {
        validate_start_request(&request)?;
        let redirect_path = validate_redirect_path(request.redirect_path.as_deref())?;
        let timeout = Duration::from_millis(request.timeout_ms.unwrap_or(DEFAULT_OAUTH_TIMEOUT_MS));
        let state = random_url_safe(STATE_BYTES)?;
        let verifier = random_url_safe(PKCE_VERIFIER_BYTES)?;
        let challenge = pkce_s256_challenge(&verifier)?;
        let listener = listen_for_oauth_callback(&redirect_path).await?;
        let redirect_uri = listener.redirect_uri().to_owned();
        let authorization_url = authorization_url(
            &request.authorization_endpoint,
            &request.client_id,
            &request.scopes,
            &redirect_uri,
            &state,
            &challenge,
        )?;
        if authorization_url.as_str().len() > MAX_AUTHORIZATION_URL_BYTES {
            return Err(OAuthError::InvalidInput("oauth.authorizationUrl.tooLarge"));
        }

        let cancellation = self.insert_flow(request.flow_id)?;
        let result = async {
            self.browser.open(&authorization_url)?;
            let callback = listener.wait(cancellation.clone(), timeout).await?;
            callback_result(request.flow_id, redirect_uri, state, callback)
        }
        .await;
        self.remove_flow(request.flow_id);
        result
    }

    pub fn cancel(
        &self,
        flow_id: OAuthFlowId,
    ) -> Result<CancelOAuthAuthorizationResult, OAuthError> {
        let cancellation = {
            let state = self
                .state
                .lock()
                .map_err(|_| OAuthError::StateUnavailable)?;
            state.flows.get(&flow_id).cloned()
        };

        if let Some(cancellation) = cancellation {
            cancellation.cancel();
            Ok(CancelOAuthAuthorizationResult {
                flow_id,
                cancelled: true,
            })
        } else {
            Ok(CancelOAuthAuthorizationResult {
                flow_id,
                cancelled: false,
            })
        }
    }

    fn insert_flow(&self, flow_id: OAuthFlowId) -> Result<CancellationToken, OAuthError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| OAuthError::StateUnavailable)?;
        if state.flows.contains_key(&flow_id) {
            return Err(OAuthError::InvalidInput("oauth.flow.alreadyRunning"));
        }
        let cancellation = CancellationToken::new();
        state.flows.insert(flow_id, cancellation.clone());
        Ok(cancellation)
    }

    fn remove_flow(&self, flow_id: OAuthFlowId) {
        if let Ok(mut state) = self.state.lock() {
            state.flows.remove(&flow_id);
        }
    }
}

#[derive(Default)]
struct OAuthState {
    flows: HashMap<OAuthFlowId, CancellationToken>,
}

#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("oauth input is invalid: {0}")]
    InvalidInput(&'static str),
    #[error("oauth callback listener failed")]
    ListenerFailed,
    #[error("oauth callback listener timed out")]
    Timeout,
    #[error("oauth authorization was cancelled")]
    Cancelled,
    #[error("oauth state mismatch")]
    StateMismatch,
    #[error("oauth authorization code is missing")]
    MissingCode,
    #[error("oauth browser could not be opened")]
    BrowserOpenFailed,
    #[error("oauth state is unavailable")]
    StateUnavailable,
}

pub fn pkce_s256_challenge(verifier: &str) -> Result<String, OAuthError> {
    validate_pkce_verifier(verifier)?;
    let digest = Sha256::digest(verifier.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(digest))
}

pub fn validate_pkce_verifier(verifier: &str) -> Result<(), OAuthError> {
    if verifier.len() < 43 || verifier.len() > 128 {
        return Err(OAuthError::InvalidInput("oauth.pkce.verifier.invalid"));
    }
    if !verifier.bytes().all(
        |byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'),
    ) {
        return Err(OAuthError::InvalidInput("oauth.pkce.verifier.invalid"));
    }
    Ok(())
}

fn validate_start_request(request: &StartOAuthAuthorizationRequest) -> Result<(), OAuthError> {
    let endpoint = Url::parse(&request.authorization_endpoint)
        .map_err(|_| OAuthError::InvalidInput("oauth.authorizationEndpoint.invalid"))?;
    if !matches!(endpoint.scheme(), "http" | "https") {
        return Err(OAuthError::InvalidInput(
            "oauth.authorizationEndpoint.invalid",
        ));
    }
    if request.client_id.trim().is_empty() {
        return Err(OAuthError::InvalidInput("oauth.clientId.required"));
    }
    if request
        .scopes
        .iter()
        .any(|scope| scope.trim().is_empty() || scope.len() > MAX_OAUTH_TEXT_BYTES)
    {
        return Err(OAuthError::InvalidInput("oauth.scope.invalid"));
    }
    if matches!(request.timeout_ms, Some(0)) {
        return Err(OAuthError::InvalidInput("oauth.timeout.invalid"));
    }
    Ok(())
}

fn validate_redirect_path(path: Option<&str>) -> Result<String, OAuthError> {
    let path = path.unwrap_or("/oauth/callback");
    if !path.starts_with('/') || path.contains('?') || path.contains('#') || path.contains("..") {
        return Err(OAuthError::InvalidInput("oauth.redirectPath.invalid"));
    }
    Ok(path.to_owned())
}

fn authorization_url(
    endpoint: &str,
    client_id: &str,
    scopes: &[String],
    redirect_uri: &str,
    state: &str,
    challenge: &str,
) -> Result<Url, OAuthError> {
    let mut url = Url::parse(endpoint)
        .map_err(|_| OAuthError::InvalidInput("oauth.authorizationEndpoint.invalid"))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
        if !scopes.is_empty() {
            query.append_pair("scope", &scopes.join(" "));
        }
    }
    Ok(url)
}

fn callback_result(
    flow_id: OAuthFlowId,
    redirect_uri: String,
    expected_state: String,
    callback: LoopbackCallback,
) -> Result<OAuthAuthorizationResult, OAuthError> {
    let state = callback.query_value("state").map(truncate_oauth_text);
    if state.as_deref() != Some(expected_state.as_str()) {
        return Err(OAuthError::StateMismatch);
    }
    if let Some(error) = callback.query_value("error") {
        return Ok(OAuthAuthorizationResult {
            flow_id,
            redirect_uri,
            code: None,
            state,
            error: Some(truncate_oauth_text(error)),
            error_description: callback
                .query_value("error_description")
                .map(truncate_oauth_text),
        });
    }
    let code = callback
        .query_value("code")
        .map(truncate_oauth_text)
        .filter(|code| !code.is_empty())
        .ok_or(OAuthError::MissingCode)?;
    Ok(OAuthAuthorizationResult {
        flow_id,
        redirect_uri,
        code: Some(code),
        state,
        error: None,
        error_description: None,
    })
}

fn random_url_safe(bytes: usize) -> Result<String, OAuthError> {
    let mut value = vec![0_u8; bytes];
    fill_random(&mut value).map_err(|_| OAuthError::StateUnavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(value))
}

fn truncate_oauth_text(value: &str) -> String {
    value.chars().take(MAX_OAUTH_TEXT_BYTES).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn pkce_rejects_missing_verifier() {
        assert!(matches!(
            pkce_s256_challenge(""),
            Err(OAuthError::InvalidInput("oauth.pkce.verifier.invalid"))
        ));
    }

    #[tokio::test]
    async fn oauth_authorization_code_flow_completes_against_fixture() {
        let fixture = start_authorization_fixture(FixtureMode::Success).await;
        let browser = Arc::new(CapturingBrowser::following_redirects());
        let coordinator = OAuthCoordinator::new(browser.clone());
        let flow_id = OAuthFlowId::new();
        let result = coordinator
            .start(request(flow_id, fixture.url()))
            .await
            .expect("oauth completes");

        assert_eq!(result.flow_id, flow_id);
        assert_eq!(result.code.as_deref(), Some("fixture-code"));
        assert!(result.error.is_none());
        let opened = browser.opened_url();
        assert!(opened.contains("code_challenge_method=S256"));
        assert!(opened.contains("code_challenge="));
        assert!(opened.contains("state="));
        assert!(!opened.contains("fixture-access-token"));
        assert!(fixture.captured_request().contains("code_challenge="));
    }

    #[tokio::test]
    async fn oauth_denial_returns_safe_error_result() {
        let fixture = start_authorization_fixture(FixtureMode::Denied).await;
        let coordinator = OAuthCoordinator::new(Arc::new(CapturingBrowser::following_redirects()));
        let result = coordinator
            .start(request(OAuthFlowId::new(), fixture.url()))
            .await
            .expect("denial is terminal result");

        assert_eq!(result.error.as_deref(), Some("access_denied"));
        assert_eq!(result.error_description.as_deref(), Some("user denied"));
        assert!(result.code.is_none());
    }

    #[tokio::test]
    async fn oauth_state_mismatch_is_rejected() {
        let fixture = start_authorization_fixture(FixtureMode::StateMismatch).await;
        let coordinator = OAuthCoordinator::new(Arc::new(CapturingBrowser::following_redirects()));
        let error = coordinator
            .start(request(OAuthFlowId::new(), fixture.url()))
            .await
            .expect_err("state mismatch rejected");

        assert!(matches!(error, OAuthError::StateMismatch));
    }

    #[tokio::test]
    async fn oauth_timeout_closes_listener() {
        let browser = Arc::new(CapturingBrowser::default());
        let coordinator = OAuthCoordinator::new(browser.clone());
        let error = coordinator
            .start(StartOAuthAuthorizationRequest {
                timeout_ms: Some(20),
                ..request(OAuthFlowId::new(), "http://127.0.0.1:9/auth".to_owned())
            })
            .await
            .expect_err("timeout");

        assert!(matches!(error, OAuthError::Timeout));
        let redirect = redirect_uri_port(&browser.opened_url());
        assert!(tokio::net::TcpStream::connect(("127.0.0.1", redirect))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn oauth_cancel_closes_listener() {
        let browser = Arc::new(CapturingBrowser::default());
        let coordinator = Arc::new(OAuthCoordinator::new(browser.clone()));
        let flow_id = OAuthFlowId::new();
        let task = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .start(request(flow_id, "http://127.0.0.1:9/auth".to_owned()))
                    .await
            })
        };

        browser.wait_until_opened().await;
        let port = redirect_uri_port(&browser.opened_url());
        tokio::time::sleep(Duration::from_millis(100)).await;
        let cancelled = coordinator.cancel(flow_id).expect("cancel flow");
        assert!(cancelled.cancelled);
        let error = task.await.expect("join").expect_err("cancelled result");
        assert!(matches!(error, OAuthError::Cancelled));
        assert!(tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err());
    }

    fn request(
        flow_id: OAuthFlowId,
        authorization_endpoint: String,
    ) -> StartOAuthAuthorizationRequest {
        StartOAuthAuthorizationRequest {
            flow_id,
            authorization_endpoint,
            client_id: "fixture-client".to_owned(),
            scopes: vec!["read".to_owned(), "write".to_owned()],
            redirect_path: None,
            timeout_ms: Some(5_000),
        }
    }

    #[derive(Clone, Copy)]
    enum FixtureMode {
        Success,
        Denied,
        StateMismatch,
    }

    struct AuthorizationFixture {
        address: String,
        captured: Arc<Mutex<Option<String>>>,
    }

    impl AuthorizationFixture {
        fn url(&self) -> String {
            format!("http://{}/authorize", self.address)
        }

        fn captured_request(&self) -> String {
            self.captured
                .lock()
                .expect("lock captured")
                .clone()
                .expect("captured request")
        }
    }

    async fn start_authorization_fixture(mode: FixtureMode) -> AuthorizationFixture {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind authorization fixture");
        let address = listener.local_addr().expect("fixture address").to_string();
        let captured = Arc::new(Mutex::new(None));
        let captured_for_task = Arc::clone(&captured);
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept authorize");
            let mut request = vec![0_u8; 8192];
            let read = stream.read(&mut request).await.expect("read authorize");
            let request = String::from_utf8_lossy(&request[..read]).into_owned();
            *captured_for_task.lock().expect("lock captured") = Some(request.clone());
            let target = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .expect("request target");
            let authorization_url =
                Url::parse(&format!("http://fixture.test{target}")).expect("parse request url");
            let redirect_uri = authorization_url
                .query_pairs()
                .find(|(name, _)| name == "redirect_uri")
                .map(|(_, value)| value.into_owned())
                .expect("redirect uri");
            let state = authorization_url
                .query_pairs()
                .find(|(name, _)| name == "state")
                .map(|(_, value)| value.into_owned())
                .expect("state");
            let callback = match mode {
                FixtureMode::Success => format!("{redirect_uri}?code=fixture-code&state={state}"),
                FixtureMode::Denied => {
                    format!("{redirect_uri}?error=access_denied&error_description=user%20denied&state={state}")
                }
                FixtureMode::StateMismatch => {
                    format!("{redirect_uri}?code=fixture-code&state=wrong-state")
                }
            };
            let response =
                "HTTP/1.1 302 Found\r\nLocation: about:blank\r\nContent-Length: 0\r\n\r\n";
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write authorize response");
            reqwest::get(callback).await.expect("send callback");
        });

        AuthorizationFixture { address, captured }
    }

    #[derive(Default)]
    struct CapturingBrowser {
        opened: Mutex<Option<String>>,
        notify: tokio::sync::Notify,
        follow_redirects: bool,
    }

    impl CapturingBrowser {
        fn following_redirects() -> Self {
            Self {
                follow_redirects: true,
                ..Self::default()
            }
        }

        fn opened_url(&self) -> String {
            self.opened
                .lock()
                .expect("lock opened")
                .clone()
                .expect("opened url")
        }

        async fn wait_until_opened(&self) {
            loop {
                if self.opened.lock().expect("lock opened").is_some() {
                    return;
                }
                self.notify.notified().await;
            }
        }
    }

    impl BrowserLauncher for CapturingBrowser {
        fn open(&self, url: &Url) -> Result<(), OAuthError> {
            let url = url.to_string();
            *self.opened.lock().expect("lock opened") = Some(url.clone());
            self.notify.notify_waiters();
            if self.follow_redirects {
                tokio::spawn(async move {
                    let _ = reqwest::get(url).await;
                });
            }
            Ok(())
        }
    }

    fn redirect_uri_port(opened_url: &str) -> u16 {
        let url = Url::parse(opened_url).expect("opened url");
        let redirect_uri = url
            .query_pairs()
            .find(|(name, _)| name == "redirect_uri")
            .map(|(_, value)| value.into_owned())
            .expect("redirect uri");
        Url::parse(&redirect_uri)
            .expect("redirect url")
            .port()
            .expect("redirect port")
    }
}
