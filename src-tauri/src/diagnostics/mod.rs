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
    application::request::{RequestRepository, RequestService},
    domain::request::{RequestContent, RequestDraftId},
    domain::workspace::WorkspaceId,
};

const E2E_REQUEST_REPORT_FILE_ENV: &str = "POSTMITE_E2E_REQUEST_REPORT_FILE";
const E2E_REQUEST_URL_ENV: &str = "POSTMITE_E2E_REQUEST_URL";
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
