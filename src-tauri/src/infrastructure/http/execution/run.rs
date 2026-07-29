use std::{
    env,
    fs::{self, File as StdFile, OpenOptions as StdOpenOptions},
    io::{BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bytes::Bytes;
use futures_util::{stream, StreamExt, TryStreamExt};
use reqwest::{
    header::{
        HeaderMap, HeaderName, HeaderValue, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH,
        LOCATION,
    },
    multipart, Method, NoProxy, Proxy, Url, Version,
};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;

use crate::{
    application::execution::{
        ExecutionCoordinator, ExecutionEvent, ExecutionEventKind, ExecutionHeader, ExecutionId,
        ExecutionProxyMetadata, ExecutionRequest, ExecutionTimeoutMetadata,
        ExecutionTimingMetadata, ResponseFileMetadata, MAX_NORMAL_RESPONSE_DECODED_BYTES,
        MAX_RESPONSE_PREVIEW_BYTES, RESPONSE_TEMP_RETENTION_SECONDS,
    },
    domain::request::{
        BodyFilePath, BodyFileReference, MultipartPart, OrderedField, ProxySource, RequestBody,
        RequestContent, TimeoutPolicy,
    },
};

const UPLOAD_CHUNK_BYTES: usize = 16 * 1024;
static RESPONSE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

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
                decoded_bytes: completed.decoded_bytes,
                wire_bytes: completed.wire_bytes,
                response_file: completed.response_file,
                timing: completed.timing,
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
    let started_at = Instant::now();
    let mut method = Method::from_bytes(content.method.as_bytes())
        .map_err(|_| HttpExecutionError::InvalidInput("method.invalid"))?;
    let mut url = resolve_url(content)?;
    let mut headers = resolve_headers(&content.headers)?;
    if !headers.contains_key(ACCEPT_ENCODING) {
        headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, br, deflate, zstd"),
        );
    }

    emit(
        &coordinator,
        &sink,
        execution_id,
        ExecutionEventKind::Started {
            method: method.as_str().to_owned(),
            url: url.to_string(),
            tls_verification: content.tls.verify,
            proxy: proxy_metadata(content, &url),
            timeouts: timeout_metadata(&content.transport.timeouts),
            queued_ms: coordinator.queued_ms(execution_id),
        },
    );

    let client = build_client(content, base_directory)?;
    let max_redirects = if content.redirect.enabled {
        content.redirect.max_redirects.min(10)
    } else {
        0
    };
    let mut redirect_count = 0_u8;
    let mut include_body = true;
    let empty_body = RequestBody::None;
    let (response, first_byte_ms) = loop {
        let request_body = if include_body {
            &content.body
        } else {
            &empty_body
        };
        let body = build_request_body(
            request_body,
            base_directory,
            cancellation.clone(),
            execution_id,
            Arc::clone(&coordinator),
            Arc::clone(&sink),
        )
        .await?;
        let builder = apply_body(
            client
                .request(method.clone(), url.clone())
                .headers(headers.clone()),
            body,
            cancellation.clone(),
            execution_id,
            Arc::clone(&coordinator),
            Arc::clone(&sink),
        );

        let send_started_at = Instant::now();
        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(HttpExecutionError::Cancelled),
            result = builder.send() => match result {
                Ok(response) => response,
                Err(_) if cancellation.is_cancelled() => return Err(HttpExecutionError::Cancelled),
                Err(error) if error.is_timeout() => {
                    let timeout = if error.is_connect() {
                        TimeoutKind::Connect
                    } else {
                        TimeoutKind::Overall
                    };
                    return Err(HttpExecutionError::Timeout(timeout));
                }
                Err(error) if error.is_request() || error.is_connect() => {
                    return Err(HttpExecutionError::Transport);
                }
                Err(_) => return Err(HttpExecutionError::Response),
            },
        };
        let first_byte_ms = send_started_at.elapsed().as_millis() as u64;

        if !response.status().is_redirection() {
            break (response, first_byte_ms);
        }
        if redirect_count >= max_redirects {
            break (response, first_byte_ms);
        }
        let Some(next_url) = redirect_target(response.url(), response.headers())? else {
            break (response, first_byte_ms);
        };
        let status = response.status();
        let from = url.to_string();
        let to = next_url.to_string();
        let next_method = redirect_method(&method, status.as_u16());
        if next_method != method {
            include_body = false;
        }
        method = next_method;
        url = next_url;
        redirect_count = redirect_count.saturating_add(1);
        emit(
            &coordinator,
            &sink,
            execution_id,
            ExecutionEventKind::Redirected {
                from,
                to,
                status: status.as_u16(),
            },
        );
    };

    let status = response.status().as_u16();
    let wire_bytes = response_wire_bytes(response.headers());
    let content_encoding = response_content_encoding(response.headers());
    let protocol = http_version(response.version()).to_owned();
    let remote_addr = response.remote_addr().map(|address| address.to_string());
    let headers = response_headers(response.headers());
    emit(
        &coordinator,
        &sink,
        execution_id,
        ExecutionEventKind::ResponseHeaders {
            status,
            headers,
            protocol,
            remote_addr,
        },
    );

    let encoded_response = content_encoding.is_some();
    let mut stream = response.bytes_stream();
    let mut response_body = ResponseBodyCollector::new(
        execution_id,
        ResponseCollectionLimits::normal(),
        SystemTime::now(),
    )?;
    let mut encoded_body_file = if encoded_response {
        Some(TemporaryResponseFile::create(execution_id, "encoded")?)
    } else {
        None
    };
    let mut received_bytes = 0_u64;
    let download_started_at = Instant::now();

    loop {
        let chunk = tokio::select! {
            _ = cancellation.cancelled() => return Err(HttpExecutionError::Cancelled),
            chunk = stream.next() => chunk,
        };

        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk.map_err(|error| {
            if cancellation.is_cancelled() {
                HttpExecutionError::Cancelled
            } else if error.is_timeout() {
                HttpExecutionError::Timeout(TimeoutKind::Idle)
            } else {
                HttpExecutionError::Response
            }
        })?;
        received_bytes += chunk.len() as u64;
        if let Some(file) = encoded_body_file.as_mut() {
            file.write_all(&chunk)?;
        } else {
            response_body.push(&chunk)?;
        }

        emit(
            &coordinator,
            &sink,
            execution_id,
            ExecutionEventKind::DownloadProgress {
                received_bytes,
                total_bytes: wire_bytes,
            },
        );
    }

    if let Some(mut file) = encoded_body_file.take() {
        file.flush()?;
        decode_response_body_from_path(
            file.path(),
            content_encoding.as_deref(),
            &mut response_body,
        )?;
    }
    let collected_body = response_body.finish()?;

    Ok(CompletedHttpExecution {
        status,
        body_preview: String::from_utf8_lossy(&collected_body.preview).into_owned(),
        body_truncated: collected_body.truncated,
        decoded_bytes: collected_body.decoded_bytes,
        wire_bytes: wire_bytes.or(Some(received_bytes)),
        response_file: collected_body.response_file,
        timing: ExecutionTimingMetadata {
            queued_ms: coordinator.queued_ms(execution_id),
            dns_ms: None,
            connect_ms: None,
            tls_ms: None,
            first_byte_ms: Some(first_byte_ms),
            download_ms: Some(download_started_at.elapsed().as_millis() as u64),
            total_ms: started_at.elapsed().as_millis() as u64,
        },
    })
}

