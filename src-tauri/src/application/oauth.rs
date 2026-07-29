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
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::application::secrets::{SecretClass, SecretOwner, SecretStore};
use crate::domain::{
    request::{EnvironmentId, OrderedField, RequestAuth, RequestContent},
    workspace::WorkspaceId,
};
use crate::infrastructure::oauth::{listen_for_oauth_callback, LoopbackCallback};

pub const DEFAULT_OAUTH_TIMEOUT_MS: u64 = 120_000;
const PKCE_VERIFIER_BYTES: usize = 32;
const STATE_BYTES: usize = 32;
const MAX_AUTHORIZATION_URL_BYTES: usize = 8 * 1024;
const MAX_OAUTH_TEXT_BYTES: usize = 1024;
const TOKEN_REFRESH_SKEW_SECONDS: i64 = 60;
const MAX_TOKEN_ENDPOINT_BYTES: usize = 2048;
const MAX_CLIENT_ID_BYTES: usize = 1024;
const MAX_CLIENT_SECRET_BYTES: usize = 4096;
const MAX_SCOPE_BYTES: usize = 4096;

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
    tokens: AsyncMutex<OAuthTokenState>,
    browser: Arc<dyn BrowserLauncher>,
    clock: Arc<dyn OAuthClock>,
}

impl OAuthCoordinator {
    pub fn new(browser: Arc<dyn BrowserLauncher>) -> Self {
        Self::with_clock(browser, Arc::new(SystemOAuthClock))
    }

