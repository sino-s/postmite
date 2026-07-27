use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use futures_util::{stream, StreamExt};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Method, Url,
};
use tokio_util::sync::CancellationToken;

use crate::{
    application::execution::{
        ExecutionCoordinator, ExecutionEvent, ExecutionEventKind, ExecutionHeader, ExecutionId,
        ExecutionRequest, MAX_RESPONSE_PREVIEW_BYTES,
    },
    domain::request::{OrderedField, RequestContent},
};

const UPLOAD_CHUNK_BYTES: usize = 16 * 1024;

pub async fn run_http_execution(
    execution_id: ExecutionId,
    request: ExecutionRequest,
    cancellation: CancellationToken,
    coordinator: Arc<ExecutionCoordinator>,
    sink: Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
) {
    let content = request.content;
    let result = execute_http(
        execution_id,
        &content,
        cancellation.clone(),
        Arc::clone(&coordinator),
        Arc::clone(&sink),
    )
    .await;

    match result {
        Ok(completed) => emit(
            &coordinator,
            &sink,
            execution_id,
            ExecutionEventKind::Completed {
                status: completed.status,
                body_preview: completed.body_preview,
                body_truncated: completed.body_truncated,
            },
        ),
        Err(HttpExecutionError::Cancelled) => emit(
            &coordinator,
            &sink,
            execution_id,
            ExecutionEventKind::Cancelled,
        ),
        Err(error) => emit(
            &coordinator,
            &sink,
            execution_id,
            ExecutionEventKind::Failed {
                message: error.safe_message(),
            },
        ),
    }
}

async fn execute_http(
    execution_id: ExecutionId,
    content: &RequestContent,
    cancellation: CancellationToken,
    coordinator: Arc<ExecutionCoordinator>,
    sink: Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
) -> Result<CompletedHttpExecution, HttpExecutionError> {
    let method = Method::from_bytes(content.method.as_bytes())
        .map_err(|_| HttpExecutionError::InvalidInput("method.invalid"))?;
    let url = resolve_url(content)?;
    let headers = resolve_headers(&content.headers)?;
    let body = content.body.as_bytes().to_vec();

    emit(
        &coordinator,
        &sink,
        execution_id,
        ExecutionEventKind::Started {
            method: method.as_str().to_owned(),
            url: url.to_string(),
        },
    );

    let client = reqwest::Client::builder()
        .no_gzip()
        .no_brotli()
        .no_zstd()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| HttpExecutionError::Transport)?;

    let mut builder = client.request(method, url).headers(headers);
    if !body.is_empty() {
        let total_bytes = body.len() as u64;
        let body_stream = cancellable_upload_stream(
            body,
            cancellation.clone(),
            execution_id,
            Arc::clone(&coordinator),
            Arc::clone(&sink),
        );
        builder = builder.body(reqwest::Body::wrap_stream(body_stream));
        builder = builder.header(reqwest::header::CONTENT_LENGTH, total_bytes);
    }

    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(HttpExecutionError::Cancelled),
        result = builder.send() => match result {
            Ok(response) => response,
            Err(_) if cancellation.is_cancelled() => return Err(HttpExecutionError::Cancelled),
            Err(error) if error.is_request() || error.is_connect() || error.is_timeout() => {
                return Err(HttpExecutionError::Transport);
            }
            Err(_) => return Err(HttpExecutionError::Response),
        },
    };

    let status = response.status().as_u16();
    let total_bytes = response.content_length();
    let headers = response_headers(response.headers());
    emit(
        &coordinator,
        &sink,
        execution_id,
        ExecutionEventKind::ResponseHeaders { status, headers },
    );

    let mut stream = response.bytes_stream();
    let mut preview = Vec::new();
    let mut received_bytes = 0_u64;
    let mut body_truncated = false;

    loop {
        let chunk = tokio::select! {
            _ = cancellation.cancelled() => return Err(HttpExecutionError::Cancelled),
            chunk = stream.next() => chunk,
        };

        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk.map_err(|_| {
            if cancellation.is_cancelled() {
                HttpExecutionError::Cancelled
            } else {
                HttpExecutionError::Response
            }
        })?;
        received_bytes += chunk.len() as u64;
        if preview.len() < MAX_RESPONSE_PREVIEW_BYTES {
            let remaining = MAX_RESPONSE_PREVIEW_BYTES - preview.len();
            let copied = chunk.len().min(remaining);
            preview.extend_from_slice(&chunk[..copied]);
            body_truncated = copied < chunk.len();
        } else {
            body_truncated = true;
        }

        emit(
            &coordinator,
            &sink,
            execution_id,
            ExecutionEventKind::DownloadProgress {
                received_bytes,
                total_bytes,
            },
        );
    }

    Ok(CompletedHttpExecution {
        status,
        body_preview: String::from_utf8_lossy(&preview).into_owned(),
        body_truncated,
    })
}

