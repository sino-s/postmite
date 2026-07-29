fn resolved_request_content_from_dto(
    resolved: ResolvedRequestContentDto,
    content: &RequestContent,
) -> ResolvedRequestContent {
    let body_kind = match &content.body {
        RequestBody::None => ResolvedRequestBody::None,
        RequestBody::Raw { .. } => ResolvedRequestBody::Raw {
            content: ResolvedValue::from(resolved.body.clone()),
        },
        RequestBody::UrlEncoded { fields } => ResolvedRequestBody::UrlEncoded {
            fields: fields
                .iter()
                .map(|field| {
                    let contains_secret = resolved.body.contains_secret;
                    ResolvedField {
                        enabled: field.enabled,
                        order: field.order,
                        name: ResolvedValue {
                            value: field.name.clone(),
                            contains_secret: false,
                        },
                        value: ResolvedValue {
                            value: if contains_secret {
                                REDACTED_VALUE.to_owned()
                            } else {
                                field.value.clone()
                            },
                            contains_secret,
                        },
                    }
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
                        name: ResolvedValue {
                            value: name.clone(),
                            contains_secret: false,
                        },
                        value: ResolvedValue {
                            value: if resolved.body.contains_secret {
                                REDACTED_VALUE.to_owned()
                            } else {
                                value.clone()
                            },
                            contains_secret: resolved.body.contains_secret,
                        },
                    },
                    MultipartPart::File {
                        enabled,
                        order,
                        name,
                        ..
                    } => ResolvedMultipartPart::File {
                        enabled: *enabled,
                        order: *order,
                        name: ResolvedValue {
                            value: name.clone(),
                            contains_secret: false,
                        },
                    },
                })
                .collect(),
        },
        RequestBody::Binary { .. } => ResolvedRequestBody::Binary,
    };
    ResolvedRequestContent {
        url: ResolvedValue::from(resolved.url),
        body: ResolvedValue::from(resolved.body),
        body_kind,
        query: resolved
            .query
            .into_iter()
            .map(ResolvedField::from)
            .collect(),
        headers: resolved
            .headers
            .into_iter()
            .map(ResolvedField::from)
            .collect(),
        unsafe_tls_visible: resolved.unsafe_tls_visible,
        references: resolved
            .references
            .into_iter()
            .map(ResolvedVariableReference::from)
            .collect(),
        errors: resolved
            .errors
            .into_iter()
            .map(VariableResolutionError::from)
            .collect(),
    }
}

impl From<ExecutionHistorySnapshot> for ExecutionHistorySnapshotDto {
    fn from(snapshot: ExecutionHistorySnapshot) -> Self {
        Self {
            workspace_id: snapshot.workspace_id.to_string(),
            disabled: snapshot.disabled,
            records: snapshot
                .records
                .into_iter()
                .map(ExecutionRecordDto::from)
                .collect(),
            warning: snapshot.warning,
        }
    }
}

impl From<CookieJarSnapshot> for CookieJarSnapshotDto {
    fn from(snapshot: CookieJarSnapshot) -> Self {
        Self {
            workspace_id: snapshot.workspace_id.to_string(),
            cookies: snapshot
                .cookies
                .into_iter()
                .map(WorkspaceCookieDto::from)
                .collect(),
        }
    }
}

impl From<WorkspaceCookie> for WorkspaceCookieDto {
    fn from(cookie: WorkspaceCookie) -> Self {
        Self {
            id: cookie.id.to_string(),
            workspace_id: cookie.workspace_id.to_string(),
            name: cookie.name,
            domain: cookie.domain,
            path: cookie.path,
            secure: cookie.secure,
            http_only: cookie.http_only,
            same_site: cookie.same_site.map(CookieSameSiteDto::from),
            expires_at_epoch_seconds: cookie.expires_at_epoch_seconds,
            session: cookie.session,
            has_value: cookie.has_value,
            value_preview: REDACTED_VALUE.to_owned(),
        }
    }
}

impl From<CookieSameSite> for CookieSameSiteDto {
    fn from(value: CookieSameSite) -> Self {
        match value {
            CookieSameSite::Strict => Self::Strict,
            CookieSameSite::Lax => Self::Lax,
            CookieSameSite::None => Self::None,
        }
    }
}

