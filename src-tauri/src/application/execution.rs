use std::{
    collections::{HashMap, VecDeque},
    fmt,
    future::Future,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Notify, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::request::{MultipartPart, RequestBody, RequestContent, RequestDraftId};

pub const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
pub const MAX_RESPONSE_PREVIEW_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_NORMAL_RESPONSE_DECODED_BYTES: u64 = 1024 * 1024 * 1024;
pub const RESPONSE_TEMP_RETENTION_SECONDS: u64 = 24 * 60 * 60;
pub const DEFAULT_EXECUTION_CONCURRENCY: usize = 8;

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
        queued_ms: u64,
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
        response_file: Option<ResponseFileMetadata>,
        timing: ExecutionTimingMetadata,
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
pub struct ResponseFileMetadata {
    pub path: String,
    pub byte_count: u64,
    pub expires_at_epoch_seconds: u64,
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

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionTimingMetadata {
    pub queued_ms: u64,
    pub dns_ms: Option<u64>,
    pub connect_ms: Option<u64>,
    pub tls_ms: Option<u64>,
    pub first_byte_ms: Option<u64>,
    pub download_ms: Option<u64>,
    pub total_ms: u64,
}

pub type ExecutionEventSink = Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>;

pub struct ExecutionCoordinator {
    state: Mutex<ExecutionState>,
    permits: Arc<Semaphore>,
    queue_notify: Notify,
}

impl ExecutionCoordinator {
    pub fn new() -> Self {
        Self::with_concurrency(DEFAULT_EXECUTION_CONCURRENCY)
    }

    pub fn with_concurrency(concurrency: usize) -> Self {
        let concurrency = concurrency.max(1);
        Self {
            state: Mutex::new(ExecutionState::default()),
            permits: Arc::new(Semaphore::new(concurrency)),
            queue_notify: Notify::new(),
        }
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
        self.start_with_id(ExecutionId::new(), request, sink, run)
    }

    pub fn start_with_id<F, Fut>(
        self: &Arc<Self>,
        execution_id: ExecutionId,
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
        self.start_with_id_observed(execution_id, request, sink, |_| {}, |_| {}, run)
    }

    pub(crate) fn start_with_id_observed<F, Fut, Q, R>(
        self: &Arc<Self>,
        execution_id: ExecutionId,
        request: ExecutionRequest,
        sink: ExecutionEventSink,
        on_queued: Q,
        on_running: R,
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
        Q: FnOnce(ExecutionId),
        R: FnOnce(ExecutionId) + Send + 'static,
    {
        validate_request(&request)?;

        let cancellation = CancellationToken::new();
        let replaced = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| ExecutionError::StateUnavailable)?;
            let replaced = state.latest_by_draft.insert(request.draft_id, execution_id);
            state.queue.push_back(execution_id);
            state.executions.insert(
                execution_id,
                ExecutionEntry {
                    draft_id: request.draft_id,
                    cancellation: cancellation.clone(),
                    queued_at: Instant::now(),
                    last_sequence: 0,
                    terminal_emitted: false,
                    status: ExecutionStatus::Queued,
                },
            );
            replaced.and_then(|id| {
                state
                    .executions
                    .get(&id)
                    .map(|entry| entry.cancellation.clone())
            })
        };

        if let Some(replaced) = replaced {
            replaced.cancel();
        }
        on_queued(execution_id);

        let coordinator = Arc::clone(self);
        let permits = Arc::clone(&self.permits);
        tauri::async_runtime::spawn(async move {
            let is_next = tokio::select! {
                _ = cancellation.cancelled() => {
                    coordinator.emit_cancelled_if_current(execution_id, &sink);
                    return;
                }
                is_next = coordinator.wait_until_next_in_queue(execution_id) => is_next,
            };
            if !is_next {
                coordinator.emit_cancelled_if_current(execution_id, &sink);
                return;
            }

            let permit = tokio::select! {
                _ = cancellation.cancelled() => {
                    coordinator.emit_cancelled_if_current(execution_id, &sink);
                    return;
                }
                permit = permits.acquire_owned() => match permit {
                    Ok(permit) => permit,
                    Err(_) => {
                        coordinator.emit_cancelled_if_current(execution_id, &sink);
                        return;
                    }
                },
            };

            if !coordinator.mark_running(execution_id) {
                coordinator.emit_cancelled_if_current(execution_id, &sink);
                return;
            }
            on_running(execution_id);

            if cancellation.is_cancelled() {
                coordinator.emit_cancelled_if_current(execution_id, &sink);
                return;
            }

            run(
                execution_id,
                request,
                cancellation,
                Arc::clone(&coordinator),
                Arc::clone(&sink),
            )
            .await;
            drop(permit);
        });

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
                .executions
                .get(&execution_id)
                .map(|execution| execution.cancellation.clone())
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
        let mut notify_queue_waiters = false;
        let mut state = self.state.lock().ok()?;
        let draft_id = state.executions.get(&execution_id)?.draft_id;

        if state.latest_by_draft.get(&draft_id) != Some(&execution_id) {
            if kind.is_terminal() {
                state.executions.remove(&execution_id);
                let initial_queue_len = state.queue.len();
                state.queue.retain(|id| *id != execution_id);
                notify_queue_waiters = state.queue.len() != initial_queue_len;
            }
            drop(state);
            if notify_queue_waiters {
                self.queue_notify.notify_waiters();
            }
            return None;
        }

        let execution = state.executions.get_mut(&execution_id)?;
        if execution.terminal_emitted {
            return None;
        }

        execution.last_sequence += 1;
        let sequence = execution.last_sequence;
        let is_terminal = kind.is_terminal();

        if is_terminal {
            execution.terminal_emitted = true;
            state.executions.remove(&execution_id);
            let initial_queue_len = state.queue.len();
            state.queue.retain(|id| *id != execution_id);
            notify_queue_waiters = state.queue.len() != initial_queue_len;
            if state.latest_by_draft.get(&draft_id) == Some(&execution_id) {
                state.latest_by_draft.remove(&draft_id);
            }
        }

        let event = ExecutionEvent {
            execution_id,
            sequence,
            kind,
        };
        drop(state);
        if notify_queue_waiters {
            self.queue_notify.notify_waiters();
        }
        Some(event)
    }

    pub fn queued_ms(&self, execution_id: ExecutionId) -> u64 {
        self.state
            .lock()
            .ok()
            .and_then(|state| {
                state
                    .executions
                    .get(&execution_id)
                    .map(|entry| entry.queued_at)
            })
            .map(|queued_at| queued_at.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn cancel_all(&self) {
        let cancellations = self
            .state
            .lock()
            .map(|state| {
                state
                    .executions
                    .values()
                    .map(|entry| entry.cancellation.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for cancellation in cancellations {
            cancellation.cancel();
        }
    }

    fn mark_running(&self, execution_id: ExecutionId) -> bool {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return false,
        };
        if state.queue.front() != Some(&execution_id) {
            return false;
        }
        state.queue.retain(|id| *id != execution_id);
        let Some(execution) = state.executions.get_mut(&execution_id) else {
            return false;
        };
        if execution.cancellation.is_cancelled() || execution.terminal_emitted {
            return false;
        }
        execution.status = ExecutionStatus::Running;
        drop(state);
        self.queue_notify.notify_waiters();
        true
    }

    fn emit_cancelled_if_current(&self, execution_id: ExecutionId, sink: &ExecutionEventSink) {
        if let Some(event) = self.record_event(execution_id, ExecutionEventKind::Cancelled) {
            sink(event);
        }
    }

    async fn wait_until_next_in_queue(&self, execution_id: ExecutionId) -> bool {
        loop {
            let notified = self.queue_notify.notified();
            {
                let state = match self.state.lock() {
                    Ok(state) => state,
                    Err(_) => return false,
                };
                if state.queue.front() == Some(&execution_id) {
                    return true;
                }
                if !state.executions.contains_key(&execution_id) {
                    return false;
                }
            }
            notified.await;
        }
    }
}

impl Default for ExecutionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExecutionCoordinator {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

#[derive(Default)]
struct ExecutionState {
    executions: HashMap<ExecutionId, ExecutionEntry>,
    latest_by_draft: HashMap<RequestDraftId, ExecutionId>,
    queue: VecDeque<ExecutionId>,
}

struct ExecutionEntry {
    draft_id: RequestDraftId,
    cancellation: CancellationToken,
    queued_at: Instant,
    last_sequence: u64,
    terminal_emitted: bool,
    status: ExecutionStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionStatus {
    Queued,
    Running,
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc as std_mpsc,
    };
    use tokio::sync::{mpsc, oneshot};
    use tokio::time::{sleep, Duration};

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
                    queued_ms: 0,
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

    #[test]
    fn start_can_be_called_without_a_tokio_reactor() {
        let coordinator = Arc::new(ExecutionCoordinator::with_concurrency(1));
        let (started_tx, started_rx) = std_mpsc::channel();

        coordinator
            .start(
                request(),
                Arc::new(|_| {}),
                move |execution_id, _, _, coordinator, sink| async move {
                    started_tx
                        .send(execution_id)
                        .expect("send started execution");
                    if let Some(event) = coordinator.record_event(
                        execution_id,
                        ExecutionEventKind::Completed {
                            status: 200,
                            body_preview: String::new(),
                            body_truncated: false,
                            decoded_bytes: 0,
                            wire_bytes: Some(0),
                            response_file: None,
                            timing: ExecutionTimingMetadata::default(),
                        },
                    ) {
                        sink(event);
                    }
                },
            )
            .expect("start execution");

        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("execution task started outside tokio reactor");
    }

    #[tokio::test]
    async fn lifecycle_observers_run_in_order_before_execution_task() {
        let coordinator = Arc::new(ExecutionCoordinator::with_concurrency(1));
        let execution_id = ExecutionId::new();
        let queued = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(false));
        let queued_for_observer = Arc::clone(&queued);
        let queued_for_running = Arc::clone(&queued);
        let running_for_observer = Arc::clone(&running);
        let queued_for_run = Arc::clone(&queued);
        let running_for_run = Arc::clone(&running);
        let (started_tx, started_rx) = oneshot::channel();

        coordinator
            .start_with_id_observed(
                execution_id,
                request(),
                Arc::new(|_| {}),
                move |_| queued_for_observer.store(true, Ordering::SeqCst),
                move |_| {
                    assert!(queued_for_running.load(Ordering::SeqCst));
                    running_for_observer.store(true, Ordering::SeqCst);
                },
                move |_, _, _, _, _| {
                    assert!(queued_for_run.load(Ordering::SeqCst));
                    assert!(running_for_run.load(Ordering::SeqCst));
                    async move {
                        started_tx.send(()).expect("execution started");
                    }
                },
            )
            .expect("start observed execution");

        started_rx.await.expect("execution task completed");
    }

    #[tokio::test]
    async fn ninth_execution_waits_until_a_slot_opens() {
        let coordinator = Arc::new(ExecutionCoordinator::with_concurrency(8));
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();
        let mut release_txs = Vec::new();

        for index in 0..9 {
            let (release_tx, release_rx) = oneshot::channel();
            release_txs.push(release_tx);
            let started_tx = started_tx.clone();
            coordinator
                .start(
                    request(),
                    Arc::new(|_| {}),
                    move |execution_id, _, _, coordinator, sink| async move {
                        started_tx.send(index).expect("started signal");
                        let _ = release_rx.await;
                        if let Some(event) = coordinator.record_event(
                            execution_id,
                            ExecutionEventKind::Completed {
                                status: 200,
                                body_preview: String::new(),
                                body_truncated: false,
                                decoded_bytes: 0,
                                wire_bytes: Some(0),
                                response_file: None,
                                timing: ExecutionTimingMetadata::default(),
                            },
                        ) {
                            sink(event);
                        }
                    },
                )
                .expect("start execution");
        }

        let mut started = Vec::new();
        for _ in 0..8 {
            started.push(started_rx.recv().await.expect("started execution"));
        }
        started.sort_unstable();
        assert_eq!(started, vec![0, 1, 2, 3, 4, 5, 6, 7]);
        sleep(Duration::from_millis(50)).await;
        assert!(started_rx.try_recv().is_err());

        release_txs.remove(0).send(()).expect("release first");
        assert_eq!(started_rx.recv().await.expect("ninth starts"), 8);
    }

    #[tokio::test]
    async fn queued_execution_can_be_cancelled_before_it_runs() {
        let coordinator = Arc::new(ExecutionCoordinator::with_concurrency(1));
        let events = Arc::new(Mutex::new(Vec::new()));
        let (release_tx, release_rx) = oneshot::channel();
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();

        coordinator
            .start(
                request(),
                Arc::new(|_| {}),
                move |_, _, _, _, _| async move {
                    started_tx.send("first").expect("first started");
                    let _ = release_rx.await;
                },
            )
            .expect("start first");
        assert_eq!(started_rx.recv().await, Some("first"));

        let second = coordinator
            .start(
                request(),
                event_sink(&events),
                move |_, _, _, _, _| async move {
                    panic!("cancelled queued execution must not run");
                },
            )
            .expect("start queued");

        let result = coordinator
            .cancel(second.execution_id)
            .expect("cancel queued");
        assert!(result.cancelled);
        let events = wait_for_terminal(events).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].kind, ExecutionEventKind::Cancelled));

        release_tx.send(()).expect("release first");
    }

    #[tokio::test]
    async fn replacing_a_queued_execution_wakes_the_replacement() {
        let coordinator = Arc::new(ExecutionCoordinator::with_concurrency(1));
        let events = Arc::new(Mutex::new(Vec::new()));
        let (release_tx, release_rx) = oneshot::channel();
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();

        coordinator
            .start(request(), Arc::new(|_| {}), {
                let started_tx = started_tx.clone();
                move |_, _, _, _, _| async move {
                    started_tx.send("first").expect("first started");
                    let _ = release_rx.await;
                }
            })
            .expect("start first");
        assert_eq!(started_rx.recv().await, Some("first"));

        let draft_id = RequestDraftId::new();
        coordinator
            .start(
                request_with_draft(draft_id),
                event_sink(&events),
                move |_, _, _, _, _| async move {
                    panic!("replaced queued execution must not run");
                },
            )
            .expect("start replaced queued execution");

        coordinator
            .start(request_with_draft(draft_id), Arc::new(|_| {}), {
                let started_tx = started_tx.clone();
                move |_, _, _, _, _| async move {
                    started_tx.send("replacement").expect("replacement started");
                }
            })
            .expect("start replacement");

        sleep(Duration::from_millis(50)).await;
        assert!(started_rx.try_recv().is_err());

        release_tx.send(()).expect("release first");
        assert_eq!(started_rx.recv().await, Some("replacement"));
    }

    fn insert_active(
        coordinator: &Arc<ExecutionCoordinator>,
        draft_id: RequestDraftId,
    ) -> ExecutionId {
        let execution_id = ExecutionId::new();
        let mut state = coordinator.state.lock().expect("lock coordinator");
        state.latest_by_draft.insert(draft_id, execution_id);
        state.executions.insert(
            execution_id,
            ExecutionEntry {
                draft_id,
                cancellation: CancellationToken::new(),
                queued_at: Instant::now(),
                last_sequence: 0,
                terminal_emitted: false,
                status: ExecutionStatus::Running,
            },
        );
        execution_id
    }

    fn request() -> ExecutionRequest {
        request_with_draft(RequestDraftId::new())
    }

    fn request_with_draft(draft_id: RequestDraftId) -> ExecutionRequest {
        ExecutionRequest {
            draft_id,
            workspace_base_directory: None,
            content: RequestContent {
                url: "http://127.0.0.1".to_owned(),
                ..RequestContent::blank()
            },
        }
    }

    fn event_sink(
        events: &Arc<Mutex<Vec<ExecutionEvent>>>,
    ) -> Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static> {
        let events = Arc::clone(events);
        Arc::new(move |event| {
            events.lock().expect("lock events").push(event);
        })
    }

    async fn wait_for_terminal(events: Arc<Mutex<Vec<ExecutionEvent>>>) -> Vec<ExecutionEvent> {
        for _ in 0..100 {
            {
                let events = events.lock().expect("lock events");
                if events.iter().any(|event| event.kind.is_terminal()) {
                    return events.clone();
                }
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for terminal event");
    }
}
