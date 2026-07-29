fn validate_request_content(content: &RequestContent) -> Result<(), RequestError> {
    if content.name.trim().is_empty() {
        return Err(RequestError::InvalidInput("name.required".to_owned()));
    }
    if content.method.trim().is_empty() {
        return Err(RequestError::InvalidInput("method.required".to_owned()));
    }
    Ok(())
}

fn request_body_to_sql(body: &RequestBody) -> Result<String, RequestError> {
    serde_json::to_string(body).map_err(RequestError::persistence)
}

fn request_body_from_sql(value: String) -> Result<RequestBody, serde_json::Error> {
    if value.trim_start().starts_with('{') {
        serde_json::from_str(&value)
    } else if value.is_empty() {
        Ok(RequestBody::None)
    } else {
        Ok(RequestBody::Raw { content: value })
    }
}

fn request_auth_to_sql(auth: &RequestAuth) -> Result<String, RequestError> {
    serde_json::to_string(auth).map_err(RequestError::persistence)
}

fn request_auth_from_sql(value: String) -> Result<RequestAuth, serde_json::Error> {
    if value.trim().is_empty() {
        Ok(RequestAuth::default())
    } else {
        serde_json::from_str(&value)
    }
}

fn redirect_policy_to_sql(policy: &RedirectPolicy) -> Result<String, RequestError> {
    serde_json::to_string(policy).map_err(RequestError::persistence)
}

fn redirect_policy_from_sql(value: String) -> Result<RedirectPolicy, serde_json::Error> {
    if value.trim().is_empty() {
        Ok(RedirectPolicy::default())
    } else {
        serde_json::from_str(&value)
    }
}

fn tls_policy_to_sql(policy: &TlsPolicy) -> Result<String, RequestError> {
    serde_json::to_string(policy).map_err(RequestError::persistence)
}

fn tls_policy_from_sql(value: String) -> Result<TlsPolicy, serde_json::Error> {
    if value.trim().is_empty() {
        Ok(TlsPolicy::default())
    } else {
        serde_json::from_str(&value)
    }
}

fn transport_policy_to_sql(policy: &TransportPolicy) -> Result<String, RequestError> {
    serde_json::to_string(policy).map_err(RequestError::persistence)
}

fn transport_policy_from_sql(value: String) -> Result<TransportPolicy, serde_json::Error> {
    if value.trim().is_empty() {
        Ok(TransportPolicy::default())
    } else {
        serde_json::from_str(&value)
    }
}

fn replace_body_file_reference(
    body: &mut RequestBody,
    from_path: &str,
    replacement: &BodyFileReference,
) -> bool {
    match body {
        RequestBody::Binary { file } if body_file_path_matches(file, from_path) => {
            *file = replacement.clone();
            true
        }
        RequestBody::Multipart { parts } => {
            let mut changed = false;
            for part in parts {
                if let MultipartPart::File { file, .. } = part {
                    if body_file_path_matches(file, from_path) {
                        *file = replacement.clone();
                        changed = true;
                    }
                }
            }
            changed
        }
        _ => false,
    }
}

fn body_file_path_matches(file: &BodyFileReference, from_path: &str) -> bool {
    match &file.path {
        BodyFilePath::Relative { path } | BodyFilePath::Absolute { path } => path == from_path,
    }
}

fn validate_collection_name(name: &str) -> Result<(), RequestError> {
    if name.trim().is_empty() {
        return Err(RequestError::InvalidInput(
            "collection.name.required".to_owned(),
        ));
    }
    Ok(())
}

fn validate_cookie_metadata(draft: &CookieDraft) -> Result<(), RequestError> {
    if draft.name.trim().is_empty() {
        return Err(RequestError::InvalidInput(
            "cookie.name.required".to_owned(),
        ));
    }
    if draft.domain.trim().is_empty() {
        return Err(RequestError::InvalidInput(
            "cookie.domain.required".to_owned(),
        ));
    }
    if !draft.path.starts_with('/') {
        return Err(RequestError::InvalidInput("cookie.path.invalid".to_owned()));
    }
    Ok(())
}

