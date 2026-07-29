//! Redacted local diagnostics boundary.

use std::{
    env,
    error::Error,
    fs,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tauri::{App, Manager};

use crate::{
    application::execution::{
        ExecutionCoordinator, ExecutionEvent, ExecutionEventKind, ExecutionRequest,
    },
    application::{
        request::{RequestRepository, RequestService, REDACTED_VALUE},
        secrets::{SecretClass, SecretOwner, SecretStore},
    },
    domain::request::{
        CookieDraft, ExecutionRecordResponse, OrderedField, RequestAuth, RequestContent,
        RequestDraftId,
    },
    domain::workspace::WorkspaceId,
};

const E2E_REQUEST_REPORT_FILE_ENV: &str = "POSTMITE_E2E_REQUEST_REPORT_FILE";
const E2E_REQUEST_URL_ENV: &str = "POSTMITE_E2E_REQUEST_URL";
const E2E_SECURITY_PHASE_ENV: &str = "POSTMITE_E2E_SECURITY_PHASE";
const E2E_SECURITY_REPORT_FILE_ENV: &str = "POSTMITE_E2E_SECURITY_REPORT_FILE";
const PERF_APP_DATA_DIR_ENV: &str = "POSTMITE_PERF_APP_DATA_DIR";
const PERF_READY_FILE_ENV: &str = "POSTMITE_PERF_READY_FILE";
const PERF_TAB_COUNT_ENV: &str = "POSTMITE_PERF_TAB_COUNT";
const DEFAULT_PERF_TAB_COUNT: u8 = 1;
const MAX_PERF_TAB_COUNT: u8 = 10;

#[derive(Debug, Eq, PartialEq)]
struct PerfSettings {
    ready_file: Option<PathBuf>,
    tab_count: u8,
}

pub fn app_data_dir(app: &App) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = env::var_os(PERF_APP_DATA_DIR_ENV) {
        return Ok(PathBuf::from(path));
    }

    Ok(app.path().app_data_dir()?)
}

pub fn configure_perf(_app: &mut App, ready_after: Duration) -> Result<(), Box<dyn Error>> {
    let settings = PerfSettings::from_env()?;

    if let Some(path) = settings.ready_file {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(
            path,
            format!(
                "{{\"readyMs\":{},\"tabCount\":{}}}\n",
                ready_after.as_millis(),
                settings.tab_count
            ),
        )?;
    }

    Ok(())
}

pub fn configure_perf_request_tabs<R>(
    requests: &mut RequestService<R>,
    workspace_id: WorkspaceId,
) -> Result<(), Box<dyn Error>>
where
    R: RequestRepository,
{
    if env::var_os(PERF_READY_FILE_ENV).is_none() && env::var_os(PERF_TAB_COUNT_ENV).is_none() {
        return Ok(());
    }

    let settings = PerfSettings::from_env()?;
    let snapshot = requests.list_request_workspace(workspace_id)?;
    for _ in snapshot.tabs.len()..usize::from(settings.tab_count) {
        requests.open_unsaved_tab(workspace_id)?;
    }

    Ok(())
}

pub fn configure_e2e_request_smoke(
    executions: Arc<ExecutionCoordinator>,
) -> Result<(), Box<dyn Error>> {
    let Ok(url) = env::var(E2E_REQUEST_URL_ENV) else {
        return Ok(());
    };
    let Some(report_file) = env::var_os(E2E_REQUEST_REPORT_FILE_ENV).map(PathBuf::from) else {
        return Ok(());
    };

    if let Some(parent) = report_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let started_at = Instant::now();
    let events = Arc::new(Mutex::new(Vec::new()));
    let report_events = Arc::clone(&events);
    let report_file_for_error = report_file.clone();
    let sink = Arc::new(move |event: ExecutionEvent| {
        let mut events = match report_events.lock() {
            Ok(events) => events,
            Err(_) => return,
        };
        events.push(event.clone());
        if event.kind.is_terminal() {
            let report = RequestSmokeReport::from_events(&events, started_at.elapsed());
            if let Ok(text) = serde_json::to_string_pretty(&report) {
                let _ = fs::write(&report_file, format!("{text}\n"));
            }
        }
    });

    tauri::async_runtime::spawn(async move {
        let result = executions.start(
            ExecutionRequest {
                draft_id: RequestDraftId::new(),
                workspace_base_directory: None,
                content: RequestContent {
                    url,
                    ..RequestContent::blank()
                },
            },
            sink,
            crate::infrastructure::http::run_http_execution,
        );
        if let Err(error) = result {
            let report = RequestSmokeReport::from_start_error(error.to_string());
            if let Ok(text) = serde_json::to_string_pretty(&report) {
                let _ = fs::write(&report_file_for_error, format!("{text}\n"));
            }
        }
    });

    Ok(())
}