fn resolve_url(content: &RequestContent) -> Result<Url, HttpExecutionError> {
    let mut url =
        Url::parse(&content.url).map_err(|_| HttpExecutionError::InvalidInput("url.invalid"))?;
    url.set_query(None);
    let fields = sorted_enabled_fields(&content.query);
    if !fields.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for field in fields {
            pairs.append_pair(&field.name, &field.value);
        }
    }
    Ok(url)
}

fn resolve_headers(fields: &[OrderedField]) -> Result<HeaderMap, HttpExecutionError> {
    let mut headers = HeaderMap::new();
    for field in sorted_enabled_fields(fields) {
        if field.name.trim().is_empty() {
            return Err(HttpExecutionError::InvalidInput("header.name.required"));
        }
        let name = HeaderName::from_bytes(field.name.as_bytes())
            .map_err(|_| HttpExecutionError::InvalidInput("header.name.invalid"))?;
        let value = HeaderValue::from_str(&field.value)
            .map_err(|_| HttpExecutionError::InvalidInput("header.value.invalid"))?;
        headers.append(name, value);
    }
    Ok(headers)
}

fn sorted_enabled_fields(fields: &[OrderedField]) -> Vec<&OrderedField> {
    let mut fields = fields
        .iter()
        .filter(|field| field.enabled)
        .collect::<Vec<_>>();
    fields.sort_by_key(|field| field.order);
    fields
}

fn response_headers(headers: &HeaderMap) -> Vec<ExecutionHeader> {
    headers
        .iter()
        .map(|(name, value)| ExecutionHeader {
            name: name.as_str().to_owned(),
            value: value.to_str().unwrap_or("<binary>").to_owned(),
        })
        .collect()
}

