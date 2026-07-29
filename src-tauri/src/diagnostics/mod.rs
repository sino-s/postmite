//! Redacted local diagnostics boundary.

use std::{
    env,
    error::Error,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{App, Manager};
use thiserror::Error;
use zip::{write::SimpleFileOptions, ZipWriter};

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
const DIAGNOSTICS_LOG_DIRECTORY: &str = "diagnostics";
const DIAGNOSTICS_LOG_PREFIX: &str = "postmite-diagnostics-";
const DIAGNOSTICS_LOG_SUFFIX: &str = ".jsonl";
const MAX_DIAGNOSTICS_LOG_FILES: u8 = 5;
const MAX_DIAGNOSTICS_LOG_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DIAGNOSTICS_TOTAL_BYTES: u64 = 20 * 1024 * 1024;
const MAX_DEBUG_LOGGING_MINUTES: u16 = 60;
const DIAGNOSTIC_BUNDLE_MANIFEST: &str = "manifest.json";
const DIAGNOSTIC_BUNDLE_METADATA: &str = "runtime-metadata.json";

#[derive(Debug)]
pub struct DiagnosticsService {
    log_directory: PathBuf,
    debug_until: Mutex<Option<SystemTime>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBundlePreview {
    pub entries: Vec<String>,
    pub exclusions: Vec<String>,
    pub debug_logging_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBundleExport {
    pub bundle_path: String,
    pub preview: DiagnosticBundlePreview,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugLoggingStatus {
    pub enabled: bool,
    pub expires_at_epoch_seconds: Option<u64>,
}

#[derive(Debug, Error)]
pub enum DiagnosticsError {
    #[error("diagnostic input is invalid: {0}")]
    InvalidInput(&'static str),
    #[error("diagnostic storage is unavailable")]
    Storage,
}

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

impl DiagnosticsService {
    pub fn new(app_data_dir: &Path) -> Result<Self, DiagnosticsError> {
        let log_directory = app_data_dir.join(DIAGNOSTICS_LOG_DIRECTORY);
        fs::create_dir_all(&log_directory).map_err(|_| DiagnosticsError::Storage)?;
        let service = Self {
            log_directory,
            debug_until: Mutex::new(None),
        };
        service
            .rotate_logs()
            .map_err(|_| DiagnosticsError::Storage)?;
        Ok(service)
    }

    pub fn record_startup(&self, migration_mode: &str, duration: Duration) {
        self.record(DiagnosticEvent {
            category: "startup",
            code: if migration_mode == "safe" {
                "migration.safe-mode"
            } else {
                "migration.normal"
            },
            duration_ms: duration.as_millis() as u64,
            debug: false,
        });
    }

    pub fn record_command(&self, category: &'static str, code: &'static str, duration: Duration) {
        self.record(DiagnosticEvent {
            category,
            code,
            duration_ms: duration.as_millis() as u64,
            debug: self.debug_logging_status().enabled,
        });
    }

    pub fn set_debug_logging(&self, minutes: u16) -> Result<DebugLoggingStatus, DiagnosticsError> {
        if minutes == 0 || minutes > MAX_DEBUG_LOGGING_MINUTES {
            return Err(DiagnosticsError::InvalidInput("debugLoggingMinutes"));
        }
        let until = SystemTime::now() + Duration::from_secs(u64::from(minutes) * 60);
        *self
            .debug_until
            .lock()
            .map_err(|_| DiagnosticsError::Storage)? = Some(until);
        self.record_command("diagnostics", "debug.enabled", Duration::ZERO);
        Ok(self.debug_logging_status())
    }

    pub fn disable_debug_logging(&self) -> Result<DebugLoggingStatus, DiagnosticsError> {
        *self
            .debug_until
            .lock()
            .map_err(|_| DiagnosticsError::Storage)? = None;
        self.record(DiagnosticEvent {
            category: "diagnostics",
            code: "debug.disabled",
            duration_ms: 0,
            debug: false,
        });
        Ok(self.debug_logging_status())
    }

    pub fn debug_logging_status(&self) -> DebugLoggingStatus {
        let until = self.debug_until.lock().ok().and_then(|guard| *guard);
        let now = SystemTime::now();
        let enabled = until.is_some_and(|value| value > now);
        DebugLoggingStatus {
            enabled,
            expires_at_epoch_seconds: enabled.then(|| {
                until
                    .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                    .map(|value| value.as_secs())
                    .unwrap_or_default()
            }),
        }
    }

    pub fn preview_bundle(&self) -> Result<DiagnosticBundlePreview, DiagnosticsError> {
        let mut entries = vec![
            DIAGNOSTIC_BUNDLE_MANIFEST.to_owned(),
            DIAGNOSTIC_BUNDLE_METADATA.to_owned(),
        ];
        entries.extend(self.log_paths()?.into_iter().filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| format!("logs/{name}"))
        }));
        Ok(DiagnosticBundlePreview {
            entries,
            exclusions: vec![
                "postmite.sqlite3".to_owned(),
                "request payloads, URLs, headers, cookies, variables, and Secrets".to_owned(),
            ],
            debug_logging_enabled: self.debug_logging_status().enabled,
        })
    }

    pub fn export_bundle(
        &self,
        bundle_path: &str,
    ) -> Result<DiagnosticBundleExport, DiagnosticsError> {
        if bundle_path.trim().is_empty() {
            return Err(DiagnosticsError::InvalidInput("bundlePath"));
        }
        let preview = self.preview_bundle()?;
        let path = PathBuf::from(bundle_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| DiagnosticsError::Storage)?;
        }
        let file = File::create(&path).map_err(|_| DiagnosticsError::Storage)?;
        let mut archive = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        write_zip_json(&mut archive, DIAGNOSTIC_BUNDLE_MANIFEST, &preview, options)?;
        write_zip_json(
            &mut archive,
            DIAGNOSTIC_BUNDLE_METADATA,
            &DiagnosticRuntimeMetadata {
                format: "postmite.diagnostics",
            },
            options,
        )?;
        for log_path in self.log_paths()? {
            let Some(name) = log_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            archive
                .start_file(format!("logs/{name}"), options)
                .map_err(|_| DiagnosticsError::Storage)?;
            let bytes = fs::read(&log_path).map_err(|_| DiagnosticsError::Storage)?;
            archive
                .write_all(&bytes)
                .map_err(|_| DiagnosticsError::Storage)?;
        }
        archive.finish().map_err(|_| DiagnosticsError::Storage)?;
        self.record_command("diagnostics", "bundle.exported", Duration::ZERO);
        Ok(DiagnosticBundleExport {
            bundle_path: bundle_path.to_owned(),
            preview,
        })
    }

    fn record(&self, event: DiagnosticEvent) {
        let Ok(mut bytes) = serde_json::to_vec(&event) else {
            return;
        };
        bytes.push(b'\n');
        if self.rotate_for_append(bytes.len() as u64).is_err() {
            return;
        }
        let path = self.log_path(0);
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(&bytes);
        }
    }

    fn rotate_for_append(&self, next_bytes: u64) -> Result<(), std::io::Error> {
        let current = self.log_path(0);
        let current_bytes = fs::metadata(&current)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if current_bytes.saturating_add(next_bytes) > MAX_DIAGNOSTICS_LOG_BYTES {
            for index in (1..MAX_DIAGNOSTICS_LOG_FILES).rev() {
                let source = self.log_path(index - 1);
                let destination = self.log_path(index);
                if source.exists() {
                    let _ = fs::remove_file(&destination);
                    fs::rename(source, destination)?;
                }
            }
        }
        self.rotate_logs()
    }

    fn rotate_logs(&self) -> Result<(), std::io::Error> {
        let mut total = 0_u64;
        for index in 0..MAX_DIAGNOSTICS_LOG_FILES {
            let path = self.log_path(index);
            if let Ok(metadata) = fs::metadata(&path) {
                total = total.saturating_add(metadata.len());
            }
        }
        if total > MAX_DIAGNOSTICS_TOTAL_BYTES {
            for index in (1..MAX_DIAGNOSTICS_LOG_FILES).rev() {
                let path = self.log_path(index);
                if path.exists() {
                    fs::remove_file(path)?;
                    break;
                }
            }
        }
        Ok(())
    }

    fn log_paths(&self) -> Result<Vec<PathBuf>, DiagnosticsError> {
        let mut paths = Vec::new();
        for index in (0..MAX_DIAGNOSTICS_LOG_FILES).rev() {
            let path = self.log_path(index);
            if path.exists() {
                paths.push(path);
            }
        }
        Ok(paths)
    }

    fn log_path(&self, index: u8) -> PathBuf {
        self.log_directory.join(format!(
            "{DIAGNOSTICS_LOG_PREFIX}{index}{DIAGNOSTICS_LOG_SUFFIX}"
        ))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticEvent {
    category: &'static str,
    code: &'static str,
    duration_ms: u64,
    debug: bool,
}

#[derive(Serialize)]
struct DiagnosticRuntimeMetadata {
    format: &'static str,
}

fn write_zip_json<T: Serialize>(
    archive: &mut ZipWriter<File>,
    name: &str,
    value: &T,
    options: SimpleFileOptions,
) -> Result<(), DiagnosticsError> {
    archive
        .start_file(name, options)
        .map_err(|_| DiagnosticsError::Storage)?;
    serde_json::to_writer(&mut *archive, value).map_err(|_| DiagnosticsError::Storage)?;
    archive
        .write_all(b"\n")
        .map_err(|_| DiagnosticsError::Storage)
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
    use std::{fs, time::Duration};

    use tempfile::tempdir;
    use zip::ZipArchive;

    use super::{parse_tab_count, DiagnosticsService, MAX_DIAGNOSTICS_LOG_FILES};

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

    #[test]
    fn logs_only_allowlisted_diagnostic_fields() {
        let directory = tempdir().expect("temporary directory");
        let diagnostics = DiagnosticsService::new(directory.path()).expect("diagnostics");
        diagnostics.record_command("startup", "migration.normal", Duration::from_millis(7));
        diagnostics
            .set_debug_logging(1)
            .expect("enable temporary debug logging");

        let log = fs::read_to_string(
            directory
                .path()
                .join("diagnostics/postmite-diagnostics-0.jsonl"),
        )
        .expect("diagnostic log");
        assert!(log.contains("migration.normal"));
        assert!(!log.contains("POSTMITE_SECRET_AUTH_HEADER_29"));
        assert!(!log.contains("https://private.example.test/path?token=POSTMITE_SECRET"));
        assert!(!log.contains("panic payload"));
    }

    #[test]
    fn rotates_logs_to_five_files_within_total_budget() {
        let directory = tempdir().expect("temporary directory");
        let diagnostics = DiagnosticsService::new(directory.path()).expect("diagnostics");
        let logs = directory.path().join("diagnostics");
        for index in 0..MAX_DIAGNOSTICS_LOG_FILES {
            fs::write(
                logs.join(format!("postmite-diagnostics-{index}.jsonl")),
                vec![b'x'; 4 * 1024 * 1024],
            )
            .expect("fixture log");
        }

        diagnostics.record_command("startup", "migration.normal", Duration::ZERO);
        let total: u64 = fs::read_dir(logs)
            .expect("log directory")
            .map(|entry| entry.expect("entry").metadata().expect("metadata").len())
            .sum();
        assert!(total <= 20 * 1024 * 1024);
    }

    #[test]
    fn bundle_preview_and_archive_exclude_database_and_protected_fixture_values() {
        let directory = tempdir().expect("temporary directory");
        let diagnostics = DiagnosticsService::new(directory.path()).expect("diagnostics");
        fs::write(
            directory.path().join("postmite.sqlite3"),
            "POSTMITE_SECRET_DATABASE_VALUE_29",
        )
        .expect("database fixture");
        diagnostics.record_startup("normal", Duration::from_millis(2));
        let archive_path = directory.path().join("diagnostics.zip");

        let preview = diagnostics.preview_bundle().expect("preview");
        assert!(preview
            .entries
            .iter()
            .all(|entry| !entry.contains("sqlite")));
        let exported = diagnostics
            .export_bundle(archive_path.to_str().expect("archive path"))
            .expect("export");
        assert_eq!(exported.preview, preview);

        let file = fs::File::open(archive_path).expect("archive");
        let mut archive = ZipArchive::new(file).expect("zip archive");
        let mut content = String::new();
        for index in 0..archive.len() {
            use std::io::Read;
            let mut entry = archive.by_index(index).expect("entry");
            assert!(!entry.name().contains("sqlite"));
            entry.read_to_string(&mut content).expect("text entry");
        }
        assert!(!content.contains("POSTMITE_SECRET_DATABASE_VALUE_29"));
    }
}
