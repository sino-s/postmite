fn insert_converted_postman_import(
    tx: &Transaction<'_>,
    import: &ConvertedPostmanImport,
) -> Result<(), PostmanImportError> {
    let mut collection_ids: Vec<Option<CollectionId>> = vec![None; import.collections.len()];
    let mut root_collection_ids = Vec::new();
    for collection in &import.collections {
        let collection_id = CollectionId::new();
        let parent_id = collection
            .parent_import_index
            .and_then(|index| collection_ids.get(index).copied().flatten());
        ensure_import_parent_present(collection, parent_id)?;
        let position = next_collection_position(tx, import.workspace_id, parent_id)
            .map_err(postman_request_error)?;
        validate_collection_name(&collection.name).map_err(postman_request_error)?;
        tx.execute(
            "INSERT INTO collections
                (id, workspace_id, parent_collection_id, name, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                collection_id.to_string(),
                import.workspace_id.to_string(),
                parent_id.map(|id| id.to_string()),
                collection.name.trim(),
                position,
            ],
        )
        .map_err(map_postman_sqlite_error)?;
        if collection.parent_import_index.is_none() {
            root_collection_ids.push(collection_id);
        }
        if let Some(slot) = collection_ids.get_mut(collection.import_index) {
            *slot = Some(collection_id);
        }
    }

    for request in &import.requests {
        let collection_id = request
            .collection_import_index
            .and_then(|index| collection_ids.get(index).copied().flatten());
        insert_saved_request(
            tx,
            import.workspace_id,
            SavedRequestId::new(),
            collection_id,
            &request.content,
        )
        .map_err(postman_request_error)?;
    }

    let mut environment_ids = Vec::new();
    for environment in &import.environments {
        validate_collection_name(&environment.name).map_err(postman_request_error)?;
        let environment_id = EnvironmentId::new();
        let position = next_environment_position(tx, import.workspace_id)?;
        tx.execute(
            "INSERT INTO environments (id, workspace_id, name, position)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                environment_id.to_string(),
                import.workspace_id.to_string(),
                environment.name.trim(),
                position,
            ],
        )
        .map_err(map_postman_sqlite_error)?;
        environment_ids.push(environment_id);
        for variable in &environment.variables {
            insert_environment_variable(
                tx,
                import.workspace_id,
                environment_id,
                &variable.name,
                &variable.value,
            )?;
        }
    }

    tx.execute(
        "INSERT INTO postman_import_records
            (id, workspace_id, source_id, source_name, source_hash,
             collection_json_sha256, environment_json_sha256,
             warning_count, unsupported_count, warnings_json, unsupported_json,
             collection_ids_json, environment_ids_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            uuid::Uuid::new_v4().to_string(),
            import.workspace_id.to_string(),
            import.source_id,
            import.source_name,
            import.source_hash,
            import.collection_json_sha256,
            import.environment_json_sha256,
            import.warnings.len() as i64,
            import.unsupported.len() as i64,
            serde_json::to_string(&import.warnings)
                .map_err(|error| PostmanImportError::Persistence(error.to_string()))?,
            serde_json::to_string(&import.unsupported)
                .map_err(|error| PostmanImportError::Persistence(error.to_string()))?,
            entity_ids_to_json(&root_collection_ids)?,
            entity_ids_to_json(&environment_ids)?,
        ],
    )
    .map_err(map_postman_sqlite_error)?;
    Ok(())
}

#[derive(Default)]
struct BackupIdMap {
    collections: HashMap<CollectionId, CollectionId>,
    environments: HashMap<EnvironmentId, EnvironmentId>,
    saved_requests: HashMap<SavedRequestId, SavedRequestId>,
    drafts: HashMap<RequestDraftId, RequestDraftId>,
}

