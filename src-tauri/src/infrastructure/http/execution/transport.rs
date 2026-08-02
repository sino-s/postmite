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

fn apply_body(
    mut builder: reqwest::RequestBuilder,
    body: BuiltRequestBody,
    cancellation: CancellationToken,
    execution_id: ExecutionId,
    coordinator: Arc<ExecutionCoordinator>,
    sink: Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static>,
) -> reqwest::RequestBuilder {
    match body {
        BuiltRequestBody::None => {}
        BuiltRequestBody::Bytes {
            bytes,
            content_type,
        } => {
            let total_bytes = bytes.len() as u64;
            let body_stream =
                cancellable_upload_stream(bytes, cancellation, execution_id, coordinator, sink);
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
    builder
}

fn build_client(
    content: &RequestContent,
    base_directory: Option<&str>,
) -> Result<reqwest::Client, HttpExecutionError> {
    let mut builder = reqwest::Client::builder()
        .no_gzip()
        .no_brotli()
        .no_zstd()
        .no_deflate()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(!content.tls.verify);
    builder = builder.tls_backend_rustls();
    builder = apply_timeouts(builder, &content.transport.timeouts);
    builder = apply_proxy(builder, content)?;

    #[cfg(target_os = "windows")]
    let mut use_native_tls = false;
    if let Some(reference) = content.tls.custom_ca_reference.as_deref() {
        if !reference.trim().is_empty() {
            let bytes = std::fs::read(resolve_reference_path(reference, base_directory)?)
                .map_err(|_| HttpExecutionError::Certificate)?;
            let certificate = reqwest::Certificate::from_pem(&bytes)
                .map_err(|_| HttpExecutionError::Certificate)?;
            #[cfg(target_os = "windows")]
            {
                // Keep Windows' SChannel trust stores and policy active while
                // adding the user-provided private root for this client.
                builder = builder.tls_backend_native().http1_only();
                use_native_tls = true;
            }
            builder = builder.tls_certs_merge([certificate]);
        }
    }

    match (
        content.tls.client_certificate_reference.as_deref(),
        content.tls.client_key_reference.as_deref(),
    ) {
        (Some(certificate_reference), Some(key_reference))
            if !certificate_reference.trim().is_empty() && !key_reference.trim().is_empty() =>
        {
            let certificate_pem = std::fs::read(resolve_reference_path(
                certificate_reference,
                base_directory,
            )?)
            .map_err(|_| HttpExecutionError::Certificate)?;
            let key = std::fs::read(resolve_reference_path(key_reference, base_directory)?)
                .map_err(|_| HttpExecutionError::Certificate)?;
            let mut pem = certificate_pem.clone();
            pem.extend_from_slice(&key);
            #[cfg(target_os = "windows")]
            let identity = if use_native_tls {
                reqwest::Identity::from_pkcs8_pem(&certificate_pem, &key)
                    .map_err(|_| HttpExecutionError::Certificate)?
            } else {
                reqwest::Identity::from_pem(&pem)
                    .map_err(|_| HttpExecutionError::Certificate)?
            };
            #[cfg(not(target_os = "windows"))]
            let identity =
                reqwest::Identity::from_pem(&pem).map_err(|_| HttpExecutionError::Certificate)?;
            builder = builder.identity(identity);
        }
        _ => {}
    }

    builder.build().map_err(|_| HttpExecutionError::Transport)
}

fn apply_timeouts(
    mut builder: reqwest::ClientBuilder,
    timeouts: &TimeoutPolicy,
) -> reqwest::ClientBuilder {
    if let Some(timeout) = duration_from_ms(timeouts.connect_ms) {
        builder = builder.connect_timeout(timeout);
    }
    if let Some(timeout) = duration_from_ms(timeouts.overall_ms) {
        builder = builder.timeout(timeout);
    }
    if let Some(timeout) = duration_from_ms(timeouts.idle_ms) {
        builder = builder.read_timeout(timeout);
    }
    builder
}

fn apply_proxy(
    mut builder: reqwest::ClientBuilder,
    content: &RequestContent,
) -> Result<reqwest::ClientBuilder, HttpExecutionError> {
    let proxy = &content.transport.proxy;
    match proxy.source {
        ProxySource::Disabled => Ok(builder.no_proxy()),
        ProxySource::ProcessEnvironment => Ok(builder),
        ProxySource::Custom => {
            let Some(proxy_url) = proxy
                .url
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            else {
                return Ok(builder.no_proxy());
            };
            let parsed_proxy_url = Url::parse(proxy_url)
                .map_err(|_| HttpExecutionError::InvalidInput("proxy.url.invalid"))?;
            let mut reqwest_proxy = Proxy::all(proxy_url)
                .map_err(|_| HttpExecutionError::InvalidInput("proxy.url.invalid"))?;
            if !parsed_proxy_url.username().is_empty() {
                reqwest_proxy = reqwest_proxy.basic_auth(
                    parsed_proxy_url.username(),
                    parsed_proxy_url.password().unwrap_or_default(),
                );
            }
            if let Some(no_proxy) = no_proxy_from_entries(&proxy.no_proxy) {
                reqwest_proxy = reqwest_proxy.no_proxy(Some(no_proxy));
            }
            builder = builder.no_proxy().proxy(reqwest_proxy);
            Ok(builder)
        }
    }
}

fn resolve_reference_path(
    reference: &str,
    base_directory: Option<&str>,
) -> Result<PathBuf, HttpExecutionError> {
    let path = PathBuf::from(reference);
    if path.is_absolute() {
        return Ok(path);
    }
    if path_has_unsafe_components(reference) {
        return Err(HttpExecutionError::InvalidInput(
            "certificate.reference.invalid",
        ));
    }
    let Some(base_directory) = base_directory else {
        return Err(HttpExecutionError::InvalidInput(
            "workspace.baseDirectory.required",
        ));
    };
    Ok(PathBuf::from(base_directory).join(reference))
}

fn redirect_target(base_url: &Url, headers: &HeaderMap) -> Result<Option<Url>, HttpExecutionError> {
    let Some(location) = headers.get(LOCATION) else {
        return Ok(None);
    };
    let location = location
        .to_str()
        .map_err(|_| HttpExecutionError::InvalidInput("redirect.location.invalid"))?;
    base_url
        .join(location)
        .map(Some)
        .map_err(|_| HttpExecutionError::InvalidInput("redirect.location.invalid"))
}

fn redirect_method(method: &Method, status: u16) -> Method {
    match status {
        301 | 302 if *method == Method::POST => Method::GET,
        303 if *method != Method::HEAD => Method::GET,
        _ => method.clone(),
    }
}
