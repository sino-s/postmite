#[derive(Clone, Copy)]
struct ResponseCollectionLimits {
    preview_bytes: usize,
    normal_decoded_bytes: u64,
}

impl ResponseCollectionLimits {
    fn normal() -> Self {
        Self {
            preview_bytes: MAX_RESPONSE_PREVIEW_BYTES,
            normal_decoded_bytes: MAX_NORMAL_RESPONSE_DECODED_BYTES,
        }
    }
}

struct CollectedResponseBody {
    preview: Vec<u8>,
    truncated: bool,
    decoded_bytes: u64,
    response_file: Option<ResponseFileMetadata>,
}

struct ResponseBodyCollector {
    execution_id: ExecutionId,
    limits: ResponseCollectionLimits,
    created_at: SystemTime,
    preview: Vec<u8>,
    decoded_bytes: u64,
    spool: Option<TemporaryResponseFile>,
}

impl ResponseBodyCollector {
    fn new(
        execution_id: ExecutionId,
        limits: ResponseCollectionLimits,
        created_at: SystemTime,
    ) -> Result<Self, HttpExecutionError> {
        fs::create_dir_all(response_temp_dir()).map_err(|_| HttpExecutionError::Response)?;
        Ok(Self {
            execution_id,
            limits,
            created_at,
            preview: Vec::new(),
            decoded_bytes: 0,
            spool: None,
        })
    }

    fn push(&mut self, bytes: &[u8]) -> Result<(), HttpExecutionError> {
        let next_decoded_bytes = self
            .decoded_bytes
            .checked_add(bytes.len() as u64)
            .ok_or(HttpExecutionError::ResponseTooLarge)?;
        if next_decoded_bytes > self.limits.normal_decoded_bytes {
            return Err(HttpExecutionError::ResponseTooLarge);
        }

        if self.spool.is_none()
            && self.decoded_bytes as usize + bytes.len() > self.limits.preview_bytes
        {
            let mut spool = TemporaryResponseFile::create(self.execution_id, "decoded")?;
            spool.write_all(&self.preview)?;
            self.spool = Some(spool);
        }

        if let Some(spool) = self.spool.as_mut() {
            spool.write_all(bytes)?;
        }

        let remaining_preview = self.limits.preview_bytes.saturating_sub(self.preview.len());
        if remaining_preview > 0 {
            let preview_len = bytes.len().min(remaining_preview);
            self.preview.extend_from_slice(&bytes[..preview_len]);
        }
        self.decoded_bytes = next_decoded_bytes;
        Ok(())
    }

    fn finish(mut self) -> Result<CollectedResponseBody, HttpExecutionError> {
        let response_file = if let Some(mut spool) = self.spool.take() {
            spool.flush()?;
            let path = spool.path().to_path_buf();
            spool.persist();
            Some(ResponseFileMetadata {
                path: path.to_string_lossy().into_owned(),
                byte_count: self.decoded_bytes,
                expires_at_epoch_seconds: expires_at_epoch_seconds(self.created_at),
            })
        } else {
            None
        };
        Ok(CollectedResponseBody {
            preview: self.preview,
            truncated: response_file.is_some(),
            decoded_bytes: self.decoded_bytes,
            response_file,
        })
    }
}

struct TemporaryResponseFile {
    path: PathBuf,
    file: StdFile,
    delete_on_drop: bool,
}