pub fn cleanup_expired_response_temp_files(now: SystemTime) {
    let _ = cleanup_response_temp_files(response_temp_dir(), now, response_temp_retention());
}

pub fn cleanup_all_response_temp_files() {
    let _ = cleanup_response_temp_files(response_temp_dir(), SystemTime::now(), Duration::ZERO);
}

pub fn save_response_file(
    source_path: &Path,
    destination_path: &Path,
    now: SystemTime,
) -> Result<u64, HttpExecutionError> {
    let source = validate_response_temp_source(source_path, now)?;
    if destination_path.as_os_str().is_empty()
        || !destination_path.is_absolute()
        || destination_path.is_dir()
    {
        return Err(HttpExecutionError::InvalidInput(
            "response.file.destination.invalid",
        ));
    }
    if source == destination_path {
        return Err(HttpExecutionError::InvalidInput(
            "response.file.destination.invalid",
        ));
    }
    fs::copy(source, destination_path).map_err(|_| HttpExecutionError::Response)
}

fn cleanup_response_temp_files(
    directory: PathBuf,
    now: SystemTime,
    retention: Duration,
) -> Result<(), HttpExecutionError> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(|_| HttpExecutionError::Response)?;
        let metadata = entry.metadata().map_err(|_| HttpExecutionError::Response)?;
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if now
            .duration_since(modified)
            .unwrap_or_default()
            .ge(&retention)
        {
            fs::remove_file(entry.path()).map_err(|_| HttpExecutionError::Response)?;
        }
    }
    Ok(())
}

fn validate_response_temp_source(
    source_path: &Path,
    now: SystemTime,
) -> Result<PathBuf, HttpExecutionError> {
    let source = source_path
        .canonicalize()
        .map_err(|_| HttpExecutionError::InvalidInput("response.file.source.invalid"))?;
    let temp_dir = response_temp_dir()
        .canonicalize()
        .map_err(|_| HttpExecutionError::InvalidInput("response.file.source.invalid"))?;
    if source.parent() != Some(temp_dir.as_path()) {
        return Err(HttpExecutionError::InvalidInput(
            "response.file.source.invalid",
        ));
    }
    let metadata = fs::metadata(&source)
        .map_err(|_| HttpExecutionError::InvalidInput("response.file.source.invalid"))?;
    if !metadata.is_file() {
        return Err(HttpExecutionError::InvalidInput(
            "response.file.source.invalid",
        ));
    }
    let modified = metadata
        .modified()
        .map_err(|_| HttpExecutionError::InvalidInput("response.file.source.invalid"))?;
    if now
        .duration_since(modified)
        .unwrap_or_default()
        .ge(&response_temp_retention())
    {
        return Err(HttpExecutionError::InvalidInput(
            "response.file.source.expired",
        ));
    }
    Ok(source)
}
