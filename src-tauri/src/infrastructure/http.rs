use std::{
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use bytes::Bytes;
use futures_util::{stream, StreamExt, TryStreamExt};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    multipart, Method, Url,
};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;

use crate::{
    application::execution::{
        ExecutionCoordinator, ExecutionEvent, ExecutionEventKind, ExecutionHeader, ExecutionId,
        ExecutionRequest, MAX_RESPONSE_PREVIEW_BYTES,
    },
    domain::request::{
        BodyFilePath, BodyFileReference, MultipartPart, OrderedField, RequestBody, RequestContent,
    },
};

const UPLOAD_CHUNK_BYTES: usize = 16 * 1024;

pub async fn run_http_execution(
    execution_id: ExecutionId,
    request: ExecutionRequest,
    cancellation: CancellationToken,
    coordinator: Arc<ExecutionCoordinator>,
    sink: Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
) {
    let base_directory = request.workspace_base_directory.clone();
    let content = request.content;
    let result = execute_http(
        execution_id,
        &content,
        base_directory.as_deref(),
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
    base_directory: Option<&str>,
    cancellation: CancellationToken,
    coordinator: Arc<ExecutionCoordinator>,
    sink: Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
) -> Result<CompletedHttpExecution, HttpExecutionError> {
    let method = Method::from_bytes(content.method.as_bytes())
        .map_err(|_| HttpExecutionError::InvalidInput("method.invalid"))?;
    let url = resolve_url(content)?;
    let headers = resolve_headers(&content.headers)?;
    let body = build_request_body(
        &content.body,
        base_directory,
        cancellation.clone(),
        execution_id,
        Arc::clone(&coordinator),
        Arc::clone(&sink),
    )
    .await?;

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
    match body {
        BuiltRequestBody::None => {}
        BuiltRequestBody::Bytes {
            bytes,
            content_type,
        } => {
            let total_bytes = bytes.len() as u64;
            let body_stream = cancellable_upload_stream(
                bytes,
                cancellation.clone(),
                execution_id,
                Arc::clone(&coordinator),
                Arc::clone(&sink),
            );
            builder = builder.body(reqwest::Body::wrap_stream(body_stream));
            builder = builder.header(reqwest::header::CONTENT_LENGTH, total_bytes);
            if let Some(content_type) = content_type {
                builder = builder.header(reqwest::header::CONTENT_TYPE, content_type);
            }
        }
        BuiltRequestBody::Stream {
            body,
            content_length,
            content_type,
        } => {
            builder = builder.body(body);
            builder = builder.header(reqwest::header::CONTENT_LENGTH, content_length);
            if let Some(content_type) = content_type {
                builder = builder.header(reqwest::header::CONTENT_TYPE, content_type);
            }
        }
        BuiltRequestBody::Multipart { form } => {
            builder = builder.multipart(form);
        }
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

enum BuiltRequestBody {
    None,
    Bytes {
        bytes: Vec<u8>,
        content_type: Option<&'static str>,
    },
    Stream {
        body: reqwest::Body,
        content_length: u64,
        content_type: Option<&'static str>,
    },
    Multipart {
        form: multipart::Form,
    },
}

async fn build_request_body(
    body: &RequestBody,
    base_directory: Option<&str>,
    cancellation: CancellationToken,
    execution_id: ExecutionId,
    coordinator: Arc<ExecutionCoordinator>,
    sink: Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
) -> Result<BuiltRequestBody, HttpExecutionError> {
    match body {
        RequestBody::None => Ok(BuiltRequestBody::None),
        RequestBody::Raw { content } => Ok(BuiltRequestBody::Bytes {
            bytes: content.as_bytes().to_vec(),
            content_type: None,
        }),
        RequestBody::UrlEncoded { fields } => {
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for field in sorted_enabled_fields(fields) {
                serializer.append_pair(&field.name, &field.value);
            }
            Ok(BuiltRequestBody::Bytes {
                bytes: serializer.finish().into_bytes(),
                content_type: Some("application/x-www-form-urlencoded"),
            })
        }
        RequestBody::Binary { file } => {
            let resolved = resolve_body_file_path(file, base_directory)?;
            ensure_body_file_current(file, &resolved).await?;
            let body = file_upload_body(
                &resolved,
                file.size,
                cancellation,
                execution_id,
                coordinator,
                sink,
            )
            .await?;
            Ok(BuiltRequestBody::Stream {
                body,
                content_length: file.size,
                content_type: None,
            })
        }
        RequestBody::Multipart { parts } => {
            let mut form = multipart::Form::new();
            for part in parts.iter().filter(|part| match part {
                MultipartPart::Field { enabled, .. } | MultipartPart::File { enabled, .. } => {
                    *enabled
                }
            }) {
                match part {
                    MultipartPart::Field { name, value, .. } => {
                        form = form.text(name.clone(), value.clone());
                    }
                    MultipartPart::File { name, file, .. } => {
                        let resolved = resolve_body_file_path(file, base_directory)?;
                        ensure_body_file_current(file, &resolved).await?;
                        let part_body = file_upload_body(
                            &resolved,
                            file.size,
                            cancellation.clone(),
                            execution_id,
                            Arc::clone(&coordinator),
                            Arc::clone(&sink),
                        )
                        .await?;
                        form = form.part(
                            name.clone(),
                            multipart::Part::stream_with_length(part_body, file.size)
                                .file_name(file.file_name.clone()),
                        );
                    }
                }
            }
            Ok(BuiltRequestBody::Multipart { form })
        }
    }
}

async fn file_upload_body(
    path: &Path,
    total_bytes: u64,
    cancellation: CancellationToken,
    execution_id: ExecutionId,
    coordinator: Arc<ExecutionCoordinator>,
    sink: Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
) -> Result<reqwest::Body, HttpExecutionError> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| HttpExecutionError::BodyFileMissing)?;
    let sent = Arc::new(AtomicU64::new(0));
    let stream = ReaderStream::new(file)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::Other))
        .map_ok({
            let sent = Arc::clone(&sent);
            move |chunk| {
                let next =
                    sent.fetch_add(chunk.len() as u64, Ordering::Relaxed) + chunk.len() as u64;
                emit(
                    &coordinator,
                    &sink,
                    execution_id,
                    ExecutionEventKind::UploadProgress {
                        sent_bytes: next,
                        total_bytes,
                    },
                );
                chunk
            }
        })
        .take_until(cancellation.cancelled_owned());
    Ok(reqwest::Body::wrap_stream(stream))
}