fn cancellable_upload_stream(
    body: Vec<u8>,
    cancellation: CancellationToken,
    execution_id: ExecutionId,
    coordinator: Arc<ExecutionCoordinator>,
    sink: Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static {
    let total_bytes = body.len() as u64;
    stream::unfold(0_usize, move |offset| {
        let body = body.clone();
        let cancellation = cancellation.clone();
        let coordinator = Arc::clone(&coordinator);
        let sink = Arc::clone(&sink);
        async move {
            if offset >= body.len() || cancellation.is_cancelled() {
                return None;
            }

            let next_offset = (offset + UPLOAD_CHUNK_BYTES).min(body.len());
            let chunk = Bytes::copy_from_slice(&body[offset..next_offset]);
            emit(
                &coordinator,
                &sink,
                execution_id,
                ExecutionEventKind::UploadProgress {
                    sent_bytes: next_offset as u64,
                    total_bytes,
                },
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
            Some((Ok(chunk), next_offset))
        }
    })
}

fn emit(
    coordinator: &ExecutionCoordinator,
    sink: &Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
    execution_id: ExecutionId,
    kind: ExecutionEventKind,
) {
    if let Some(event) = coordinator.record_event(execution_id, kind) {
        sink(event);
    }
}

struct CompletedHttpExecution {
    status: u16,
    body_preview: String,
    body_truncated: bool,
}

#[derive(Debug)]
enum HttpExecutionError {
    InvalidInput(&'static str),
    Transport,
    Response,
    Cancelled,
}

impl HttpExecutionError {
    fn safe_message(&self) -> String {
        match self {
            Self::InvalidInput(detail) => (*detail).to_owned(),
            Self::Transport => "transport.failed".to_owned(),
            Self::Response => "response.failed".to_owned(),
            Self::Cancelled => "cancelled".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::{mpsc, oneshot},
    };

    use super::*;
    use crate::{
        application::execution::{ExecutionCoordinator, ExecutionEventKind, ExecutionRequest},
        domain::request::{OrderedField, RequestContent, RequestDraftId},
    };

    #[tokio::test]
    async fn get_reaches_local_fixture_with_ordered_query_and_headers() {
        let (server, captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/echo?ignored=true"),
            query: vec![
                field(1, "tag", "second"),
                field(0, "tag", "first"),
                OrderedField {
                    enabled: false,
                    order: 2,
                    name: "skip".to_owned(),
                    value: "no".to_owned(),
                },
            ],
            headers: vec![
                field(1, "x-duplicate", "two"),
                field(0, "x-duplicate", "one"),
            ],
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.starts_with("GET /echo?tag=first&tag=second HTTP/1.1"));
        assert!(request.contains("\r\nx-duplicate: one\r\n"));
        assert!(request.contains("\r\nx-duplicate: two\r\n"));
        assert_terminal_completed(&events);
        assert_ordered_sequences(&events);
    }

    #[tokio::test]
    async fn post_reaches_local_fixture_with_raw_body_without_cors_headers() {
        let (server, captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/post"),
            body: "{\"ok\":true}".to_owned(),
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(
            request.starts_with("POST /post HTTP/1.1"),
            "unexpected request:\n{request}"
        );
        assert!(request.ends_with("{\"ok\":true}"));
        assert_terminal_completed(&events);
        assert!(events.iter().any(|event| {
            matches!(
                event.kind,
                ExecutionEventKind::UploadProgress {
                    sent_bytes: 11,
                    total_bytes: 11
                }
            )
        }));
    }

    #[tokio::test]
    async fn cancel_before_connect_emits_one_terminal_event() {
        let (server, _captured) = start_fixture(FixtureMode::SlowHeaders).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            content: RequestContent {
                url: server.url("/slow"),
                ..RequestContent::blank()
            },
        };
        let sink = event_sink(&events);
        let started = coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        coordinator
            .cancel(started.execution_id)
            .expect("cancel execution");
        let events = wait_for_terminal(events).await;

        assert_one_cancelled_terminal(&events);
    }

    #[tokio::test]
    async fn cancel_during_download_emits_one_terminal_event() {
        let (server, _captured) = start_fixture(FixtureMode::SlowDownload).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            content: RequestContent {
                url: server.url("/download"),
                ..RequestContent::blank()
            },
        };
        let sink = event_sink(&events);
        let started = coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        wait_until(&events, |events| {
            events
                .iter()
                .any(|event| matches!(event.kind, ExecutionEventKind::DownloadProgress { .. }))
        })
        .await;
        coordinator
            .cancel(started.execution_id)
            .expect("cancel execution");
        let events = wait_for_terminal(events).await;

        assert_one_cancelled_terminal(&events);
    }

    #[tokio::test]
    async fn cancel_during_upload_emits_one_terminal_event() {
        let (server, _captured) = start_fixture(FixtureMode::SlowUploadRead).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            content: RequestContent {
                method: "POST".to_owned(),
                url: server.url("/upload"),
                body: "x".repeat(128 * 1024),
                ..RequestContent::blank()
            },
        };
        let sink = event_sink(&events);
        let started = coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        wait_until(&events, |events| {
            events
                .iter()
                .any(|event| matches!(event.kind, ExecutionEventKind::UploadProgress { .. }))
        })
        .await;
        coordinator
            .cancel(started.execution_id)
            .expect("cancel execution");
        let events = wait_for_terminal(events).await;

        assert_one_cancelled_terminal(&events);
    }

    async fn run_fixture_request(content: RequestContent) -> Vec<ExecutionEvent> {
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            content,
        };
        let sink = event_sink(&events);

        coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        wait_for_terminal(events).await
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
        wait_until(&events, |events| {
            events.iter().any(|event| event.kind.is_terminal())
        })
        .await;
        events.lock().expect("lock events").clone()
    }

    async fn wait_until(
        events: &Arc<Mutex<Vec<ExecutionEvent>>>,
        predicate: impl Fn(&[ExecutionEvent]) -> bool,
    ) {
        for _ in 0..300 {
            {
                let events = events.lock().expect("lock events");
                if predicate(&events) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for execution event");
    }

    fn assert_terminal_completed(events: &[ExecutionEvent]) {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind.is_terminal())
                .count(),
            1
        );
        assert!(events
            .iter()
            .any(|event| matches!(event.kind, ExecutionEventKind::Completed { .. })));
    }

    fn assert_one_cancelled_terminal(events: &[ExecutionEvent]) {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind.is_terminal())
                .count(),
            1
        );
        assert!(events
            .iter()
            .any(|event| matches!(event.kind, ExecutionEventKind::Cancelled)));
    }

    fn assert_ordered_sequences(events: &[ExecutionEvent]) {
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event.sequence, index as u64 + 1);
        }
    }

    fn field(order: u32, name: &str, value: &str) -> OrderedField {
        OrderedField {
            enabled: true,
            order,
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }

    struct FixtureServer {
        address: String,
    }

    impl FixtureServer {
        fn url(&self, path: &str) -> String {
            format!("http://{}{}", self.address, path)
        }
    }

    enum FixtureMode {
        Echo,
        SlowHeaders,
        SlowDownload,
        SlowUploadRead,
    }

    async fn start_fixture(mode: FixtureMode) -> (FixtureServer, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address").to_string();
        let (captured_tx, captured_rx) = oneshot::channel();
        let (ready_tx, mut ready_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            ready_tx.send(()).await.expect("signal fixture readiness");
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            let request = read_http_request(&mut stream).await;
            let _ = captured_tx.send(request);

            match mode {
                FixtureMode::Echo => {
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: text/plain\r\n\r\nok",
                        )
                        .await
                        .expect("write echo response");
                }
                FixtureMode::SlowHeaders => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .await;
                }
                FixtureMode::SlowDownload => {
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\none")
                        .await
                        .expect("write first chunk");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = stream.write_all(b"two").await;
                }
                FixtureMode::SlowUploadRead => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .await;
                }
            }
        });

        ready_rx.recv().await.expect("fixture ready");
        (FixtureServer { address }, captured_rx)
    }

    async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
        let mut buffer = Vec::new();
        let mut temp = [0_u8; 4096];
        loop {
            let read = stream.read(&mut temp).await.expect("read request");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&temp[..read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let header_end = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
            .expect("header terminator");
        let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length: ")
                    .or_else(|| line.strip_prefix("Content-Length: "))
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);

        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut temp).await.expect("read body");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&temp[..read]);
        }

        String::from_utf8_lossy(&buffer).into_owned()
    }
}
