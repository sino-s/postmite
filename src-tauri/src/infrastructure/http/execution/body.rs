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
    decoded_bytes: u64,
    wire_bytes: Option<u64>,
    response_file: Option<ResponseFileMetadata>,
    timing: ExecutionTimingMetadata,
}

#[derive(Debug)]
pub enum HttpExecutionError {
    InvalidInput(&'static str),
    BodyFileChanged,
    BodyFileMissing,
    Certificate,
    Transport,
    Timeout(TimeoutKind),
    Response,
    ResponseTooLarge,
    Cancelled,
}

#[derive(Clone, Copy, Debug)]
pub enum TimeoutKind {
    Connect,
    Overall,
    Idle,
}

impl HttpExecutionError {
    pub fn safe_message(&self) -> String {
        match self {
            Self::InvalidInput(detail) => (*detail).to_owned(),
            Self::BodyFileChanged => "body.file.changed".to_owned(),
            Self::BodyFileMissing => "body.file.missing".to_owned(),
            Self::Certificate => "certificate.invalid".to_owned(),
            Self::Transport => "transport.failed".to_owned(),
            Self::Timeout(TimeoutKind::Connect) => "timeout.connect".to_owned(),
            Self::Timeout(TimeoutKind::Overall) => "timeout.overall".to_owned(),
            Self::Timeout(TimeoutKind::Idle) => "timeout.idle".to_owned(),
            Self::Response => "response.failed".to_owned(),
            Self::ResponseTooLarge => "response.too_large".to_owned(),
            Self::Cancelled => "cancelled".to_owned(),
        }
    }
}