impl From<CookieSameSiteDto> for CookieSameSite {
    fn from(value: CookieSameSiteDto) -> Self {
        match value {
            CookieSameSiteDto::Strict => Self::Strict,
            CookieSameSiteDto::Lax => Self::Lax,
            CookieSameSiteDto::None => Self::None,
        }
    }
}

impl TryFrom<UpsertCookieInput> for CookieDraft {
    type Error = IpcError;

    fn try_from(input: UpsertCookieInput) -> Result<Self, Self::Error> {
        Ok(Self {
            id: input
                .cookie_id
                .as_deref()
                .map(parse_cookie_id)
                .transpose()?,
            workspace_id: parse_workspace_id(&input.workspace_id)?,
            name: input.name,
            value: input.value,
            domain: input.domain,
            path: input.path,
            secure: input.secure,
            http_only: input.http_only,
            same_site: input.same_site.map(CookieSameSite::from),
            expires_at_epoch_seconds: input.expires_at_epoch_seconds,
        })
    }
}

impl From<ExecutionRecord> for ExecutionRecordDto {
    fn from(record: ExecutionRecord) -> Self {
        Self {
            id: record.id.to_string(),
            workspace_id: record.workspace_id.to_string(),
            created_at_epoch_seconds: record.created_at_epoch_seconds,
            request: RequestContentDto::from(record.request),
            response: ExecutionRecordResponseDto::from(record.response),
            pinned: record.pinned,
        }
    }
}

impl From<ExecutionRecordResponse> for ExecutionRecordResponseDto {
    fn from(response: ExecutionRecordResponse) -> Self {
        Self {
            status: response.status,
            headers: response
                .headers
                .into_iter()
                .map(OrderedFieldDto::from)
                .collect(),
            body_preview: response.body_preview,
            body_truncated: response.body_truncated,
            error: response.error,
            duration_ms: response.duration_ms,
        }
    }
}

impl From<SavedRequest> for SavedRequestDto {
    fn from(request: SavedRequest) -> Self {
        Self {
            id: request.id.to_string(),
            workspace_id: request.workspace_id.to_string(),
            collection_id: request.collection_id.map(|id| id.to_string()),
            position: request.position,
            content: RequestContentDto::from(request.content),
        }
    }
}

impl From<CollectionFolder> for CollectionFolderDto {
    fn from(folder: CollectionFolder) -> Self {
        Self {
            id: folder.id.to_string(),
            workspace_id: folder.workspace_id.to_string(),
            parent_collection_id: folder.parent_collection_id.map(|id| id.to_string()),
            name: folder.name,
            position: folder.position,
        }
    }
}

impl From<Environment> for EnvironmentDto {
    fn from(environment: Environment) -> Self {
        Self {
            id: environment.id.to_string(),
            workspace_id: environment.workspace_id.to_string(),
            name: environment.name,
            position: environment.position,
            is_selected: environment.is_selected,
        }
    }
}

impl From<CollectionVariable> for CollectionVariableDto {
    fn from(variable: CollectionVariable) -> Self {
        Self {
            workspace_id: variable.workspace_id.to_string(),
            variable: VariableDto::from(variable.variable),
        }
    }
}

impl From<EnvironmentVariable> for EnvironmentVariableDto {
    fn from(variable: EnvironmentVariable) -> Self {
        Self {
            environment_id: variable.environment_id.to_string(),
            workspace_id: variable.workspace_id.to_string(),
            variable: VariableDto::from(variable.variable),
        }
    }
}

impl From<Variable> for VariableDto {
    fn from(variable: Variable) -> Self {
        Self {
            name: variable.name,
            value: VariableValueDto::from(variable.value),
        }
    }
}

impl From<VariableValue> for VariableValueDto {
    fn from(value: VariableValue) -> Self {
        match value {
            VariableValue::Plain(value) => Self::Plain { value },
            VariableValue::SecretReference(reference) => Self::SecretReference { reference },
        }
    }
}

impl TryFrom<CollectionLocationDto> for CollectionLocation {
    type Error = IpcError;

    fn try_from(location: CollectionLocationDto) -> Result<Self, Self::Error> {
        Ok(Self {
            collection_id: parse_optional_collection_id(location.collection_id)?,
            position: location.position,
        })
    }
}