fn load_backup_workspace(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<NativeBackupWorkspace, NativeBackupError> {
    connection
        .query_row(
            "SELECT id, name, base_directory FROM workspaces WHERE id = ?1",
            params![workspace_id.to_string()],
            |row| {
                Ok(NativeBackupWorkspace {
                    id: workspace_id_from_row(row)?,
                    name: row.get(1)?,
                    base_directory: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(NativeBackupError::persistence)?
        .ok_or(NativeBackupError::WorkspaceNotFound)
}

fn restore_backup_requests(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    snapshot: RequestWorkspaceSnapshot,
) -> Result<BackupIdMap, NativeBackupError> {
    let mut map = BackupIdMap::default();
    for folder in &snapshot.collection_folders {
        map.collections.insert(folder.id, CollectionId::new());
    }
    for environment in &snapshot.environments {
        map.environments
            .insert(environment.id, EnvironmentId::new());
    }
    for request in &snapshot.saved_requests {
        map.saved_requests.insert(request.id, SavedRequestId::new());
    }
    for draft in &snapshot.drafts {
        map.drafts.insert(draft.id, RequestDraftId::new());
    }

    for folder in snapshot.collection_folders {
        let new_id = map.collections[&folder.id];
        let parent_id = folder
            .parent_collection_id
            .map(|id| {
                map.collections
                    .get(&id)
                    .copied()
                    .ok_or_else(invalid_backup_reference)
            })
            .transpose()?;
        insert_backup_collection(tx, workspace_id, new_id, parent_id, &folder)?;
    }
    for environment in &snapshot.environments {
        let new_id = map.environments[&environment.id];
        insert_backup_environment(tx, workspace_id, new_id, environment)?;
    }
    for variable in snapshot.collection_variables {
        insert_backup_collection_variable(tx, workspace_id, variable)?;
    }
    for variable in snapshot.environment_variables {
        let environment_id = map
            .environments
            .get(&variable.environment_id)
            .copied()
            .ok_or_else(invalid_backup_reference)?;
        insert_backup_environment_variable(tx, workspace_id, environment_id, variable)?;
    }
    for environment in &snapshot.environments {
        if environment.is_selected {
            let environment_id = map.environments[&environment.id];
            tx.execute(
                "INSERT INTO selected_environments (workspace_id, environment_id)
                 VALUES (?1, ?2)
                 ON CONFLICT(workspace_id) DO UPDATE SET
                    environment_id = excluded.environment_id,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![workspace_id.to_string(), environment_id.to_string()],
            )
            .map_err(native_backup_sqlite_error)?;
        }
    }
    for request in snapshot.saved_requests {
        let new_id = map.saved_requests[&request.id];
        let collection_id = request
            .collection_id
            .map(|id| {
                map.collections
                    .get(&id)
                    .copied()
                    .ok_or_else(invalid_backup_reference)
            })
            .transpose()?;
        insert_saved_request_at(
            tx,
            workspace_id,
            new_id,
            collection_id,
            request.position,
            &request.content,
        )
        .map_err(native_backup_request_error)?;
    }
    for draft in snapshot.drafts {
        let new_id = map.drafts[&draft.id];
        let saved_request_id = draft
            .saved_request_id
            .map(|id| {
                map.saved_requests
                    .get(&id)
                    .copied()
                    .ok_or_else(invalid_backup_reference)
            })
            .transpose()?;
        insert_draft(
            tx,
            workspace_id,
            new_id,
            saved_request_id,
            &draft.content,
            draft.is_dirty,
        )
        .map_err(native_backup_request_error)?;
    }
    for tab in snapshot.tabs {
        let draft_id = map
            .drafts
            .get(&tab.draft_id)
            .copied()
            .ok_or_else(invalid_backup_reference)?;
        let saved_request_id = tab
            .saved_request_id
            .map(|id| {
                map.saved_requests
                    .get(&id)
                    .copied()
                    .ok_or_else(invalid_backup_reference)
            })
            .transpose()?;
        insert_backup_tab(tx, workspace_id, saved_request_id, draft_id, tab)?;
    }
    Ok(map)
}

fn restore_backup_execution_history(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    snapshot: ExecutionHistorySnapshot,
) -> Result<(), NativeBackupError> {
    tx.execute(
        "INSERT INTO execution_history_settings (workspace_id, disabled)
         VALUES (?1, ?2)
         ON CONFLICT(workspace_id) DO UPDATE SET
            disabled = excluded.disabled,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        params![workspace_id.to_string(), bool_to_i64(snapshot.disabled)],
    )
    .map_err(native_backup_sqlite_error)?;
    for record in snapshot.records {
        insert_backup_execution_record(tx, workspace_id, record)?;
    }
    Ok(())
}

fn restore_backup_cookies(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    cookies: Vec<WorkspaceCookie>,
) -> Result<(), NativeBackupError> {
    for cookie in cookies {
        tx.execute(
            "INSERT INTO workspace_cookies
                (id, workspace_id, name, domain, path, secure, http_only, same_site,
                 expires_at_epoch_seconds, session, has_value, secret_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, NULL)",
            params![
                CookieId::new().to_string(),
                workspace_id.to_string(),
                cookie.name,
                cookie.domain,
                cookie.path,
                bool_to_i64(cookie.secure),
                bool_to_i64(cookie.http_only),
                cookie.same_site.map(cookie_same_site_to_sql),
                cookie.expires_at_epoch_seconds,
                bool_to_i64(cookie.session),
            ],
        )
        .map_err(native_backup_sqlite_error)?;
    }
    Ok(())
}

fn insert_backup_collection(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    collection_id: CollectionId,
    parent_collection_id: Option<CollectionId>,
    folder: &CollectionFolder,
) -> Result<(), NativeBackupError> {
    validate_collection_name(&folder.name).map_err(native_backup_request_error)?;
    tx.execute(
        "INSERT INTO collections (id, workspace_id, parent_collection_id, name, position)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            collection_id.to_string(),
            workspace_id.to_string(),
            parent_collection_id.map(|id| id.to_string()),
            folder.name.trim(),
            i64::from(folder.position),
        ],
    )
    .map_err(native_backup_sqlite_error)?;
    Ok(())
}

fn insert_backup_environment(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    environment_id: EnvironmentId,
    environment: &Environment,
) -> Result<(), NativeBackupError> {
    validate_collection_name(&environment.name).map_err(native_backup_request_error)?;
    tx.execute(
        "INSERT INTO environments (id, workspace_id, name, position)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            environment_id.to_string(),
            workspace_id.to_string(),
            environment.name.trim(),
            i64::from(environment.position),
        ],
    )
    .map_err(native_backup_sqlite_error)?;
    Ok(())
}

fn insert_backup_collection_variable(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    variable: CollectionVariable,
) -> Result<(), NativeBackupError> {
    let (plain_value, secret_ref) = variable_value_columns(&variable.variable.value);
    tx.execute(
        "INSERT INTO collection_variables (workspace_id, name, plain_value, secret_ref)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            workspace_id.to_string(),
            variable.variable.name,
            plain_value,
            secret_ref,
        ],
    )
    .map_err(native_backup_sqlite_error)?;
    Ok(())
}

fn insert_backup_environment_variable(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    environment_id: EnvironmentId,
    variable: EnvironmentVariable,
) -> Result<(), NativeBackupError> {
    let (plain_value, secret_ref) = variable_value_columns(&variable.variable.value);
    let position: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0)
             FROM environment_variables
             WHERE workspace_id = ?1 AND environment_id = ?2",
            params![workspace_id.to_string(), environment_id.to_string()],
            |row| row.get(0),
        )
        .map_err(native_backup_sqlite_error)?;
    tx.execute(
        "INSERT INTO environment_variables
            (environment_id, workspace_id, name, plain_value, secret_ref, position)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            environment_id.to_string(),
            workspace_id.to_string(),
            variable.variable.name,
            plain_value,
            secret_ref,
            position,
        ],
    )
    .map_err(native_backup_sqlite_error)?;
    Ok(())
}

