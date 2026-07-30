#[derive(Clone)]
struct PendingDraft {
    workspace_id: WorkspaceId,
    content: RequestContent,
}

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("workspace was not found")]
    WorkspaceNotFound,
    #[error("request item was not found")]
    NotFound,
    #[error("the saved request is already open in this workspace")]
    SavedRequestAlreadyOpen,
    #[error("request input is invalid: {0}")]
    InvalidInput(String),
    #[error("request persistence failed: {0}")]
    Persistence(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedRequestContent {
    pub url: ResolvedValue,
    pub body: ResolvedValue,
    pub body_kind: ResolvedRequestBody,
    pub query: Vec<ResolvedField>,
    pub headers: Vec<ResolvedField>,
    pub unsafe_tls_visible: bool,
    pub references: Vec<ResolvedVariableReference>,
    pub errors: Vec<VariableResolutionError>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum ResolvedRequestBody {
    None,
    Raw { content: ResolvedValue },
    UrlEncoded { fields: Vec<ResolvedField> },
    Multipart { parts: Vec<ResolvedMultipartPart> },
    Binary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum ResolvedMultipartPart {
    Field {
        enabled: bool,
        order: u32,
        name: ResolvedValue,
        value: ResolvedValue,
    },
    File {
        enabled: bool,
        order: u32,
        name: ResolvedValue,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedField {
    pub enabled: bool,
    pub order: u32,
    pub name: ResolvedValue,
    pub value: ResolvedValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedValue {
    pub value: String,
    pub contains_secret: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedVariableReference {
    pub name: String,
    pub source: VariableSource,
    pub value: ResolvedValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum VariableSource {
    Collection,
    Environment,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VariableResolutionError {
    pub name: String,
    pub kind: VariableResolutionErrorKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum VariableResolutionErrorKind {
    Missing,
    Cycle,
}

pub fn resolve_request_content(
    snapshot: &RequestWorkspaceSnapshot,
    content: &RequestContent,
) -> ResolvedRequestContent {
    resolve_request_content_with_secret_resolver(snapshot, content, &|_| None)
}

fn resolve_request_content_with_secret_resolver(
    snapshot: &RequestWorkspaceSnapshot,
    content: &RequestContent,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedRequestContent {
    let scope = VariableScope::from_snapshot(snapshot);
    let mut state = ResolutionState::default();
    let url = resolve_text(&content.url, &scope, &mut state, secret_resolver);
    let body_kind = resolve_request_body(&content.body, &scope, &mut state, secret_resolver);
    let body = resolved_body_preview(&body_kind);
    let mut query = content
        .query
        .iter()
        .map(|field| ResolvedField {
            enabled: field.enabled,
            order: field.order,
            name: resolve_text(&field.name, &scope, &mut state, secret_resolver),
            value: resolve_text(&field.value, &scope, &mut state, secret_resolver),
        })
        .collect();
    let mut headers = content
        .headers
        .iter()
        .map(|field| ResolvedField {
            enabled: field.enabled,
            order: field.order,
            name: resolve_text(&field.name, &scope, &mut state, secret_resolver),
            value: resolve_text(&field.value, &scope, &mut state, secret_resolver),
        })
        .collect();
    apply_resolved_auth(
        &content.auth,
        &scope,
        &mut state,
        &mut query,
        &mut headers,
        secret_resolver,
    );

    let mut references = state.references.into_values().collect::<Vec<_>>();
    references.sort_by(|left, right| left.name.cmp(&right.name));
    let mut errors = state.errors.into_values().collect::<Vec<_>>();
    errors.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
    });

    ResolvedRequestContent {
        url,
        body,
        body_kind,
        query,
        headers,
        unsafe_tls_visible: !content.tls.verify,
        references,
        errors,
    }
}

pub fn redact_request_content(
    content: RequestContent,
    resolved: &ResolvedRequestContent,
) -> RequestContent {
    RequestContent {
        name: content.name,
        method: content.method,
        url: redact_value_pair(content.url, &resolved.url),
        body: redact_request_body(content.body, &resolved.body_kind),
        query: content
            .query
            .into_iter()
            .zip(resolved.query.iter())
            .map(|(field, resolved)| OrderedField {
                enabled: field.enabled,
                order: field.order,
                name: redact_value_pair(field.name, &resolved.name),
                value: redact_value_pair(field.value, &resolved.value),
            })
            .collect(),
        headers: content
            .headers
            .into_iter()
            .zip(resolved.headers.iter())
            .map(|(field, resolved)| {
                let sensitive_header = is_sensitive_header(&field.name);
                OrderedField {
                    enabled: field.enabled,
                    order: field.order,
                    name: field.name,
                    value: if sensitive_header || resolved.value.contains_secret {
                        REDACTED_VALUE.to_owned()
                    } else {
                        field.value
                    },
                }
            })
            .collect(),
        auth: redact_request_auth(content.auth),
        redirect: content.redirect,
        tls: content.tls,
        transport: redact_transport_policy(content.transport),
    }
}

fn redact_transport_policy(
    mut policy: crate::domain::request::TransportPolicy,
) -> crate::domain::request::TransportPolicy {
    if let Some(url) = policy.proxy.url.as_deref() {
        policy.proxy.url = Some(redact_url_credentials(url));
    }
    policy
}

fn redact_url_credentials(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return REDACTED_VALUE.to_owned();
    };
    if !url.username().is_empty() {
        let _ = url.set_username(REDACTED_VALUE);
    }
    if url.password().is_some() {
        let _ = url.set_password(Some(REDACTED_VALUE));
    }
    url.to_string()
}

pub fn materialize_request_auth(
    snapshot: &RequestWorkspaceSnapshot,
    content: RequestContent,
) -> RequestContent {
    materialize_request_auth_with_secret_resolver(snapshot, content, &|_| None)
}

pub(crate) fn materialize_request_content_for_curl(
    snapshot: &RequestWorkspaceSnapshot,
    expected_environment_id: Option<EnvironmentId>,
    content: RequestContent,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> Result<RequestContent, RequestError> {
    use std::cell::Cell;

    let selected_environment_id = snapshot
        .environments
        .iter()
        .find(|environment| environment.is_selected)
        .map(|environment| environment.id);
    if selected_environment_id != expected_environment_id {
        return Err(RequestError::InvalidInput(
            "curl.resolution.stale".to_owned(),
        ));
    }

    let missing_secret = Cell::new(false);
    let tracked_secret_resolver = |reference: &str| {
        let value = secret_resolver(reference);
        if value.is_none() {
            missing_secret.set(true);
        }
        value
    };
    let mut materialized = content;
    let original_query_count = materialized.query.len();
    let original_header_count = materialized.headers.len();
    let resolved = resolve_request_content_with_secret_resolver(
        snapshot,
        &materialized,
        &tracked_secret_resolver,
    );
    materialized.url = resolved.url.value;
    materialized.body = materialize_request_body(materialized.body, &resolved.body_kind);
    materialized.query = resolved_fields_to_ordered(
        resolved
            .query
            .into_iter()
            .take(original_query_count)
            .collect(),
    );
    materialized.headers = resolved_fields_to_ordered(
        resolved
            .headers
            .into_iter()
            .take(original_header_count)
            .collect(),
    );
    let scope = VariableScope::from_snapshot(snapshot);
    let mut state = ResolutionState::default();
    materialized.auth = match materialized.auth {
        RequestAuth::None => RequestAuth::None,
        RequestAuth::Basic { username, password } => RequestAuth::Basic {
            username: resolve_text(
                &username,
                &scope,
                &mut state,
                &tracked_secret_resolver,
            )
            .value,
            password: resolve_text(
                &password,
                &scope,
                &mut state,
                &tracked_secret_resolver,
            )
            .value,
        },
        RequestAuth::Bearer { token } => RequestAuth::Bearer {
            token: resolve_text(&token, &scope, &mut state, &tracked_secret_resolver).value,
        },
        RequestAuth::ApiKey {
            placement,
            name,
            value,
        } => RequestAuth::ApiKey {
            placement,
            name: resolve_text(&name, &scope, &mut state, &tracked_secret_resolver).value,
            value: resolve_text(&value, &scope, &mut state, &tracked_secret_resolver).value,
        },
        RequestAuth::ClientCredentials {
            token_endpoint,
            client_id,
            client_secret,
            scopes,
        } => RequestAuth::ClientCredentials {
            token_endpoint: resolve_text(
                &token_endpoint,
                &scope,
                &mut state,
                &tracked_secret_resolver,
            )
            .value,
            client_id: resolve_text(
                &client_id,
                &scope,
                &mut state,
                &tracked_secret_resolver,
            )
            .value,
            client_secret: resolve_text(
                &client_secret,
                &scope,
                &mut state,
                &tracked_secret_resolver,
            )
            .value,
            scopes: scopes
                .into_iter()
                .map(|scope_value| {
                    resolve_text(
                        &scope_value,
                        &scope,
                        &mut state,
                        &tracked_secret_resolver,
                    )
                    .value
                })
                .collect(),
        },
    };
    if missing_secret.get() {
        return Err(RequestError::InvalidInput(
            "curl.secret.unavailable".to_owned(),
        ));
    }
    Ok(materialized)
}

fn materialize_request_auth_with_secret_resolver(
    snapshot: &RequestWorkspaceSnapshot,
    mut content: RequestContent,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> RequestContent {
    let auth = content.auth.clone();
    let resolved =
        resolve_request_content_with_secret_resolver(snapshot, &content, secret_resolver);
    content.url = resolved.url.value;
    content.body = materialize_request_body(content.body, &resolved.body_kind);
    content.query = resolved_fields_to_ordered(resolved.query);
    content.headers = resolved_fields_to_ordered(resolved.headers);
    content.auth = match auth {
        RequestAuth::ClientCredentials {
            token_endpoint,
            client_id,
            client_secret,
            scopes,
        } => {
            let scope = VariableScope::from_snapshot(snapshot);
            let mut state = ResolutionState::default();
            RequestAuth::ClientCredentials {
                token_endpoint: resolve_text(&token_endpoint, &scope, &mut state, secret_resolver)
                    .value,
                client_id: resolve_text(&client_id, &scope, &mut state, secret_resolver).value,
                client_secret: resolve_text(&client_secret, &scope, &mut state, secret_resolver)
                    .value,
                scopes: scopes
                    .into_iter()
                    .map(|scope_value| {
                        resolve_text(&scope_value, &scope, &mut state, secret_resolver).value
                    })
                    .collect(),
            }
        }
        _ => RequestAuth::None,
    };
    content
}

fn materialize_request_body(body: RequestBody, resolved: &ResolvedRequestBody) -> RequestBody {
    match (body, resolved) {
        (RequestBody::Raw { .. }, ResolvedRequestBody::Raw { content }) => RequestBody::Raw {
            content: content.value.clone(),
        },
        (
            RequestBody::UrlEncoded { fields },
            ResolvedRequestBody::UrlEncoded {
                fields: resolved_fields,
            },
        ) => RequestBody::UrlEncoded {
            fields: fields
                .into_iter()
                .zip(resolved_fields.iter())
                .map(|(field, resolved)| OrderedField {
                    enabled: field.enabled,
                    order: field.order,
                    name: resolved.name.value.clone(),
                    value: resolved.value.value.clone(),
                })
                .collect(),
        },
        (RequestBody::Multipart { parts }, ResolvedRequestBody::Multipart { parts: resolved }) => {
            RequestBody::Multipart {
                parts: parts
                    .into_iter()
                    .zip(resolved.iter())
                    .map(|(part, resolved)| match (part, resolved) {
                        (
                            MultipartPart::Field { enabled, order, .. },
                            ResolvedMultipartPart::Field { name, value, .. },
                        ) => MultipartPart::Field {
                            enabled,
                            order,
                            name: name.value.clone(),
                            value: value.value.clone(),
                        },
                        (
                            MultipartPart::File {
                                enabled,
                                order,
                                file,
                                ..
                            },
                            ResolvedMultipartPart::File { name, .. },
                        ) => MultipartPart::File {
                            enabled,
                            order,
                            name: name.value.clone(),
                            file,
                        },
                        (part, _) => part,
                    })
                    .collect(),
            }
        }
        (body, _) => body,
    }
}

fn resolved_fields_to_ordered(fields: Vec<ResolvedField>) -> Vec<OrderedField> {
    fields
        .into_iter()
        .map(|field| OrderedField {
            enabled: field.enabled,
            order: field.order,
            name: field.name.value,
            value: field.value.value,
        })
        .collect()
}

fn apply_resolved_auth(
    auth: &RequestAuth,
    scope: &VariableScope,
    state: &mut ResolutionState,
    query: &mut Vec<ResolvedField>,
    headers: &mut Vec<ResolvedField>,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) {
    match auth {
        RequestAuth::None => {}
        RequestAuth::Basic { username, password } => {
            let username = resolve_text(username, scope, state, secret_resolver);
            let password = resolve_text(password, scope, state, secret_resolver);
            let contains_secret = username.contains_secret || password.contains_secret;
            let value = if contains_secret {
                REDACTED_VALUE.to_owned()
            } else {
                format!(
                    "Basic {}",
                    BASE64_STANDARD.encode(format!("{}:{}", username.value, password.value))
                )
            };
            headers.push(resolved_auth_field(
                headers,
                "Authorization",
                ResolvedValue {
                    value,
                    contains_secret,
                },
            ));
        }
        RequestAuth::Bearer { token } => {
            let token = resolve_text(token, scope, state, secret_resolver);
            headers.push(resolved_auth_field(
                headers,
                "Authorization",
                ResolvedValue {
                    value: if token.contains_secret {
                        REDACTED_VALUE.to_owned()
                    } else {
                        format!("Bearer {}", token.value)
                    },
                    contains_secret: token.contains_secret,
                },
            ));
        }
        RequestAuth::ApiKey {
            placement,
            name,
            value,
        } => {
            let name = resolve_text(name, scope, state, secret_resolver);
            let value = resolve_text(value, scope, state, secret_resolver);
            let field = resolved_auth_field(
                match placement {
                    ApiKeyPlacement::Header => headers,
                    ApiKeyPlacement::Query => query,
                },
                &name.value,
                ResolvedValue {
                    value: if value.contains_secret {
                        REDACTED_VALUE.to_owned()
                    } else {
                        value.value
                    },
                    contains_secret: value.contains_secret,
                },
            );
            match placement {
                ApiKeyPlacement::Header => headers.push(field),
                ApiKeyPlacement::Query => query.push(field),
            }
        }
        RequestAuth::ClientCredentials {
            token_endpoint,
            client_id,
            client_secret,
            scopes,
        } => {
            let _ = resolve_text(token_endpoint, scope, state, secret_resolver);
            let _ = resolve_text(client_id, scope, state, secret_resolver);
            let _ = resolve_text(client_secret, scope, state, secret_resolver);
            for scope_value in scopes {
                let _ = resolve_text(scope_value, scope, state, secret_resolver);
            }
        }
    }
}

fn resolved_auth_field(
    existing: &[ResolvedField],
    name: &str,
    value: ResolvedValue,
) -> ResolvedField {
    ResolvedField {
        enabled: true,
        order: next_resolved_field_order(existing),
        name: ResolvedValue {
            value: name.to_owned(),
            contains_secret: false,
        },
        value,
    }
}

fn next_resolved_field_order(fields: &[ResolvedField]) -> u32 {
    fields
        .iter()
        .map(|field| field.order)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn redact_request_auth(auth: RequestAuth) -> RequestAuth {
    match auth {
        RequestAuth::None => RequestAuth::None,
        RequestAuth::Basic { username, .. } => RequestAuth::Basic {
            username,
            password: REDACTED_VALUE.to_owned(),
        },
        RequestAuth::Bearer { .. } => RequestAuth::Bearer {
            token: REDACTED_VALUE.to_owned(),
        },
        RequestAuth::ApiKey {
            placement, name, ..
        } => RequestAuth::ApiKey {
            placement,
            name,
            value: REDACTED_VALUE.to_owned(),
        },
        RequestAuth::ClientCredentials {
            token_endpoint,
            client_id,
            scopes,
            ..
        } => RequestAuth::ClientCredentials {
            token_endpoint,
            client_id,
            client_secret: REDACTED_VALUE.to_owned(),
            scopes,
        },
    }
}

fn resolve_request_body(
    body: &RequestBody,
    scope: &VariableScope,
    state: &mut ResolutionState,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedRequestBody {
    match body {
        RequestBody::None => ResolvedRequestBody::None,
        RequestBody::Raw { content } => ResolvedRequestBody::Raw {
            content: resolve_text(content, scope, state, secret_resolver),
        },
        RequestBody::UrlEncoded { fields } => ResolvedRequestBody::UrlEncoded {
            fields: fields
                .iter()
                .map(|field| ResolvedField {
                    enabled: field.enabled,
                    order: field.order,
                    name: resolve_text(&field.name, scope, state, secret_resolver),
                    value: resolve_text(&field.value, scope, state, secret_resolver),
                })
                .collect(),
        },
        RequestBody::Multipart { parts } => ResolvedRequestBody::Multipart {
            parts: parts
                .iter()
                .map(|part| match part {
                    MultipartPart::Field {
                        enabled,
                        order,
                        name,
                        value,
                    } => ResolvedMultipartPart::Field {
                        enabled: *enabled,
                        order: *order,
                        name: resolve_text(name, scope, state, secret_resolver),
                        value: resolve_text(value, scope, state, secret_resolver),
                    },
                    MultipartPart::File {
                        enabled,
                        order,
                        name,
                        ..
                    } => ResolvedMultipartPart::File {
                        enabled: *enabled,
                        order: *order,
                        name: resolve_text(name, scope, state, secret_resolver),
                    },
                })
                .collect(),
        },
        RequestBody::Binary { .. } => ResolvedRequestBody::Binary,
    }
}

fn resolved_body_preview(body: &ResolvedRequestBody) -> ResolvedValue {
    match body {
        ResolvedRequestBody::None => ResolvedValue {
            value: String::new(),
            contains_secret: false,
        },
        ResolvedRequestBody::Raw { content } => content.clone(),
        ResolvedRequestBody::UrlEncoded { fields } => ResolvedValue {
            value: fields
                .iter()
                .filter(|field| field.enabled)
                .map(|field| format!("{}={}", field.name.value, field.value.value))
                .collect::<Vec<_>>()
                .join("&"),
            contains_secret: fields
                .iter()
                .any(|field| field.name.contains_secret || field.value.contains_secret),
        },
        ResolvedRequestBody::Multipart { parts } => ResolvedValue {
            value: format!("{} multipart part(s)", parts.len()),
            contains_secret: parts.iter().any(|part| match part {
                ResolvedMultipartPart::Field { name, value, .. } => {
                    name.contains_secret || value.contains_secret
                }
                ResolvedMultipartPart::File { name, .. } => name.contains_secret,
            }),
        },
        ResolvedRequestBody::Binary => ResolvedValue {
            value: "binary file".to_owned(),
            contains_secret: false,
        },
    }
}

fn redact_request_body(body: RequestBody, resolved: &ResolvedRequestBody) -> RequestBody {
    match (body, resolved) {
        (RequestBody::Raw { content }, ResolvedRequestBody::Raw { content: resolved }) => {
            RequestBody::Raw {
                content: redact_value_pair(content, resolved),
            }
        }
        (
            RequestBody::UrlEncoded { fields },
            ResolvedRequestBody::UrlEncoded { fields: resolved },
        ) => RequestBody::UrlEncoded {
            fields: fields
                .into_iter()
                .zip(resolved.iter())
                .map(|(field, resolved)| OrderedField {
                    enabled: field.enabled,
                    order: field.order,
                    name: redact_value_pair(field.name, &resolved.name),
                    value: redact_value_pair(field.value, &resolved.value),
                })
                .collect(),
        },
        (RequestBody::Multipart { parts }, ResolvedRequestBody::Multipart { parts: resolved }) => {
            RequestBody::Multipart {
                parts: parts
                    .into_iter()
                    .zip(resolved.iter())
                    .map(|(part, resolved)| match (part, resolved) {
                        (
                            MultipartPart::Field {
                                enabled,
                                order,
                                name,
                                value,
                            },
                            ResolvedMultipartPart::Field {
                                name: resolved_name,
                                value: resolved_value,
                                ..
                            },
                        ) => MultipartPart::Field {
                            enabled,
                            order,
                            name: redact_value_pair(name, resolved_name),
                            value: redact_value_pair(value, resolved_value),
                        },
                        (part, _) => part,
                    })
                    .collect(),
            }
        }
        (body, _) => body,
    }
}

fn redact_response(response: ExecutionRecordResponse) -> ExecutionRecordResponse {
    ExecutionRecordResponse {
        headers: response
            .headers
            .into_iter()
            .map(|field| OrderedField {
                value: if is_sensitive_header(&field.name) {
                    REDACTED_VALUE.to_owned()
                } else {
                    field.value
                },
                ..field
            })
            .collect(),
        ..response
    }
}

fn redact_value_pair(original: String, resolved: &ResolvedValue) -> String {
    if resolved.contains_secret {
        REDACTED_VALUE.to_owned()
    } else {
        original
    }
}

fn is_sensitive_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("authorization")
        || name.eq_ignore_ascii_case("cookie")
        || name.eq_ignore_ascii_case("set-cookie")
}

fn has_enabled_cookie_header(headers: &[OrderedField]) -> bool {
    headers
        .iter()
        .any(|field| field.enabled && field.name.eq_ignore_ascii_case("cookie"))
}