async fn ensure_body_file_current(
    reference: &BodyFileReference,
    path: &Path,
) -> Result<(), HttpExecutionError> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|_| HttpExecutionError::BodyFileMissing)?;
    if !metadata.is_file() {
        return Err(HttpExecutionError::BodyFileMissing);
    }
    if metadata.len() != reference.size {
        return Err(HttpExecutionError::BodyFileChanged);
    }
    if !reference.sha256.is_empty() && sha256_file(path).await? != reference.sha256 {
        return Err(HttpExecutionError::BodyFileChanged);
    }
    Ok(())
}

async fn sha256_file(path: &Path) -> Result<String, HttpExecutionError> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| HttpExecutionError::BodyFileMissing)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; UPLOAD_CHUNK_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|_| HttpExecutionError::BodyFileMissing)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn resolve_body_file_path(
    reference: &BodyFileReference,
    base_directory: Option<&str>,
) -> Result<PathBuf, HttpExecutionError> {
    match &reference.path {
        BodyFilePath::Absolute { path } => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                Ok(path)
            } else {
                Err(HttpExecutionError::InvalidInput(
                    "body.file.absolute.invalid",
                ))
            }
        }
        BodyFilePath::Relative { path } => {
            if path_has_unsafe_components(path) {
                return Err(HttpExecutionError::InvalidInput(
                    "body.file.relative.invalid",
                ));
            }
            let Some(base_directory) = base_directory else {
                return Err(HttpExecutionError::InvalidInput(
                    "workspace.baseDirectory.required",
                ));
            };
            Ok(PathBuf::from(base_directory).join(path))
        }
    }
}

fn path_has_unsafe_components(path: &str) -> bool {
    let path = Path::new(path);
    path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
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
    BodyFileChanged,
    BodyFileMissing,
    Transport,
    Response,
    Cancelled,
}