fn variable_value_columns(value: &VariableValue) -> (Option<&str>, Option<&str>) {
    match value {
        VariableValue::Plain(value) => (Some(value.as_str()), None),
        VariableValue::SecretReference(reference) => (None, Some(reference.as_str())),
    }
}

fn insert_backup_tab(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    saved_request_id: Option<SavedRequestId>,
    draft_id: RequestDraftId,
    tab: RequestTab,
) -> Result<(), NativeBackupError> {
    tx.execute(
        "INSERT INTO request_tabs
            (id, workspace_id, saved_request_id, draft_id, position, title, is_active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            RequestTabId::new().to_string(),
            workspace_id.to_string(),
            saved_request_id.map(|id| id.to_string()),
            draft_id.to_string(),
            i64::from(tab.position),
            tab.title,
            bool_to_i64(tab.is_active),
        ],
    )
    .map_err(native_backup_sqlite_error)?;
    Ok(())
}

fn insert_backup_execution_record(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    record: ExecutionRecord,
) -> Result<(), NativeBackupError> {
    validate_request_content(&record.request).map_err(native_backup_request_error)?;
    let record_id = ExecutionRecordId::new();
    tx.execute(
        "INSERT INTO execution_records
            (id, workspace_id, created_at_epoch_seconds, pinned, name, method, url, body,
             auth, redirect_policy, tls_policy, transport_policy, response_status,
             response_body_preview, response_body_truncated, response_error, response_duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            record_id.to_string(),
            workspace_id.to_string(),
            record.created_at_epoch_seconds,
            bool_to_i64(record.pinned),
            record.request.name.as_str(),
            record.request.method.as_str(),
            record.request.url.as_str(),
            request_body_to_sql(&record.request.body).map_err(native_backup_request_error)?,
            request_auth_to_sql(&record.request.auth).map_err(native_backup_request_error)?,
            redirect_policy_to_sql(&record.request.redirect)
                .map_err(native_backup_request_error)?,
            tls_policy_to_sql(&record.request.tls).map_err(native_backup_request_error)?,
            transport_policy_to_sql(&record.request.transport)
                .map_err(native_backup_request_error)?,
            record.response.status.map(i64::from),
            record.response.body_preview.as_str(),
            bool_to_i64(record.response.body_truncated),
            record.response.error.as_deref(),
            record.response.duration_ms.map(|value| value as i64),
        ],
    )
    .map_err(native_backup_sqlite_error)?;
    replace_fields(
        tx,
        "execution_record_query_rows",
        "execution_record_id",
        &record_id.to_string(),
        &record.request.query,
    )
    .map_err(native_backup_request_error)?;
    replace_fields(
        tx,
        "execution_record_header_rows",
        "execution_record_id",
        &record_id.to_string(),
        &record.request.headers,
    )
    .map_err(native_backup_request_error)?;
    replace_fields(
        tx,
        "execution_record_response_header_rows",
        "execution_record_id",
        &record_id.to_string(),
        &record.response.headers,
    )
    .map_err(native_backup_request_error)?;
    Ok(())
}

fn invalid_backup_reference() -> NativeBackupError {
    NativeBackupError::InvalidArchive("backup.reference.invalid".to_owned())
}