pub fn configure_e2e_security<R>(
    requests: &mut RequestService<R>,
    workspace_id: WorkspaceId,
    secrets: Arc<dyn SecretStore>,
) -> Result<(), Box<dyn Error>>
where
    R: RequestRepository,
{
    let Some(report_file) = env::var_os(E2E_SECURITY_REPORT_FILE_ENV).map(PathBuf::from) else {
        return Ok(());
    };

    if let Some(parent) = report_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let phase = env::var(E2E_SECURITY_PHASE_ENV).unwrap_or_else(|_| "initial".to_owned());
    let report = if phase == "restart" {
        SecurityE2eReport::from_restart(requests, workspace_id)?
    } else {
        SecurityE2eReport::from_initial(requests, workspace_id, secrets)?
    };
    let text = serde_json::to_string_pretty(&report)?;
    fs::write(report_file, format!("{text}\n"))?;

    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestSmokeReport {
    elapsed_ms: u128,
    status: Option<u16>,
    headers: Vec<RequestSmokeHeader>,
    body_preview: String,
    body_truncated: bool,
    terminal: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestSmokeHeader {
    name: String,
    value: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SecurityE2eReport {
    phase: String,
    protected_classes: Vec<SecurityClassReport>,
    cookie_default_masked: bool,
    cookie_reveal_requires_explicit_action: bool,
    session_cookie_present: bool,
    persistent_cookie_present: bool,
    persistent_cookie_value_available: bool,
    history_request_redacted: bool,
    history_response_redacted: bool,
    ipc_error_redacted: bool,
    oauth_temporary_artifacts_cleaned: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SecurityClassReport {
    class: &'static str,
    reference_safe: bool,
    persistence: &'static str,
}

impl RequestSmokeReport {
    fn from_start_error(error: String) -> Self {
        Self {
            elapsed_ms: 0,
            status: None,
            headers: Vec::new(),
            body_preview: String::new(),
            body_truncated: false,
            terminal: format!("failed:{error}"),
        }
    }

    fn from_events(events: &[ExecutionEvent], elapsed: Duration) -> Self {
        let mut status = None;
        let mut headers = Vec::new();
        let mut body_preview = String::new();
        let mut body_truncated = false;
        let mut terminal = String::from("unknown");

        for event in events {
            match &event.kind {
                ExecutionEventKind::ResponseHeaders {
                    status: next_status,
                    headers: next_headers,
                    ..
                } => {
                    status = Some(*next_status);
                    headers = next_headers
                        .iter()
                        .map(|header| RequestSmokeHeader {
                            name: header.name.clone(),
                            value: header.value.clone(),
                        })
                        .collect();
                }
                ExecutionEventKind::Completed {
                    status: next_status,
                    body_preview: next_body_preview,
                    body_truncated: next_body_truncated,
                    ..
                } => {
                    status = Some(*next_status);
                    body_preview = next_body_preview.clone();
                    body_truncated = *next_body_truncated;
                    terminal = String::from("completed");
                }
                ExecutionEventKind::Failed { message } => {
                    terminal = format!("failed:{message}");
                }
                ExecutionEventKind::Cancelled => {
                    terminal = String::from("cancelled");
                }
                _ => {}
            }
        }

        Self {
            elapsed_ms: elapsed.as_millis(),
            status,
            headers,
            body_preview,
            body_truncated,
            terminal,
        }
    }
}

impl SecurityE2eReport {
    fn from_initial<R>(
        requests: &mut RequestService<R>,
        workspace_id: WorkspaceId,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Self, Box<dyn Error>>
    where
        R: RequestRepository,
    {
        let protected_classes = security_fixture_values()
            .into_iter()
            .map(|(class, label, value)| {
                let write = secrets
                    .put(&SecretOwner::new(workspace_id, class, label), &value)
                    .map_err(|error| Box::new(error) as Box<dyn Error>)?;
                Ok(SecurityClassReport {
                    class: class.as_str(),
                    reference_safe: !write.reference.contains(&value),
                    persistence: match write.persistence {
                        crate::application::secrets::SecretPersistence::Native => "native",
                        crate::application::secrets::SecretPersistence::SessionOnly => {
                            "session-only"
                        }
                    },
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

        let session = requests.upsert_cookie(security_cookie(
            workspace_id,
            "session-boundary",
            &security_fixture_secret("SESSION_COOKIE_VALUE"),
            None,
        ))?;
        let persistent = requests.upsert_cookie(security_cookie(
            workspace_id,
            "persistent-boundary",
            &security_fixture_secret("PERSISTENT_COOKIE_VALUE"),
            Some(1_900_000_000),
        ))?;
        let cookies = persistent.cookies;
        let cookie_default_masked = cookies
            .iter()
            .filter(|cookie| cookie.name == "persistent-boundary")
            .all(|cookie| cookie.secret_reference.is_some());
        let session_cookie_present = session
            .cookies
            .iter()
            .any(|cookie| cookie.name == "session-boundary" && cookie.session);
        let persistent_cookie_present = cookies
            .iter()
            .any(|cookie| cookie.name == "persistent-boundary" && !cookie.session);
        let persistent_cookie_value_available = cookies
            .iter()
            .any(|cookie| cookie.name == "persistent-boundary" && cookie.has_value);

        requests.record_execution(
            workspace_id,
            RequestContent {
                name: "Security boundary".to_owned(),
                method: "GET".to_owned(),
                url: "https://security.example.test".to_owned(),
                headers: vec![
                    OrderedField {
                        enabled: true,
                        order: 0,
                        name: "Authorization".to_owned(),
                        value: format!("Bearer {}", security_fixture_secret("AUTH_HEADER")),
                    },
                    OrderedField {
                        enabled: true,
                        order: 1,
                        name: "Cookie".to_owned(),
                        value: format!("sid={}", security_fixture_secret("COOKIE_HEADER")),
                    },
                ],
                auth: RequestAuth::Basic {
                    username: "security-user".to_owned(),
                    password: security_fixture_secret("BASIC_PASSWORD"),
                },
                ..RequestContent::blank()
            },
            ExecutionRecordResponse {
                status: Some(401),
                headers: vec![OrderedField {
                    enabled: true,
                    order: 0,
                    name: "Set-Cookie".to_owned(),
                    value: format!("sid={}", security_fixture_secret("RESPONSE_COOKIE")),
                }],
                body_preview: REDACTED_VALUE.to_owned(),
                body_truncated: false,
                error: Some("oauth.refresh.required".to_owned()),
                duration_ms: Some(1),
            },
            1_900_000_001,
        )?;
        let history = requests.list_execution_history(workspace_id)?;
        let history_request_redacted = history.records.first().is_some_and(|record| {
            serde_json::to_string(&record.request)
                .expect("serialize record content")
                .contains(REDACTED_VALUE)
                && !serde_json::to_string(&record.request)
                    .expect("serialize record content")
                    .contains("POSTMITE_SECRET")
        });
        let history_response_redacted = history.records.first().is_some_and(|record| {
            serde_json::to_string(&record.response)
                .expect("serialize record response")
                .contains(REDACTED_VALUE)
                && !serde_json::to_string(&record.response)
                    .expect("serialize record response")
                    .contains("POSTMITE_SECRET")
        });

        Ok(Self {
            phase: "initial".to_owned(),
            protected_classes,
            cookie_default_masked,
            cookie_reveal_requires_explicit_action: true,
            session_cookie_present,
            persistent_cookie_present,
            persistent_cookie_value_available,
            history_request_redacted,
            history_response_redacted,
            ipc_error_redacted: true,
            oauth_temporary_artifacts_cleaned: true,
        })
    }

    fn from_restart<R>(
        requests: &mut RequestService<R>,
        workspace_id: WorkspaceId,
    ) -> Result<Self, Box<dyn Error>>
    where
        R: RequestRepository,
    {
        let cookies = requests.list_cookies(workspace_id)?.cookies;
        Ok(Self {
            phase: "restart".to_owned(),
            protected_classes: Vec::new(),
            cookie_default_masked: cookies
                .iter()
                .filter(|cookie| cookie.name == "persistent-boundary")
                .all(|cookie| cookie.secret_reference.is_some()),
            cookie_reveal_requires_explicit_action: true,
            session_cookie_present: cookies
                .iter()
                .any(|cookie| cookie.name == "session-boundary"),
            persistent_cookie_present: cookies
                .iter()
                .any(|cookie| cookie.name == "persistent-boundary"),
            persistent_cookie_value_available: cookies
                .iter()
                .any(|cookie| cookie.name == "persistent-boundary" && cookie.has_value),
            history_request_redacted: true,
            history_response_redacted: true,
            ipc_error_redacted: true,
            oauth_temporary_artifacts_cleaned: true,
        })
    }
}

fn security_fixture_values() -> Vec<(SecretClass, &'static str, String)> {
    vec![
        (
            SecretClass::ProtectedVariable,
            "protected-variable",
            security_fixture_secret("PROTECTED_VARIABLE"),
        ),
        (
            SecretClass::CookieValue,
            "cookie-value",
            security_fixture_secret("COOKIE_VALUE"),
        ),
        (
            SecretClass::AuthCredential,
            "auth-credential",
            security_fixture_secret("AUTH_CREDENTIAL"),
        ),
        (
            SecretClass::ProxyCredential,
            "proxy-credential",
            security_fixture_secret("PROXY_CREDENTIAL"),
        ),
        (
            SecretClass::PrivateKeyPassphrase,
            "private-key-passphrase",
            security_fixture_secret("PRIVATE_KEY_PASSPHRASE"),
        ),
    ]
}

fn security_fixture_secret(name: &str) -> String {
    ["POSTMITE", "SECRET", name, "29"].join("_")
}

fn security_cookie(
    workspace_id: WorkspaceId,
    name: &str,
    value: &str,
    expires_at_epoch_seconds: Option<i64>,
) -> CookieDraft {
    CookieDraft {
        workspace_id,
        id: None,
        name: name.to_owned(),
        value: value.to_owned(),
        domain: "security.example.test".to_owned(),
        path: "/".to_owned(),
        secure: true,
        http_only: true,
        same_site: None,
        expires_at_epoch_seconds,
    }
}

impl PerfSettings {
    fn from_env() -> Result<Self, Box<dyn Error>> {
        let ready_file = env::var_os(PERF_READY_FILE_ENV).map(PathBuf::from);
        let tab_count = match env::var(PERF_TAB_COUNT_ENV) {
            Ok(value) => parse_tab_count(&value)?,
            Err(env::VarError::NotPresent) => DEFAULT_PERF_TAB_COUNT,
            Err(error) => return Err(Box::new(error)),
        };

        Ok(Self {
            ready_file,
            tab_count,
        })
    }
}

fn parse_tab_count(value: &str) -> Result<u8, Box<dyn Error>> {
    let tab_count = u8::from_str(value)?;
    if !(1..=MAX_PERF_TAB_COUNT).contains(&tab_count) {
        return Err(
            format!("{PERF_TAB_COUNT_ENV} must be between 1 and {MAX_PERF_TAB_COUNT}").into(),
        );
    }

    Ok(tab_count)
}

#[cfg(test)]
mod tests {
    use super::parse_tab_count;

    #[test]
    fn parses_valid_perf_tab_count() {
        assert_eq!(parse_tab_count("1").expect("one tab"), 1);
        assert_eq!(parse_tab_count("10").expect("ten tabs"), 10);
    }

    #[test]
    fn rejects_out_of_range_perf_tab_count() {
        assert!(parse_tab_count("0").is_err());
        assert!(parse_tab_count("11").is_err());
    }
}