impl From<ResolvedRequestContent> for ResolvedRequestContentDto {
    fn from(content: ResolvedRequestContent) -> Self {
        Self {
            url: ResolvedValueDto::from(content.url),
            body: ResolvedValueDto::from(content.body),
            query: content
                .query
                .into_iter()
                .map(ResolvedFieldDto::from)
                .collect(),
            headers: content
                .headers
                .into_iter()
                .map(ResolvedFieldDto::from)
                .collect(),
            unsafe_tls_visible: content.unsafe_tls_visible,
            references: content
                .references
                .into_iter()
                .map(ResolvedVariableReferenceDto::from)
                .collect(),
            errors: content
                .errors
                .into_iter()
                .map(VariableResolutionErrorDto::from)
                .collect(),
        }
    }
}

impl From<ResolvedField> for ResolvedFieldDto {
    fn from(field: ResolvedField) -> Self {
        Self {
            enabled: field.enabled,
            order: field.order,
            name: ResolvedValueDto::from(field.name),
            value: ResolvedValueDto::from(field.value),
        }
    }
}

impl From<ResolvedFieldDto> for ResolvedField {
    fn from(field: ResolvedFieldDto) -> Self {
        Self {
            enabled: field.enabled,
            order: field.order,
            name: ResolvedValue::from(field.name),
            value: ResolvedValue::from(field.value),
        }
    }
}

impl From<ResolvedValue> for ResolvedValueDto {
    fn from(value: ResolvedValue) -> Self {
        Self {
            value: value.value,
            contains_secret: value.contains_secret,
        }
    }
}

impl From<ResolvedValueDto> for ResolvedValue {
    fn from(value: ResolvedValueDto) -> Self {
        Self {
            value: value.value,
            contains_secret: value.contains_secret,
        }
    }
}

impl From<ResolvedVariableReference> for ResolvedVariableReferenceDto {
    fn from(reference: ResolvedVariableReference) -> Self {
        Self {
            name: reference.name,
            source: VariableSourceDto::from(reference.source),
            value: ResolvedValueDto::from(reference.value),
        }
    }
}

impl From<ResolvedVariableReferenceDto> for ResolvedVariableReference {
    fn from(reference: ResolvedVariableReferenceDto) -> Self {
        Self {
            name: reference.name,
            source: VariableSource::from(reference.source),
            value: ResolvedValue::from(reference.value),
        }
    }
}

impl From<VariableSource> for VariableSourceDto {
    fn from(source: VariableSource) -> Self {
        match source {
            VariableSource::Collection => Self::Collection,
            VariableSource::Environment => Self::Environment,
        }
    }
}

impl From<VariableSourceDto> for VariableSource {
    fn from(source: VariableSourceDto) -> Self {
        match source {
            VariableSourceDto::Collection => Self::Collection,
            VariableSourceDto::Environment => Self::Environment,
        }
    }
}

impl From<VariableResolutionError> for VariableResolutionErrorDto {
    fn from(error: VariableResolutionError) -> Self {
        Self {
            name: error.name,
            kind: VariableResolutionErrorKindDto::from(error.kind),
        }
    }
}

impl From<VariableResolutionErrorDto> for VariableResolutionError {
    fn from(error: VariableResolutionErrorDto) -> Self {
        Self {
            name: error.name,
            kind: VariableResolutionErrorKind::from(error.kind),
        }
    }
}

impl From<VariableResolutionErrorKind> for VariableResolutionErrorKindDto {
    fn from(kind: VariableResolutionErrorKind) -> Self {
        match kind {
            VariableResolutionErrorKind::Missing => Self::Missing,
            VariableResolutionErrorKind::Cycle => Self::Cycle,
        }
    }
}

impl From<VariableResolutionErrorKindDto> for VariableResolutionErrorKind {
    fn from(kind: VariableResolutionErrorKindDto) -> Self {
        match kind {
            VariableResolutionErrorKindDto::Missing => Self::Missing,
            VariableResolutionErrorKindDto::Cycle => Self::Cycle,
        }
    }
}

impl From<RequestDraft> for RequestDraftDto {
    fn from(draft: RequestDraft) -> Self {
        Self {
            id: draft.id.to_string(),
            workspace_id: draft.workspace_id.to_string(),
            saved_request_id: draft.saved_request_id.map(|id| id.to_string()),
            content: RequestContentDto::from(draft.content),
            is_dirty: draft.is_dirty,
        }
    }
}