impl TemporaryResponseFile {
    fn create(execution_id: ExecutionId, purpose: &str) -> Result<Self, HttpExecutionError> {
        fs::create_dir_all(response_temp_dir()).map_err(|_| HttpExecutionError::Response)?;
        let counter = RESPONSE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            response_temp_dir().join(format!("response-{execution_id}-{counter}-{purpose}.tmp"));
        let file = StdOpenOptions::new()
            .create_new(true)
            .write(true)
            .read(true)
            .open(&path)
            .map_err(|_| HttpExecutionError::Response)?;
        Ok(Self {
            path,
            file,
            delete_on_drop: true,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), HttpExecutionError> {
        self.file
            .write_all(bytes)
            .map_err(|_| HttpExecutionError::Response)
    }

    fn flush(&mut self) -> Result<(), HttpExecutionError> {
        self.file.flush().map_err(|_| HttpExecutionError::Response)
    }

    fn persist(&mut self) {
        self.delete_on_drop = false;
    }
}

impl Drop for TemporaryResponseFile {
    fn drop(&mut self) {
        if self.delete_on_drop {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn response_temp_dir() -> PathBuf {
    env::temp_dir().join("postmite-response-files")
}

fn response_temp_retention() -> Duration {
    Duration::from_secs(RESPONSE_TEMP_RETENTION_SECONDS)
}

fn expires_at_epoch_seconds(created_at: SystemTime) -> u64 {
    created_at
        .checked_add(response_temp_retention())
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(RESPONSE_TEMP_RETENTION_SECONDS)
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

fn response_wire_bytes(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

fn response_content_encoding(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn decode_response_body_from_path(
    path: &Path,
    content_encoding: Option<&str>,
    collector: &mut ResponseBodyCollector,
) -> Result<(), HttpExecutionError> {
    let file = StdFile::open(path).map_err(|_| HttpExecutionError::Response)?;
    let reader = BufReader::new(file);
    match content_encoding {
        Some("gzip") => {
            let mut decoder = flate2::read::GzDecoder::new(reader);
            read_decoded_body(&mut decoder, collector)
        }
        Some("deflate") => {
            let mut decoder = flate2::read::ZlibDecoder::new(reader);
            read_decoded_body(&mut decoder, collector)
        }
        Some("br") => {
            let mut decoder = brotli::Decompressor::new(reader, 4096);
            read_decoded_body(&mut decoder, collector)
        }
        Some("zstd") => {
            let mut decoder =
                zstd::stream::Decoder::new(reader).map_err(|_| HttpExecutionError::Response)?;
            read_decoded_body(&mut decoder, collector)
        }
        _ => read_decoded_body(
            &mut BufReader::new(StdFile::open(path).map_err(|_| HttpExecutionError::Response)?),
            collector,
        ),
    }
}

fn read_decoded_body(
    reader: &mut impl Read,
    collector: &mut ResponseBodyCollector,
) -> Result<(), HttpExecutionError> {
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| HttpExecutionError::Response)?;
        if read == 0 {
            break;
        }
        collector.push(&buffer[..read])?;
    }
    Ok(())
}

fn http_version(version: Version) -> &'static str {
    match version {
        Version::HTTP_09 => "HTTP/0.9",
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        Version::HTTP_2 => "HTTP/2",
        Version::HTTP_3 => "HTTP/3",
        _ => "UNKNOWN",
    }
}

fn timeout_metadata(timeouts: &TimeoutPolicy) -> ExecutionTimeoutMetadata {
    ExecutionTimeoutMetadata {
        connect_ms: optional_timeout_ms(timeouts.connect_ms),
        overall_ms: optional_timeout_ms(timeouts.overall_ms),
        idle_ms: optional_timeout_ms(timeouts.idle_ms),
    }
}

fn optional_timeout_ms(timeout_ms: u64) -> Option<u64> {
    if timeout_ms == 0 {
        None
    } else {
        Some(timeout_ms)
    }
}

fn duration_from_ms(timeout_ms: u64) -> Option<Duration> {
    optional_timeout_ms(timeout_ms).map(Duration::from_millis)
}

fn proxy_metadata(content: &RequestContent, url: &Url) -> ExecutionProxyMetadata {
    let proxy = &content.transport.proxy;
    match proxy.source {
        ProxySource::Disabled => ExecutionProxyMetadata {
            source: "disabled".to_owned(),
            selected_proxy: None,
            bypass_reason: Some("proxy.disabled".to_owned()),
        },
        ProxySource::Custom => {
            if no_proxy_matches(url, &proxy.no_proxy) {
                return ExecutionProxyMetadata {
                    source: "custom".to_owned(),
                    selected_proxy: None,
                    bypass_reason: Some("no_proxy.custom".to_owned()),
                };
            }
            ExecutionProxyMetadata {
                source: "custom".to_owned(),
                selected_proxy: proxy
                    .url
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .map(redact_proxy_url),
                bypass_reason: None,
            }
        }
        ProxySource::ProcessEnvironment => {
            let no_proxy = env_no_proxy_entries();
            if no_proxy_matches(url, &no_proxy) {
                return ExecutionProxyMetadata {
                    source: "processEnvironment".to_owned(),
                    selected_proxy: None,
                    bypass_reason: Some("no_proxy.environment".to_owned()),
                };
            }
            ExecutionProxyMetadata {
                source: "processEnvironment".to_owned(),
                selected_proxy: env_proxy_for_url(url).map(|value| redact_proxy_url(&value)),
                bypass_reason: None,
            }
        }
    }
}

fn no_proxy_from_entries(entries: &[String]) -> Option<NoProxy> {
    let joined = entries
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>()
        .join(",");
    if joined.is_empty() {
        None
    } else {
        NoProxy::from_string(&joined)
    }
}

fn no_proxy_matches(url: &Url, entries: &[String]) -> bool {
    let Some(host) = url.host_str().map(|host| host.to_ascii_lowercase()) else {
        return false;
    };
    entries
        .iter()
        .flat_map(|entry| entry.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .any(|entry| no_proxy_entry_matches(&host, entry))
}

fn no_proxy_entry_matches(host: &str, entry: &str) -> bool {
    let entry = entry.trim().trim_start_matches('.').to_ascii_lowercase();
    entry == "*" || host == entry || host.ends_with(&format!(".{entry}"))
}

fn env_no_proxy_entries() -> Vec<String> {
    env::var("NO_PROXY")
        .or_else(|_| env::var("no_proxy"))
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn env_proxy_for_url(url: &Url) -> Option<String> {
    let scheme = url.scheme();
    let candidates = match scheme {
        "https" => ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"],
        _ => ["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"],
    };
    candidates.into_iter().find_map(|name| env::var(name).ok())
}

fn redact_proxy_url(value: &str) -> String {
    let Ok(url) = Url::parse(value) else {
        return "<invalid>".to_owned();
    };
    let Some(host) = url.host_str() else {
        return "<invalid>".to_owned();
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!("{}://{}{}", url.scheme(), host, port)
}