fn draft_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestDraft> {
    Ok(RequestDraft {
        id: request_draft_id_from_row(row)?,
        workspace_id: workspace_id_from_row_index(row, 1)?,
        saved_request_id: optional_saved_request_id_from_row(row, 2)?,
        content: RequestContent {
            name: row.get(3)?,
            method: row.get(4)?,
            url: row.get(5)?,
            body: request_body_from_sql(row.get(6)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            auth: request_auth_from_sql(row.get(7)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            redirect: redirect_policy_from_sql(row.get(8)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            tls: tls_policy_from_sql(row.get(9)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            transport: transport_policy_from_sql(row.get(10)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    10,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            query: Vec::new(),
            headers: Vec::new(),
        },
        is_dirty: row.get::<_, i64>(11)? != 0,
    })
}

fn workspace_cookie_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceCookie> {
    let same_site: Option<String> = row.get(7)?;
    Ok(WorkspaceCookie {
        id: cookie_id_from_row(row)?,
        workspace_id: workspace_id_from_row_index(row, 1)?,
        name: row.get(2)?,
        domain: row.get(3)?,
        path: row.get(4)?,
        secure: row.get::<_, i64>(5)? != 0,
        http_only: row.get::<_, i64>(6)? != 0,
        same_site: same_site
            .as_deref()
            .map(cookie_same_site_from_sql)
            .transpose()?,
        expires_at_epoch_seconds: row.get(8)?,
        session: row.get::<_, i64>(9)? != 0,
        has_value: row.get::<_, i64>(10)? != 0,
        secret_reference: row.get(11)?,
    })
}

fn execution_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionRecord> {
    let status: Option<i64> = row.get(12)?;
    let duration_ms: Option<i64> = row.get(16)?;
    Ok(ExecutionRecord {
        id: execution_record_id_from_row(row)?,
        workspace_id: workspace_id_from_row_index(row, 1)?,
        created_at_epoch_seconds: row.get(2)?,
        pinned: row.get::<_, i64>(3)? != 0,
        request: RequestContent {
            name: row.get(4)?,
            method: row.get(5)?,
            url: row.get(6)?,
            body: request_body_from_sql(row.get(7)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            auth: request_auth_from_sql(row.get(8)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            redirect: redirect_policy_from_sql(row.get(9)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            tls: tls_policy_from_sql(row.get(10)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    10,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            transport: transport_policy_from_sql(row.get(11)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            query: Vec::new(),
            headers: Vec::new(),
        },
        response: ExecutionRecordResponse {
            status: status
                .map(|value| {
                    u16::try_from(value).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            12,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })
                })
                .transpose()?,
            headers: Vec::new(),
            body_preview: row.get(13)?,
            body_truncated: row.get::<_, i64>(14)? != 0,
            error: row.get(15)?,
            duration_ms: duration_ms
                .map(|value| {
                    u64::try_from(value).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            16,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })
                })
                .transpose()?,
        },
    })
}

fn collection_folder_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CollectionFolder> {
    let position: i64 = row.get(4)?;
    Ok(CollectionFolder {
        id: collection_id_from_row(row)?,
        workspace_id: workspace_id_from_row_index(row, 1)?,
        parent_collection_id: optional_collection_id_from_row(row, 2)?,
        name: row.get(3)?,
        position: u32::try_from(position).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })?,
    })
}

fn tab_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestTab> {
    let position: i64 = row.get(4)?;
    Ok(RequestTab {
        id: request_tab_id_from_row(row)?,
        workspace_id: workspace_id_from_row_index(row, 1)?,
        saved_request_id: optional_saved_request_id_from_row(row, 2)?,
        draft_id: request_draft_id_from_row_index(row, 3)?,
        position: u32::try_from(position).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })?,
        title: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
    })
}

fn workspace_id_from_row_index(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<WorkspaceId> {
    let id: String = row.get(index)?;
    WorkspaceId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn saved_request_id_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedRequestId> {
    let id: String = row.get(0)?;
    SavedRequestId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn collection_id_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CollectionId> {
    let id: String = row.get(0)?;
    CollectionId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn environment_id_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EnvironmentId> {
    environment_id_from_row_index(row, 0)
}

fn environment_id_from_row_index(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<EnvironmentId> {
    let id: String = row.get(index)?;
    EnvironmentId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn variable_from_row(
    row: &rusqlite::Row<'_>,
    name_index: usize,
    plain_value_index: usize,
    secret_ref_index: usize,
) -> rusqlite::Result<Variable> {
    let name: String = row.get(name_index)?;
    let plain_value: Option<String> = row.get(plain_value_index)?;
    let secret_ref: Option<String> = row.get(secret_ref_index)?;
    let value = match (plain_value, secret_ref) {
        (Some(value), None) => VariableValue::Plain(value),
        (None, Some(reference)) => VariableValue::SecretReference(reference),
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                plain_value_index,
                rusqlite::types::Type::Text,
                "variable value must contain exactly one value kind".into(),
            ));
        }
    };
    Ok(Variable { name, value })
}

fn request_draft_id_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestDraftId> {
    request_draft_id_from_row_index(row, 0)
}

fn request_draft_id_from_row_index(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<RequestDraftId> {
    let id: String = row.get(index)?;
    RequestDraftId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn request_tab_id_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestTabId> {
    let id: String = row.get(0)?;
    RequestTabId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn execution_record_id_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionRecordId> {
    let id: String = row.get(0)?;
    ExecutionRecordId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn cookie_id_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CookieId> {
    let id: String = row.get(0)?;
    CookieId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn optional_saved_request_id_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<SavedRequestId>> {
    let id: Option<String> = row.get(index)?;
    id.map(|id| {
        SavedRequestId::from_str(&id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    })
    .transpose()
}

fn optional_collection_id_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<CollectionId>> {
    let id: Option<String> = row.get(index)?;
    id.map(|id| {
        CollectionId::from_str(&id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    })
    .transpose()
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn normalize_cookie_domain(domain: &str) -> String {
    domain.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn cookie_same_site_to_sql(value: CookieSameSite) -> &'static str {
    match value {
        CookieSameSite::Strict => "strict",
        CookieSameSite::Lax => "lax",
        CookieSameSite::None => "none",
    }
}

fn cookie_same_site_from_sql(value: &str) -> rusqlite::Result<CookieSameSite> {
    match value {
        "strict" => Ok(CookieSameSite::Strict),
        "lax" => Ok(CookieSameSite::Lax),
        "none" => Ok(CookieSameSite::None),
        _ => Err(rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Text,
            "invalid cookie SameSite value".into(),
        )),
    }
}

fn map_request_sqlite_error(error: rusqlite::Error) -> RequestError {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == ErrorCode::ConstraintViolation =>
        {
            RequestError::InvalidInput("constraint".to_owned())
        }
        _ => RequestError::persistence(error),
    }
}

fn map_postman_sqlite_error(error: rusqlite::Error) -> PostmanImportError {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == ErrorCode::ConstraintViolation =>
        {
            PostmanImportError::InvalidInput("postman.import.constraint".to_owned())
        }
        _ => PostmanImportError::persistence(error),
    }
}

fn postman_request_error(error: RequestError) -> PostmanImportError {
    match error {
        RequestError::WorkspaceNotFound => PostmanImportError::WorkspaceNotFound,
        RequestError::InvalidInput(detail) => PostmanImportError::InvalidInput(detail),
        RequestError::Persistence(error) => PostmanImportError::Persistence(error.to_string()),
        RequestError::NotFound | RequestError::SavedRequestAlreadyOpen => {
            PostmanImportError::InvalidInput("postman.import.invalidReference".to_owned())
        }
    }
}

fn native_backup_request_error(error: RequestError) -> NativeBackupError {
    match error {
        RequestError::WorkspaceNotFound => NativeBackupError::WorkspaceNotFound,
        RequestError::InvalidInput(detail) => NativeBackupError::InvalidInput(detail),
        RequestError::Persistence(error) => NativeBackupError::Persistence(error.to_string()),
        RequestError::NotFound | RequestError::SavedRequestAlreadyOpen => {
            NativeBackupError::InvalidArchive("backup.reference.invalid".to_owned())
        }
    }
}

fn native_backup_workspace_error(error: WorkspaceError) -> NativeBackupError {
    match error {
        WorkspaceError::NotFound => NativeBackupError::WorkspaceNotFound,
        WorkspaceError::AlreadyExists => NativeBackupError::WorkspaceAlreadyExists,
        WorkspaceError::InvalidName(error) => {
            NativeBackupError::InvalidInput(format!("workspace.name.{error}"))
        }
        WorkspaceError::CannotDeleteLastWorkspace => {
            NativeBackupError::InvalidInput("workspace.cannotDeleteLast".to_owned())
        }
        WorkspaceError::Persistence(error) => NativeBackupError::Persistence(error.to_string()),
    }
}

fn native_backup_sqlite_error(error: rusqlite::Error) -> NativeBackupError {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == ErrorCode::ConstraintViolation =>
        {
            NativeBackupError::InvalidInput("backup.restore.constraint".to_owned())
        }
        _ => NativeBackupError::persistence(error),
    }
}

fn postman_import_record_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoredPostmanImportRecord> {
    let collection_ids_json: String = row.get(4)?;
    let environment_ids_json: String = row.get(5)?;
    Ok(StoredPostmanImportRecord {
        id: row.get(0)?,
        source_id: row.get(1)?,
        source_name: row.get(2)?,
        source_hash: row.get(3)?,
        collection_ids: collection_ids_from_json(&collection_ids_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        environment_ids: environment_ids_from_json(&environment_ids_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
    })
}

fn entity_ids_to_json<T: ToString>(ids: &[T]) -> Result<String, PostmanImportError> {
    serde_json::to_string(&ids.iter().map(ToString::to_string).collect::<Vec<String>>())
        .map_err(|error| PostmanImportError::Persistence(error.to_string()))
}

fn collection_ids_from_json(json: &str) -> Result<Vec<CollectionId>, uuid::Error> {
    let ids: Vec<String> = serde_json::from_str(json).unwrap_or_default();
    ids.into_iter()
        .map(|id| CollectionId::from_str(&id))
        .collect()
}

fn environment_ids_from_json(json: &str) -> Result<Vec<EnvironmentId>, uuid::Error> {
    let ids: Vec<String> = serde_json::from_str(json).unwrap_or_default();
    ids.into_iter()
        .map(|id| EnvironmentId::from_str(&id))
        .collect()
}