impl From<RequestTab> for RequestTabDto {
    fn from(tab: RequestTab) -> Self {
        Self {
            id: tab.id.to_string(),
            workspace_id: tab.workspace_id.to_string(),
            saved_request_id: tab.saved_request_id.map(|id| id.to_string()),
            draft_id: tab.draft_id.to_string(),
            position: tab.position,
            title: tab.title,
            is_active: tab.is_active,
        }
    }
}

impl From<RequestContent> for RequestContentDto {
    fn from(content: RequestContent) -> Self {
        Self {
            name: content.name,
            method: content.method,
            url: content.url,
            body: RequestBodyDto::from(content.body),
            query: content
                .query
                .into_iter()
                .map(OrderedFieldDto::from)
                .collect(),
            headers: content
                .headers
                .into_iter()
                .map(OrderedFieldDto::from)
                .collect(),
            auth: RequestAuthDto::from(content.auth),
            redirect: RedirectPolicyDto::from(content.redirect),
            tls: TlsPolicyDto::from(content.tls),
            transport: TransportPolicyDto::from(content.transport),
        }
    }
}

impl From<RequestContentDto> for RequestContent {
    fn from(content: RequestContentDto) -> Self {
        Self {
            name: content.name,
            method: content.method,
            url: content.url,
            body: RequestBody::from(content.body),
            query: content.query.into_iter().map(OrderedField::from).collect(),
            headers: content
                .headers
                .into_iter()
                .map(OrderedField::from)
                .collect(),
            auth: RequestAuth::from(content.auth),
            redirect: RedirectPolicy::from(content.redirect),
            tls: TlsPolicy::from(content.tls),
            transport: TransportPolicy::from(content.transport),
        }
    }
}

impl From<RequestAuth> for RequestAuthDto {
    fn from(auth: RequestAuth) -> Self {
        match auth {
            RequestAuth::None => Self::None,
            RequestAuth::Basic { username, password } => Self::Basic { username, password },
            RequestAuth::Bearer { token } => Self::Bearer { token },
            RequestAuth::ApiKey {
                placement,
                name,
                value,
            } => Self::ApiKey {
                placement: ApiKeyPlacementDto::from(placement),
                name,
                value,
            },
            RequestAuth::ClientCredentials {
                token_endpoint,
                client_id,
                client_secret,
                scopes,
            } => Self::ClientCredentials {
                token_endpoint,
                client_id,
                client_secret,
                scopes,
            },
        }
    }
}

impl From<RequestAuthDto> for RequestAuth {
    fn from(auth: RequestAuthDto) -> Self {
        match auth {
            RequestAuthDto::None => Self::None,
            RequestAuthDto::Basic { username, password } => Self::Basic { username, password },
            RequestAuthDto::Bearer { token } => Self::Bearer { token },
            RequestAuthDto::ApiKey {
                placement,
                name,
                value,
            } => Self::ApiKey {
                placement: ApiKeyPlacement::from(placement),
                name,
                value,
            },
            RequestAuthDto::ClientCredentials {
                token_endpoint,
                client_id,
                client_secret,
                scopes,
            } => Self::ClientCredentials {
                token_endpoint,
                client_id,
                client_secret,
                scopes,
            },
        }
    }
}

impl From<ApiKeyPlacement> for ApiKeyPlacementDto {
    fn from(placement: ApiKeyPlacement) -> Self {
        match placement {
            ApiKeyPlacement::Header => Self::Header,
            ApiKeyPlacement::Query => Self::Query,
        }
    }
}

impl From<ApiKeyPlacementDto> for ApiKeyPlacement {
    fn from(placement: ApiKeyPlacementDto) -> Self {
        match placement {
            ApiKeyPlacementDto::Header => Self::Header,
            ApiKeyPlacementDto::Query => Self::Query,
        }
    }
}

impl From<RedirectPolicy> for RedirectPolicyDto {
    fn from(policy: RedirectPolicy) -> Self {
        Self {
            enabled: policy.enabled,
            max_redirects: policy.max_redirects,
        }
    }
}

impl From<RedirectPolicyDto> for RedirectPolicy {
    fn from(policy: RedirectPolicyDto) -> Self {
        Self {
            enabled: policy.enabled,
            max_redirects: policy.max_redirects,
        }
    }
}