impl HttpExecutionError {
    fn safe_message(&self) -> String {
        match self {
            Self::InvalidInput(detail) => (*detail).to_owned(),
            Self::BodyFileChanged => "body.file.changed".to_owned(),
            Self::BodyFileMissing => "body.file.missing".to_owned(),
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
        domain::request::{
            BodyFilePath, BodyFileReference, MultipartPart, OrderedField, RequestBody,
            RequestContent, RequestDraftId,
        },
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
            body: RequestBody::Raw {
                content: "{\"ok\":true}".to_owned(),
            },
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
    async fn url_encoded_body_reaches_local_fixture_byte_for_byte() {
        let (server, captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/form"),
            body: RequestBody::UrlEncoded {
                fields: vec![field(1, "space", "a b"), field(0, "tag", "first")],
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.contains("content-type: application/x-www-form-urlencoded"));
        assert!(request.ends_with("tag=first&space=a+b"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn binary_body_streams_file_to_local_fixture_byte_for_byte() {
        let file = tempfile::NamedTempFile::new().expect("temporary body file");
        std::fs::write(file.path(), b"binary-payload").expect("write body file");
        let metadata = std::fs::metadata(file.path()).expect("body metadata");
        let (server, captured) = start_fixture(FixtureMode::Echo).await;

        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/binary"),
            body: RequestBody::Binary {
                file: BodyFileReference {
                    path: BodyFilePath::Absolute {
                        path: file.path().to_string_lossy().into_owned(),
                    },
                    file_name: "payload.bin".to_owned(),
                    size: metadata.len(),
                    modified_at_epoch_seconds: None,
                    sha256: hash_bytes(b"binary-payload"),
                },
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.starts_with("POST /binary HTTP/1.1"));
        assert!(request.ends_with("binary-payload"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn relative_binary_body_survives_base_directory_move() {
        let base = tempfile::TempDir::new().expect("base directory");
        let file_path = base.path().join("payload.bin");
        std::fs::write(&file_path, b"relative-payload").expect("write body file");
        let metadata = std::fs::metadata(&file_path).expect("body metadata");
        let (server, captured) = start_fixture(FixtureMode::Echo).await;

        let events = run_fixture_request_with_base(
            RequestContent {
                method: "POST".to_owned(),
                url: server.url("/relative"),
                body: RequestBody::Binary {
                    file: BodyFileReference {
                        path: BodyFilePath::Relative {
                            path: "payload.bin".to_owned(),
                        },
                        file_name: "payload.bin".to_owned(),
                        size: metadata.len(),
                        modified_at_epoch_seconds: None,
                        sha256: hash_bytes(b"relative-payload"),
                    },
                },
                ..RequestContent::blank()
            },
            Some(base.path().to_string_lossy().into_owned()),
        )
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.ends_with("relative-payload"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn changed_or_missing_body_files_fail_before_upload() {
        let changed = tempfile::NamedTempFile::new().expect("changed body file");
        std::fs::write(changed.path(), b"changed").expect("write body file");
        let (server, _captured) = start_fixture(FixtureMode::Echo).await;
        let changed_events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/changed"),
            body: RequestBody::Binary {
                file: BodyFileReference {
                    path: BodyFilePath::Absolute {
                        path: changed.path().to_string_lossy().into_owned(),
                    },
                    file_name: "payload.bin".to_owned(),
                    size: 999,
                    modified_at_epoch_seconds: None,
                    sha256: hash_bytes(b"original"),
                },
            },
            ..RequestContent::blank()
        })
        .await;
        assert!(changed_events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "body.file.changed"
        )));

        let missing_path = changed.path().with_file_name("missing.bin");
        let (server, _captured) = start_fixture(FixtureMode::Echo).await;
        let missing_events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/missing"),
            body: RequestBody::Binary {
                file: BodyFileReference {
                    path: BodyFilePath::Absolute {
                        path: missing_path.to_string_lossy().into_owned(),
                    },
                    file_name: "missing.bin".to_owned(),
                    size: 7,
                    modified_at_epoch_seconds: None,
                    sha256: hash_bytes(b"missing"),
                },
            },
            ..RequestContent::blank()
        })
        .await;
        assert!(missing_events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "body.file.missing"
        )));
    }

    #[tokio::test]
    async fn multipart_body_streams_fields_and_files_to_local_fixture() {
        let file = tempfile::NamedTempFile::new().expect("temporary body file");
        std::fs::write(file.path(), b"file-payload").expect("write body file");
        let metadata = std::fs::metadata(file.path()).expect("body metadata");
        let (server, captured) = start_fixture(FixtureMode::Echo).await;

        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/multipart"),
            body: RequestBody::Multipart {
                parts: vec![
                    MultipartPart::Field {
                        enabled: true,
                        order: 0,
                        name: "kind".to_owned(),
                        value: "fixture".to_owned(),
                    },
                    MultipartPart::File {
                        enabled: true,
                        order: 1,
                        name: "upload".to_owned(),
                        file: BodyFileReference {
                            path: BodyFilePath::Absolute {
                                path: file.path().to_string_lossy().into_owned(),
                            },
                            file_name: "payload.txt".to_owned(),
                            size: metadata.len(),
                            modified_at_epoch_seconds: None,
                            sha256: hash_bytes(b"file-payload"),
                        },
                    },
                ],
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        let lower_request = request.to_ascii_lowercase();
        assert!(lower_request.contains("content-disposition: form-data; name=\"kind\""));
        assert!(request.contains("fixture"));
        assert!(lower_request
            .contains("content-disposition: form-data; name=\"upload\"; filename=\"payload.txt\""));
        assert!(request.contains("file-payload"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn cancel_before_connect_emits_one_terminal_event() {
        let (server, _captured) = start_fixture(FixtureMode::SlowHeaders).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            workspace_base_directory: None,
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
            workspace_base_directory: None,
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
            workspace_base_directory: None,
            content: RequestContent {
                method: "POST".to_owned(),
                url: server.url("/upload"),
                body: RequestBody::Raw {
                    content: "x".repeat(128 * 1024),
                },
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
        run_fixture_request_with_base(content, None).await
    }

    async fn run_fixture_request_with_base(
        content: RequestContent,
        workspace_base_directory: Option<String>,
    ) -> Vec<ExecutionEvent> {
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            workspace_base_directory,
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

    fn hash_bytes(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
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
