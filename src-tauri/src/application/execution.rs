use std::{
    collections::HashMap,
    fmt,
    future::Future,
    str::FromStr,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::request::{MultipartPart, RequestBody, RequestContent, RequestDraftId};

pub const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_RESPONSE_PREVIEW_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ExecutionId(Uuid);

impl ExecutionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ExecutionId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionRequest {
    pub draft_id: RequestDraftId,
    pub workspace_base_directory: Option<String>,
    pub content: RequestContent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StartExecutionResult {
    pub execution_id: ExecutionId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CancelExecutionResult {
    pub execution_id: ExecutionId,
    pub cancelled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionEvent {
    pub execution_id: ExecutionId,
    pub sequence: u64,
    pub kind: ExecutionEventKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum ExecutionEventKind {
    Started {
        method: String,
        url: String,
        tls_verification: bool,
        proxy: ExecutionProxyMetadata,
        timeouts: ExecutionTimeoutMetadata,
    },
    Redirected {
        from: String,
        to: String,
        status: u16,
    },
    UploadProgress {
        sent_bytes: u64,
        total_bytes: u64,
    },
    ResponseHeaders {
        status: u16,
        headers: Vec<ExecutionHeader>,
        protocol: String,
        remote_addr: Option<String>,
    },
    DownloadProgress {
        received_bytes: u64,
        total_bytes: Option<u64>,
    },
    Completed {
        status: u16,
        body_preview: String,
        body_truncated: bool,
        decoded_bytes: u64,
        wire_bytes: Option<u64>,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

impl ExecutionEventKind {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionProxyMetadata {
    pub source: String,
    pub selected_proxy: Option<String>,
    pub bypass_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionTimeoutMetadata {
    pub connect_ms: Option<u64>,
    pub overall_ms: Option<u64>,
    pub idle_ms: Option<u64>,
}

pub type ExecutionEventSink = Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>;

#[derive(Default)]
pub struct ExecutionCoordinator {
    state: Mutex<ExecutionState>,
}

impl ExecutionCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start<F, Fut>(
        self: &Arc<Self>,
        request: ExecutionRequest,
        sink: ExecutionEventSink,
        run: F,
    ) -> Result<StartExecutionResult, ExecutionError>
    where
        F: FnOnce(
                ExecutionId,
                ExecutionRequest,
                CancellationToken,
                Arc<Self>,
                ExecutionEventSink,
            ) -> Fut
            + Send
            + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        validate_request(&request)?;

        let execution_id = ExecutionId::new();
        let cancellation = CancellationToken::new();
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| ExecutionError::StateUnavailable)?;
            state.latest_by_draft.insert(request.draft_id, execution_id);
            state.active.insert(
                execution_id,
                ActiveExecution {
                    draft_id: request.draft_id,
                    cancellation: cancellation.clone(),
                    last_sequence: 0,
                    terminal_emitted: false,
                },
            );
        }

        let coordinator = Arc::clone(self);
        tokio::spawn(run(execution_id, request, cancellation, coordinator, sink));

        Ok(StartExecutionResult { execution_id })
    }

    pub fn cancel(
        &self,
        execution_id: ExecutionId,
    ) -> Result<CancelExecutionResult, ExecutionError> {
        let cancellation = {
            let state = self
                .state
                .lock()
                .map_err(|_| ExecutionError::StateUnavailable)?;
            state
                .active
                .get(&execution_id)
                .map(|active| active.cancellation.clone())
        };

        if let Some(cancellation) = cancellation {
            cancellation.cancel();
            Ok(CancelExecutionResult {
                execution_id,
                cancelled: true,
            })
        } else {
            Ok(CancelExecutionResult {
                execution_id,
                cancelled: false,
            })
        }
    }

    pub fn record_event(
        &self,
        execution_id: ExecutionId,
        kind: ExecutionEventKind,
    ) -> Option<ExecutionEvent> {
        let mut state = self.state.lock().ok()?;
        let draft_id = state.active.get(&execution_id)?.draft_id;

        if state.latest_by_draft.get(&draft_id) != Some(&execution_id) {
            return None;
        }

        let active = state.active.get_mut(&execution_id)?;
        if active.terminal_emitted {
            return None;
        }

        active.last_sequence += 1;
        let sequence = active.last_sequence;
        let is_terminal = kind.is_terminal();

        if is_terminal {
            active.terminal_emitted = true;
            state.active.remove(&execution_id);
            if state.latest_by_draft.get(&draft_id) == Some(&execution_id) {
                state.latest_by_draft.remove(&draft_id);
            }
        }

        Some(ExecutionEvent {
            execution_id,
            sequence,
            kind,
        })
    }
}

#[derive(Default)]
struct ExecutionState {
    active: HashMap<ExecutionId, ActiveExecution>,
    latest_by_draft: HashMap<RequestDraftId, ExecutionId>,
}

struct ActiveExecution {
    draft_id: RequestDraftId,
    cancellation: CancellationToken,
    last_sequence: u64,
    terminal_emitted: bool,
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("request input is invalid: {0}")]
    InvalidInput(String),
    #[error("execution state is unavailable")]
    StateUnavailable,
}

fn validate_request(request: &ExecutionRequest) -> Result<(), ExecutionError> {
    if request.content.url.trim().is_empty() {
        return Err(ExecutionError::InvalidInput("url.required".to_owned()));
    }
    if body_text_bytes(&request.content.body) > MAX_REQUEST_BODY_BYTES {
        return Err(ExecutionError::InvalidInput("body.tooLarge".to_owned()));
    }
    Ok(())
}

fn body_text_bytes(body: &RequestBody) -> usize {
    match body {
        RequestBody::None | RequestBody::Binary { .. } => 0,
        RequestBody::Raw { content } => content.len(),
        RequestBody::UrlEncoded { fields } => fields
            .iter()
            .map(|field| field.name.len() + field.value.len())
            .sum(),
        RequestBody::Multipart { parts } => parts
            .iter()
            .map(|part| match part {
                MultipartPart::Field { name, value, .. } => name.len() + value.len(),
                MultipartPart::File { name, .. } => name.len(),
            })
            .sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::request::RequestContent;

    #[test]
    fn stale_events_for_an_older_execution_are_rejected() {
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let draft_id = RequestDraftId::new();
        let first_id = insert_active(&coordinator, draft_id);
        let second_id = insert_active(&coordinator, draft_id);

        assert!(coordinator
            .record_event(first_id, ExecutionEventKind::Cancelled)
            .is_none());

        let event = coordinator
            .record_event(
                second_id,
                ExecutionEventKind::Started {
                    method: "GET".to_owned(),
                    url: "http://127.0.0.1".to_owned(),
                    tls_verification: true,
                    proxy: ExecutionProxyMetadata {
                        source: "disabled".to_owned(),
                        selected_proxy: None,
                        bypass_reason: Some("proxy.disabled".to_owned()),
                    },
                    timeouts: ExecutionTimeoutMetadata {
                        connect_ms: Some(10_000),
                        overall_ms: Some(300_000),
                        idle_ms: Some(60_000),
                    },
                },
            )
            .expect("current event");
        assert_eq!(event.sequence, 1);
    }

    #[test]
    fn only_one_terminal_event_is_recorded() {
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let execution_id = insert_active(&coordinator, RequestDraftId::new());

        assert!(coordinator
            .record_event(execution_id, ExecutionEventKind::Cancelled)
            .is_some());
        assert!(coordinator
            .record_event(
                execution_id,
                ExecutionEventKind::Failed {
                    message: "late failure".to_owned(),
                },
            )
            .is_none());
    }

    #[test]
    fn start_rejects_unbounded_body_data() {
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            workspace_base_directory: None,
            content: RequestContent {
                body: RequestBody::Raw {
                    content: "x".repeat(MAX_REQUEST_BODY_BYTES + 1),
                },
                ..RequestContent::blank()
            },
        };

        let error = coordinator
            .start(request, Arc::new(|_| {}), |_, _, _, _, _| async {})
            .expect_err("large body rejected");

        assert!(matches!(error, ExecutionError::InvalidInput(_)));
    }

    fn insert_active(
        coordinator: &Arc<ExecutionCoordinator>,
        draft_id: RequestDraftId,
    ) -> ExecutionId {
        let execution_id = ExecutionId::new();
        let mut state = coordinator.state.lock().expect("lock coordinator");
        state.latest_by_draft.insert(draft_id, execution_id);
        state.active.insert(
            execution_id,
            ActiveExecution {
                draft_id,
                cancellation: CancellationToken::new(),
                last_sequence: 0,
                terminal_emitted: false,
            },
        );
        execution_id
    }
}