impl From<TlsPolicy> for TlsPolicyDto {
    fn from(policy: TlsPolicy) -> Self {
        Self {
            verify: policy.verify,
            custom_ca_reference: policy.custom_ca_reference,
            client_certificate_reference: policy.client_certificate_reference,
            client_key_reference: policy.client_key_reference,
        }
    }
}

impl From<TlsPolicyDto> for TlsPolicy {
    fn from(policy: TlsPolicyDto) -> Self {
        Self {
            verify: policy.verify,
            custom_ca_reference: policy.custom_ca_reference,
            client_certificate_reference: policy.client_certificate_reference,
            client_key_reference: policy.client_key_reference,
        }
    }
}

impl From<TransportPolicy> for TransportPolicyDto {
    fn from(policy: TransportPolicy) -> Self {
        Self {
            proxy: ProxyPolicyDto::from(policy.proxy),
            timeouts: TimeoutPolicyDto::from(policy.timeouts),
        }
    }
}

impl From<TransportPolicyDto> for TransportPolicy {
    fn from(policy: TransportPolicyDto) -> Self {
        Self {
            proxy: ProxyPolicy::from(policy.proxy),
            timeouts: TimeoutPolicy::from(policy.timeouts),
        }
    }
}

impl From<ProxyPolicy> for ProxyPolicyDto {
    fn from(policy: ProxyPolicy) -> Self {
        Self {
            source: ProxySourceDto::from(policy.source),
            url: policy.url,
            no_proxy: policy.no_proxy,
        }
    }
}

impl From<ProxyPolicyDto> for ProxyPolicy {
    fn from(policy: ProxyPolicyDto) -> Self {
        Self {
            source: ProxySource::from(policy.source),
            url: policy.url,
            no_proxy: policy.no_proxy,
        }
    }
}

impl From<ProxySource> for ProxySourceDto {
    fn from(source: ProxySource) -> Self {
        match source {
            ProxySource::Disabled => Self::Disabled,
            ProxySource::ProcessEnvironment => Self::ProcessEnvironment,
            ProxySource::Custom => Self::Custom,
        }
    }
}

impl From<ProxySourceDto> for ProxySource {
    fn from(source: ProxySourceDto) -> Self {
        match source {
            ProxySourceDto::Disabled => Self::Disabled,
            ProxySourceDto::ProcessEnvironment => Self::ProcessEnvironment,
            ProxySourceDto::Custom => Self::Custom,
        }
    }
}

impl From<TimeoutPolicy> for TimeoutPolicyDto {
    fn from(policy: TimeoutPolicy) -> Self {
        Self {
            connect_ms: policy.connect_ms,
            overall_ms: policy.overall_ms,
            idle_ms: policy.idle_ms,
        }
    }
}

impl From<TimeoutPolicyDto> for TimeoutPolicy {
    fn from(policy: TimeoutPolicyDto) -> Self {
        Self {
            connect_ms: policy.connect_ms,
            overall_ms: policy.overall_ms,
            idle_ms: policy.idle_ms,
        }
    }
}

impl From<RequestBody> for RequestBodyDto {
    fn from(body: RequestBody) -> Self {
        match body {
            RequestBody::None => Self::None,
            RequestBody::Raw { content } => Self::Raw { content },
            RequestBody::UrlEncoded { fields } => Self::UrlEncoded {
                fields: fields.into_iter().map(OrderedFieldDto::from).collect(),
            },
            RequestBody::Multipart { parts } => Self::Multipart {
                parts: parts.into_iter().map(MultipartPartDto::from).collect(),
            },
            RequestBody::Binary { file } => Self::Binary {
                file: BodyFileReferenceDto::from(file),
            },
        }
    }
}

impl From<RequestBodyDto> for RequestBody {
    fn from(body: RequestBodyDto) -> Self {
        match body {
            RequestBodyDto::None => Self::None,
            RequestBodyDto::Raw { content } => Self::Raw { content },
            RequestBodyDto::UrlEncoded { fields } => Self::UrlEncoded {
                fields: fields.into_iter().map(OrderedField::from).collect(),
            },
            RequestBodyDto::Multipart { parts } => Self::Multipart {
                parts: parts.into_iter().map(MultipartPart::from).collect(),
            },
            RequestBodyDto::Binary { file } => Self::Binary {
                file: BodyFileReference::from(file),
            },
        }
    }
}