    pub fn with_clock(browser: Arc<dyn BrowserLauncher>, clock: Arc<dyn OAuthClock>) -> Self {
        Self {
            state: Mutex::new(OAuthState::default()),
            tokens: AsyncMutex::new(OAuthTokenState::default()),
            browser,
            clock,
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

    pub async fn apply_client_credentials_token(
        &self,
        workspace_id: WorkspaceId,
        environment_id: Option<EnvironmentId>,
        mut content: RequestContent,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<RequestContent, OAuthError> {
        let RequestAuth::ClientCredentials {
            token_endpoint,
            client_id,
            client_secret,
            scopes,
        } = content.auth.clone()
        else {
            return Ok(content);
        };

        let config = ClientCredentialsConfig {
            token_endpoint,
            client_id,
            client_secret,
            scopes,
        };
        let access_token = self
            .client_credentials_access_token(workspace_id, environment_id, config, secrets)
            .await?;
        let order = content
            .headers
            .iter()
            .map(|field| field.order)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        content.headers.push(OrderedField {
            enabled: true,
            order,
            name: "Authorization".to_owned(),
            value: format!("Bearer {access_token}"),
        });
        content.auth = RequestAuth::None;
        Ok(content)
    }

    async fn client_credentials_access_token(
        &self,
        workspace_id: WorkspaceId,
        environment_id: Option<EnvironmentId>,
        config: ClientCredentialsConfig,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<String, OAuthError> {
        validate_client_credentials_config(&config)?;
        let key = OAuthTokenKey::new(workspace_id, environment_id, &config)?;
        if let Some(token) = self
            .read_fresh_access_token(&key, Arc::clone(&secrets))
            .await?
        {
            return Ok(token);
        }

        let refresh_lock = {
            let mut tokens = self.tokens.lock().await;
            tokens
                .refresh_locks
                .entry(key.clone())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        let _guard = refresh_lock.lock().await;

        if let Some(token) = self
            .read_fresh_access_token(&key, Arc::clone(&secrets))
            .await?
        {
            return Ok(token);
        }

        let previous = {
            let tokens = self.tokens.lock().await;
            tokens.records.get(&key).cloned()
        };
        let response = if let Some(previous) = previous.as_ref() {
            if let Some(refresh_reference) = previous.refresh_reference.as_deref() {
                let refresh_token = secrets
                    .get(refresh_reference)
                    .map_err(|_| OAuthError::RefreshRequired)?;
                request_token(
                    &config.token_endpoint,
                    TokenGrant::Refresh { refresh_token },
                )
                .await
                .map_err(|_| OAuthError::RefreshRequired)?
            } else {
                let token_endpoint = config.token_endpoint.clone();
                request_token(&token_endpoint, TokenGrant::ClientCredentials { config }).await?
            }
        } else {
            let token_endpoint = config.token_endpoint.clone();
            request_token(&token_endpoint, TokenGrant::ClientCredentials { config }).await?
        };

        self.store_token_response(
            key,
            workspace_id,
            response,
            previous.map(|record| record.generation).unwrap_or(0),
            secrets,
        )
        .await
    }

    async fn read_fresh_access_token(
        &self,
        key: &OAuthTokenKey,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Option<String>, OAuthError> {
        let record = {
            let tokens = self.tokens.lock().await;
            tokens.records.get(key).cloned()
        };
        let Some(record) = record else {
            return Ok(None);
        };
        if record.expires_at_epoch_seconds
            <= self.clock.now_epoch_seconds() + TOKEN_REFRESH_SKEW_SECONDS
        {
            return Ok(None);
        }
        match secrets.get(&record.access_reference) {
            Ok(token) => Ok(Some(token)),
            Err(_) => Ok(None),
        }
    }

    async fn store_token_response(
        &self,
        key: OAuthTokenKey,
        workspace_id: WorkspaceId,
        response: TokenResponse,
        previous_generation: u64,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<String, OAuthError> {
        if !response.token_type.eq_ignore_ascii_case("bearer") {
            return Err(OAuthError::InvalidTokenResponse);
        }
        let expires_in = response.expires_in.unwrap_or(3600).max(1);
        let expires_at_epoch_seconds = self.clock.now_epoch_seconds().saturating_add(expires_in);
        let access_token = response.access_token;
        let access_reference = secrets
            .put(
                &SecretOwner::new(
                    workspace_id,
                    SecretClass::AuthCredential,
                    format!("oauth-access-token:{}", key.owner_name_hash),
                ),
                &access_token,
            )
            .map_err(|_| OAuthError::StateUnavailable)?
            .reference;
        let refresh_reference = if let Some(refresh_token) = response.refresh_token {
            Some(
                secrets
                    .put(
                        &SecretOwner::new(
                            workspace_id,
                            SecretClass::AuthCredential,
                            format!("oauth-refresh-token:{}", key.owner_name_hash),
                        ),
                        &refresh_token,
                    )
                    .map_err(|_| OAuthError::StateUnavailable)?
                    .reference,
            )
        } else {
            None
        };
        let mut tokens = self.tokens.lock().await;
        let current_generation = tokens.records.get(&key).map(|record| record.generation);
        if current_generation.is_some_and(|generation| generation > previous_generation) {
            if let Some(record) = tokens.records.get(&key) {
                return secrets
                    .get(&record.access_reference)
                    .map_err(|_| OAuthError::RefreshRequired);
            }
        }
        tokens.records.insert(
            key,
            OAuthTokenRecord {
                access_reference,
                refresh_reference,
                expires_at_epoch_seconds,
                generation: previous_generation.saturating_add(1),
            },
        );
        Ok(access_token)
    }
}

#[derive(Default)]
struct OAuthState {
    flows: HashMap<OAuthFlowId, CancellationToken>,
}

#[derive(Default)]
struct OAuthTokenState {
    records: HashMap<OAuthTokenKey, OAuthTokenRecord>,
    refresh_locks: HashMap<OAuthTokenKey, Arc<AsyncMutex<()>>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct OAuthTokenKey {
    workspace_id: WorkspaceId,
    environment_id: Option<EnvironmentId>,
    auth_config_hash: String,
    owner_name_hash: String,
}

impl OAuthTokenKey {
    fn new(
        workspace_id: WorkspaceId,
        environment_id: Option<EnvironmentId>,
        config: &ClientCredentialsConfig,
    ) -> Result<Self, OAuthError> {
        let serialized = serde_json::to_vec(config)
            .map_err(|_| OAuthError::InvalidInput("oauth.config.invalid"))?;
        let auth_config_hash = URL_SAFE_NO_PAD.encode(Sha256::digest(&serialized));
        let owner_config = PublicClientCredentialsConfig {
            token_endpoint: &config.token_endpoint,
            client_id: &config.client_id,
            scopes: &config.scopes,
        };
        let serialized_owner = serde_json::to_vec(&owner_config)
            .map_err(|_| OAuthError::InvalidInput("oauth.config.invalid"))?;
        let owner_name_hash = URL_SAFE_NO_PAD.encode(Sha256::digest(&serialized_owner));
        Ok(Self {
            workspace_id,
            environment_id,
            auth_config_hash,
            owner_name_hash,
        })
    }
}

#[derive(Clone)]
struct OAuthTokenRecord {
    access_reference: String,
    refresh_reference: Option<String>,
    expires_at_epoch_seconds: i64,
    generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCredentialsConfig {
    pub token_endpoint: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicClientCredentialsConfig<'a> {
    token_endpoint: &'a str,
    client_id: &'a str,
    scopes: &'a [String],
}

pub trait OAuthClock: Send + Sync + 'static {
    fn now_epoch_seconds(&self) -> i64;
}

struct SystemOAuthClock;

impl OAuthClock for SystemOAuthClock {
    fn now_epoch_seconds(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(0)
    }
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
    #[error("oauth token request failed")]
    TokenRequestFailed,
    #[error("oauth token response is invalid")]
    InvalidTokenResponse,
    #[error("oauth token refresh is required")]
    RefreshRequired,
}

pub fn pkce_s256_challenge(verifier: &str) -> Result<String, OAuthError> {
    validate_pkce_verifier(verifier)?;
    let digest = Sha256::digest(verifier.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(digest))
}

enum TokenGrant {
    ClientCredentials { config: ClientCredentialsConfig },
    Refresh { refresh_token: String },
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    token_type: String,
}

async fn request_token(
    token_endpoint: &str,
    grant: TokenGrant,
) -> Result<TokenResponse, OAuthError> {
    let endpoint = Url::parse(token_endpoint)
        .map_err(|_| OAuthError::InvalidInput("oauth.tokenEndpoint.invalid"))?;
    if !matches!(endpoint.scheme(), "https" | "http") {
        return Err(OAuthError::InvalidInput("oauth.tokenEndpoint.invalid"));
    }
    let mut form = Vec::<(&str, String)>::new();
    match grant {
        TokenGrant::ClientCredentials { config } => {
            form.push(("grant_type", "client_credentials".to_owned()));
            form.push(("client_id", config.client_id));
            form.push(("client_secret", config.client_secret));
            let scope = config
                .scopes
                .into_iter()
                .filter(|scope| !scope.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !scope.is_empty() {
                form.push(("scope", scope));
            }
        }
        TokenGrant::Refresh { refresh_token } => {
            form.push(("grant_type", "refresh_token".to_owned()));
            form.push(("refresh_token", refresh_token));
        }
    }
    let body = {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (name, value) in form {
            serializer.append_pair(name, &value);
        }
        serializer.finish()
    };
    let response = reqwest::Client::new()
        .post(endpoint)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|_| OAuthError::TokenRequestFailed)?;
    if !response.status().is_success() {
        return Err(OAuthError::TokenRequestFailed);
    }
    let body = response
        .bytes()
        .await
        .map_err(|_| OAuthError::InvalidTokenResponse)?;
    serde_json::from_slice::<TokenResponse>(&body).map_err(|_| OAuthError::InvalidTokenResponse)
}

fn validate_client_credentials_config(config: &ClientCredentialsConfig) -> Result<(), OAuthError> {
    let endpoint = Url::parse(&config.token_endpoint)
        .map_err(|_| OAuthError::InvalidInput("oauth.tokenEndpoint.invalid"))?;
    if !matches!(endpoint.scheme(), "https" | "http")
        || config.token_endpoint.len() > MAX_TOKEN_ENDPOINT_BYTES
    {
        return Err(OAuthError::InvalidInput("oauth.tokenEndpoint.invalid"));
    }
    if config.client_id.trim().is_empty() || config.client_id.len() > MAX_CLIENT_ID_BYTES {
        return Err(OAuthError::InvalidInput("oauth.clientId.required"));
    }
    if config.client_secret.is_empty() || config.client_secret.len() > MAX_CLIENT_SECRET_BYTES {
        return Err(OAuthError::InvalidInput("oauth.clientSecret.required"));
    }
    let scope_bytes = config.scopes.iter().map(String::len).sum::<usize>();
    if scope_bytes > MAX_SCOPE_BYTES {
        return Err(OAuthError::InvalidInput("oauth.scope.invalid"));
    }
    Ok(())
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

    #[tokio::test]
    async fn client_credentials_acquires_token_against_fixture() {
        let fixture = start_token_fixture(TokenFixtureMode::Success).await;
        let coordinator = OAuthCoordinator::new(Arc::new(CapturingBrowser::default()));
        let secrets: Arc<dyn SecretStore> =
            Arc::new(super::super::secrets::SessionSecretStore::new());
        let content = coordinator
            .apply_client_credentials_token(
                WorkspaceId::new(),
                None,
                client_credentials_content(fixture.url()),
                secrets,
            )
            .await
            .expect("client credentials token");

        assert_eq!(content.auth, RequestAuth::None);
        assert!(content
            .headers
            .iter()
            .any(|field| field.name == "Authorization"
                && field.value == "Bearer fixture-access-token-1"));
        let requests = fixture.captured_requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains("grant_type=client_credentials"));
        assert!(requests[0].contains("client_id=fixture-client"));
        assert!(requests[0].contains("client_secret=client-secret-fixture"));
    }

    #[tokio::test]
    async fn environment_switching_selects_the_correct_token_set() {
        let fixture = start_token_fixture(TokenFixtureMode::Success).await;
        let coordinator = OAuthCoordinator::new(Arc::new(CapturingBrowser::default()));
        let secrets: Arc<dyn SecretStore> =
            Arc::new(super::super::secrets::SessionSecretStore::new());
        let workspace_id = WorkspaceId::new();
        let first_environment = EnvironmentId::new();
        let second_environment = EnvironmentId::new();
        let first = coordinator
            .apply_client_credentials_token(
                workspace_id,
                Some(first_environment),
                client_credentials_content(fixture.url()),
                Arc::clone(&secrets),
            )
            .await
            .expect("first environment token");
        let second = coordinator
            .apply_client_credentials_token(
                workspace_id,
                Some(second_environment),
                client_credentials_content(fixture.url()),
                Arc::clone(&secrets),
            )
            .await
            .expect("second environment token");
        let first_again = coordinator
            .apply_client_credentials_token(
                workspace_id,
                Some(first_environment),
                client_credentials_content(fixture.url()),
                secrets,
            )
            .await
            .expect("reuse first environment token");

        assert!(authorization_value(&first).ends_with("fixture-access-token-1"));
        assert!(authorization_value(&second).ends_with("fixture-access-token-2"));
        assert!(authorization_value(&first_again).ends_with("fixture-access-token-1"));
        assert_eq!(fixture.captured_requests().len(), 2);
    }

    #[tokio::test]
    async fn controlled_clock_refreshes_shortly_before_expiry() {
        let fixture = start_token_fixture(TokenFixtureMode::Success).await;
        let clock = Arc::new(FixedClock::new(0));
        let coordinator =
            OAuthCoordinator::with_clock(Arc::new(CapturingBrowser::default()), clock.clone());
        let secrets: Arc<dyn SecretStore> =
            Arc::new(super::super::secrets::SessionSecretStore::new());
        let workspace_id = WorkspaceId::new();
        let first = coordinator
            .apply_client_credentials_token(
                workspace_id,
                None,
                client_credentials_content(fixture.url()),
                Arc::clone(&secrets),
            )
            .await
            .expect("initial token");
        clock.set(61);
        let refreshed = coordinator
            .apply_client_credentials_token(
                workspace_id,
                None,
                client_credentials_content(fixture.url()),
                secrets,
            )
            .await
            .expect("refreshed token");

        assert!(authorization_value(&first).ends_with("fixture-access-token-1"));
        assert!(authorization_value(&refreshed).ends_with("fixture-refreshed-access-token-2"));
        let requests = fixture.captured_requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].contains("grant_type=refresh_token"));
        assert!(requests[1].contains("refresh_token=fixture-refresh-token-1"));
    }

    #[tokio::test]
    async fn failed_refresh_refuses_stale_access_token() {
        let fixture = start_token_fixture(TokenFixtureMode::FailRefresh).await;
        let clock = Arc::new(FixedClock::new(0));
        let coordinator =
            OAuthCoordinator::with_clock(Arc::new(CapturingBrowser::default()), clock.clone());
        let secrets: Arc<dyn SecretStore> =
            Arc::new(super::super::secrets::SessionSecretStore::new());
        let workspace_id = WorkspaceId::new();
        coordinator
            .apply_client_credentials_token(
                workspace_id,
                None,
                client_credentials_content(fixture.url()),
                Arc::clone(&secrets),
            )
            .await
            .expect("initial token");
        clock.set(61);
        let error = coordinator
            .apply_client_credentials_token(
                workspace_id,
                None,
                client_credentials_content(fixture.url()),
                secrets,
            )
            .await
            .expect_err("refresh failure refuses stale token");

        assert!(matches!(error, OAuthError::RefreshRequired));
        assert_eq!(fixture.captured_requests().len(), 2);
    }

    #[tokio::test]
    async fn concurrent_requests_share_one_in_flight_refresh() {
        let fixture = start_token_fixture(TokenFixtureMode::SlowSuccess).await;
        let coordinator = Arc::new(OAuthCoordinator::new(Arc::new(CapturingBrowser::default())));
        let secrets: Arc<dyn SecretStore> =
            Arc::new(super::super::secrets::SessionSecretStore::new());
        let workspace_id = WorkspaceId::new();
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let coordinator = Arc::clone(&coordinator);
            let secrets = Arc::clone(&secrets);
            let token_endpoint = fixture.url();
            tasks.push(tokio::spawn(async move {
                coordinator
                    .apply_client_credentials_token(
                        workspace_id,
                        None,
                        client_credentials_content(token_endpoint),
                        secrets,
                    )
                    .await
            }));
        }
        let mut authorizations = Vec::new();
        for task in tasks {
            authorizations.push(authorization_value(
                &task.await.expect("join").expect("token result"),
            ));
        }

        assert!(authorizations
            .iter()
            .all(|value| value.ends_with("fixture-access-token-1")));
        assert_eq!(fixture.captured_requests().len(), 1);
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

    fn client_credentials_content(token_endpoint: String) -> RequestContent {
        RequestContent {
            url: "https://api.example.test/users".to_owned(),
            auth: RequestAuth::ClientCredentials {
                token_endpoint,
                client_id: "fixture-client".to_owned(),
                client_secret: "client-secret-fixture".to_owned(),
                scopes: vec!["read".to_owned(), "write".to_owned()],
            },
            ..RequestContent::blank()
        }
    }

    fn authorization_value(content: &RequestContent) -> String {
        content
            .headers
            .iter()
            .find(|field| field.name == "Authorization")
            .map(|field| field.value.clone())
            .expect("authorization header")
    }

    struct FixedClock {
        now: Mutex<i64>,
    }

    impl FixedClock {
        fn new(now: i64) -> Self {
            Self {
                now: Mutex::new(now),
            }
        }

        fn set(&self, now: i64) {
            *self.now.lock().expect("lock clock") = now;
        }
    }

    impl OAuthClock for FixedClock {
        fn now_epoch_seconds(&self) -> i64 {
            *self.now.lock().expect("lock clock")
        }
    }

    #[derive(Clone, Copy)]
    enum TokenFixtureMode {
        Success,
        FailRefresh,
        SlowSuccess,
    }

    struct TokenFixture {
        address: String,
        captured: Arc<Mutex<Vec<String>>>,
    }

    impl TokenFixture {
        fn url(&self) -> String {
            format!("http://{}/token", self.address)
        }

        fn captured_requests(&self) -> Vec<String> {
            self.captured.lock().expect("lock captured").clone()
        }
    }

    async fn start_token_fixture(mode: TokenFixtureMode) -> TokenFixture {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind token fixture");
        let address = listener
            .local_addr()
            .expect("token fixture address")
            .to_string();
        let captured = Arc::new(Mutex::new(Vec::<String>::new()));
        tokio::spawn({
            let captured = Arc::clone(&captured);
            async move {
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else {
                        break;
                    };
                    let request = read_http_request(&mut stream).await;
                    let body = request
                        .split_once("\r\n\r\n")
                        .map(|(_, body)| body.to_owned())
                        .unwrap_or_default();
                    let request_number = {
                        let mut captured = captured.lock().expect("lock captured");
                        captured.push(body.clone());
                        captured.len()
                    };
                    if matches!(mode, TokenFixtureMode::SlowSuccess) {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    let grant_type = form_value(&body, "grant_type");
                    let response = if grant_type.as_deref() == Some("refresh_token")
                        && matches!(mode, TokenFixtureMode::FailRefresh)
                    {
                        "HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\n\r\n".to_owned()
                    } else {
                        let access_token = if grant_type.as_deref() == Some("refresh_token") {
                            format!("fixture-refreshed-access-token-{request_number}")
                        } else {
                            format!("fixture-access-token-{request_number}")
                        };
                        let refresh_token = format!("fixture-refresh-token-{request_number}");
                        let body = format!(
                            r#"{{"access_token":"{access_token}","refresh_token":"{refresh_token}","expires_in":120,"token_type":"Bearer"}}"#
                        );
                        format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        )
                    };
                    stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("write token response");
                }
            }
        });
        TokenFixture { address, captured }
    }

    async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).await.expect("read token request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            let text = String::from_utf8_lossy(&request);
            let Some((headers, body)) = text.split_once("\r\n\r\n") else {
                continue;
            };
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length: ")
                        .or_else(|| line.strip_prefix("Content-Length: "))
                })
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            if body.len() >= content_length {
                break;
            }
        }
        String::from_utf8_lossy(&request).into_owned()
    }

    fn form_value(body: &str, name: &str) -> Option<String> {
        url::form_urlencoded::parse(body.as_bytes())
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
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