impl From<MultipartPart> for MultipartPartDto {
    fn from(part: MultipartPart) -> Self {
        match part {
            MultipartPart::Field {
                enabled,
                order,
                name,
                value,
            } => Self::Field {
                enabled,
                order,
                name,
                value,
            },
            MultipartPart::File {
                enabled,
                order,
                name,
                file,
            } => Self::File {
                enabled,
                order,
                name,
                file: BodyFileReferenceDto::from(file),
            },
        }
    }
}

impl From<MultipartPartDto> for MultipartPart {
    fn from(part: MultipartPartDto) -> Self {
        match part {
            MultipartPartDto::Field {
                enabled,
                order,
                name,
                value,
            } => Self::Field {
                enabled,
                order,
                name,
                value,
            },
            MultipartPartDto::File {
                enabled,
                order,
                name,
                file,
            } => Self::File {
                enabled,
                order,
                name,
                file: BodyFileReference::from(file),
            },
        }
    }
}

impl From<BodyFileReference> for BodyFileReferenceDto {
    fn from(file: BodyFileReference) -> Self {
        Self {
            path: BodyFilePathDto::from(file.path),
            file_name: file.file_name,
            size: file.size,
            modified_at_epoch_seconds: file.modified_at_epoch_seconds,
            sha256: file.sha256,
        }
    }
}

impl From<BodyFileReferenceDto> for BodyFileReference {
    fn from(file: BodyFileReferenceDto) -> Self {
        Self {
            path: BodyFilePath::from(file.path),
            file_name: file.file_name,
            size: file.size,
            modified_at_epoch_seconds: file.modified_at_epoch_seconds,
            sha256: file.sha256,
        }
    }
}

impl From<BodyFilePath> for BodyFilePathDto {
    fn from(path: BodyFilePath) -> Self {
        match path {
            BodyFilePath::Relative { path } => Self::Relative { path },
            BodyFilePath::Absolute { path } => Self::Absolute { path },
        }
    }
}

impl From<BodyFilePathDto> for BodyFilePath {
    fn from(path: BodyFilePathDto) -> Self {
        match path {
            BodyFilePathDto::Relative { path } => Self::Relative { path },
            BodyFilePathDto::Absolute { path } => Self::Absolute { path },
        }
    }
}

impl From<OrderedField> for OrderedFieldDto {
    fn from(field: OrderedField) -> Self {
        Self {
            enabled: field.enabled,
            order: field.order,
            name: field.name,
            value: field.value,
        }
    }
}

impl From<OrderedFieldDto> for OrderedField {
    fn from(field: OrderedFieldDto) -> Self {
        Self {
            enabled: field.enabled,
            order: field.order,
            name: field.name,
            value: field.value,
        }
    }
}

impl From<CloseTabDecisionDto> for CloseTabDecision {
    fn from(decision: CloseTabDecisionDto) -> Self {
        match decision {
            CloseTabDecisionDto::Save => Self::Save,
            CloseTabDecisionDto::Discard => Self::Discard,
            CloseTabDecisionDto::Cancel => Self::Cancel,
        }
    }
}

impl From<StartExecutionResult> for StartRequestExecutionOutput {
    fn from(result: StartExecutionResult) -> Self {
        Self {
            execution_id: result.execution_id.to_string(),
        }
    }
}

impl From<CancelExecutionResult> for CancelRequestExecutionOutput {
    fn from(result: CancelExecutionResult) -> Self {
        Self {
            execution_id: result.execution_id.to_string(),
            cancelled: result.cancelled,
        }
    }
}

impl TryFrom<StartOAuthAuthorizationInput> for StartOAuthAuthorizationRequest {
    type Error = IpcError;

    fn try_from(input: StartOAuthAuthorizationInput) -> Result<Self, Self::Error> {
        Ok(Self {
            flow_id: parse_oauth_flow_id(&input.flow_id)?,
            authorization_endpoint: input.authorization_endpoint,
            client_id: input.client_id,
            scopes: input.scopes,
            redirect_path: input.redirect_path,
            timeout_ms: input.timeout_ms,
        })
    }
}

impl From<OAuthAuthorizationResult> for OAuthAuthorizationResultDto {
    fn from(result: OAuthAuthorizationResult) -> Self {
        Self {
            flow_id: result.flow_id.to_string(),
            redirect_uri: result.redirect_uri,
            code: result.code,
            state: result.state,
            error: result.error,
            error_description: result.error_description,
        }
    }
}

impl From<CancelOAuthAuthorizationResult> for CancelOAuthAuthorizationOutput {
    fn from(result: CancelOAuthAuthorizationResult) -> Self {
        Self {
            flow_id: result.flow_id.to_string(),
            cancelled: result.cancelled,
        }
    }
}

impl From<ExecutionEvent> for ExecutionEventDto {
    fn from(event: ExecutionEvent) -> Self {
        Self {
            execution_id: event.execution_id.to_string(),
            sequence: event.sequence,
            kind: ExecutionEventKindDto::from(event.kind),
        }
    }
}

impl From<ExecutionEventKind> for ExecutionEventKindDto {
    fn from(kind: ExecutionEventKind) -> Self {
        match kind {
            ExecutionEventKind::Started {
                method,
                url,
                tls_verification,
                proxy,
                timeouts,
                queued_ms,
            } => Self::Started {
                method,
                url,
                tls_verification,
                proxy: ExecutionProxyMetadataDto::from(proxy),
                timeouts: ExecutionTimeoutMetadataDto::from(timeouts),
                queued_ms,
            },
            ExecutionEventKind::Redirected { from, to, status } => {
                Self::Redirected { from, to, status }
            }
            ExecutionEventKind::UploadProgress {
                sent_bytes,
                total_bytes,
            } => Self::UploadProgress {
                sent_bytes,
                total_bytes,
            },
            ExecutionEventKind::ResponseHeaders {
                status,
                headers,
                protocol,
                remote_addr,
            } => Self::ResponseHeaders {
                status,
                headers: headers.into_iter().map(ExecutionHeaderDto::from).collect(),
                protocol,
                remote_addr,
            },
            ExecutionEventKind::DownloadProgress {
                received_bytes,
                total_bytes,
            } => Self::DownloadProgress {
                received_bytes,
                total_bytes,
            },
            ExecutionEventKind::Completed {
                status,
                body_preview,
                body_truncated,
                decoded_bytes,
                wire_bytes,
                response_file,
                timing,
            } => Self::Completed {
                status,
                body_preview,
                body_truncated,
                decoded_bytes,
                wire_bytes,
                response_file: response_file.map(ResponseFileMetadataDto::from),
                timing: ExecutionTimingMetadataDto::from(timing),
            },
            ExecutionEventKind::Failed { message } => Self::Failed { message },
            ExecutionEventKind::Cancelled => Self::Cancelled,
        }
    }
}

impl From<ResponseFileMetadata> for ResponseFileMetadataDto {
    fn from(metadata: ResponseFileMetadata) -> Self {
        Self {
            path: metadata.path,
            byte_count: metadata.byte_count,
            expires_at_epoch_seconds: metadata.expires_at_epoch_seconds,
        }
    }
}

impl From<ExecutionProxyMetadata> for ExecutionProxyMetadataDto {
    fn from(metadata: ExecutionProxyMetadata) -> Self {
        Self {
            source: metadata.source,
            selected_proxy: metadata.selected_proxy,
            bypass_reason: metadata.bypass_reason,
        }
    }
}

impl From<ExecutionTimeoutMetadata> for ExecutionTimeoutMetadataDto {
    fn from(metadata: ExecutionTimeoutMetadata) -> Self {
        Self {
            connect_ms: metadata.connect_ms,
            overall_ms: metadata.overall_ms,
            idle_ms: metadata.idle_ms,
        }
    }
}

impl From<ExecutionTimingMetadata> for ExecutionTimingMetadataDto {
    fn from(metadata: ExecutionTimingMetadata) -> Self {
        Self {
            queued_ms: metadata.queued_ms,
            dns_ms: metadata.dns_ms,
            connect_ms: metadata.connect_ms,
            tls_ms: metadata.tls_ms,
            first_byte_ms: metadata.first_byte_ms,
            download_ms: metadata.download_ms,
            total_ms: metadata.total_ms,
        }
    }
}

impl From<ExecutionHeader> for ExecutionHeaderDto {
    fn from(header: ExecutionHeader) -> Self {
        let value = if header.name.eq_ignore_ascii_case("set-cookie") {
            REDACTED_VALUE.to_owned()
        } else {
            header.value
        };
        Self {
            name: header.name,
            value,
        }
    }
}
