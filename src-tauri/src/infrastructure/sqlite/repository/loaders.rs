fn current_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn execution_history_disabled(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<bool, RequestError> {
    let disabled = connection
        .query_row(
            "SELECT disabled FROM execution_history_settings WHERE workspace_id = ?1",
            params![workspace_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(RequestError::persistence)?
        .unwrap_or(0);
    Ok(disabled != 0)
}

fn load_execution_history_snapshot(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<ExecutionHistorySnapshot, RequestError> {
    Ok(ExecutionHistorySnapshot {
        workspace_id,
        disabled: execution_history_disabled(connection, workspace_id)?,
        records: load_execution_records(connection, workspace_id)?,
        warning: ExecutionHistorySnapshot::warning_text(),
    })
}

fn load_execution_records(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<ExecutionRecord>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, created_at_epoch_seconds, pinned, name, method, url, body,
                    auth, redirect_policy, tls_policy, transport_policy, response_status,
                    response_body_preview, response_body_truncated,
                    response_error, response_duration_ms
             FROM execution_records
             WHERE workspace_id = ?1
             ORDER BY pinned DESC, created_at_epoch_seconds DESC, id DESC",
        )
        .map_err(RequestError::persistence)?;
    let rows = statement
        .query_map(params![workspace_id.to_string()], execution_record_from_row)
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;

    rows.into_iter()
        .map(|mut record| {
            record.request.query = load_fields(
                connection,
                "execution_record_query_rows",
                "execution_record_id",
                &record.id.to_string(),
            )?;
            record.request.headers = load_fields(
                connection,
                "execution_record_header_rows",
                "execution_record_id",
                &record.id.to_string(),
            )?;
            record.response.headers = load_fields(
                connection,
                "execution_record_response_header_rows",
                "execution_record_id",
                &record.id.to_string(),
            )?;
            Ok(record)
        })
        .collect()
}

fn load_execution_record(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: ExecutionRecordId,
) -> Result<ExecutionRecord, RequestError> {
    load_execution_records(connection, workspace_id)?
        .into_iter()
        .find(|record| record.id == record_id)
        .ok_or(RequestError::NotFound)
}

fn load_request_snapshot(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<RequestWorkspaceSnapshot, RequestError> {
    Ok(RequestWorkspaceSnapshot {
        workspace_id,
        collection_folders: load_collection_folders(connection, workspace_id)?,
        environments: load_environments(connection, workspace_id)?,
        collection_variables: load_collection_variables(connection, workspace_id)?,
        environment_variables: load_environment_variables(connection, workspace_id)?,
        saved_requests: load_saved_requests(connection, workspace_id)?,
        drafts: load_open_drafts(connection, workspace_id)?,
        tabs: load_tabs(connection, workspace_id)?,
    })
}

fn load_workspace_cookies(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<WorkspaceCookie>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, name, domain, path, secure, http_only, same_site,
                    expires_at_epoch_seconds, session, has_value, secret_ref
             FROM workspace_cookies
             WHERE workspace_id = ?1
             ORDER BY domain, path, name",
        )
        .map_err(RequestError::persistence)?;
    let cookies = statement
        .query_map(params![workspace_id.to_string()], workspace_cookie_from_row)
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(cookies)
}

fn load_workspace_cookie_by_scope(
    connection: &Connection,
    workspace_id: WorkspaceId,
    name: &str,
    domain: &str,
    path: &str,
) -> Result<WorkspaceCookie, RequestError> {
    connection
        .query_row(
            "SELECT id, workspace_id, name, domain, path, secure, http_only, same_site,
                    expires_at_epoch_seconds, session, has_value, secret_ref
             FROM workspace_cookies
             WHERE workspace_id = ?1 AND name = ?2 AND domain = ?3 AND path = ?4",
            params![workspace_id.to_string(), name, domain, path],
            workspace_cookie_from_row,
        )
        .optional()
        .map_err(RequestError::persistence)?
        .ok_or(RequestError::NotFound)
}

fn load_expired_cookie_ids(
    connection: &Connection,
    workspace_id: WorkspaceId,
    now_epoch_seconds: i64,
) -> Result<Vec<CookieId>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT id
             FROM workspace_cookies
             WHERE workspace_id = ?1
               AND expires_at_epoch_seconds IS NOT NULL
               AND expires_at_epoch_seconds <= ?2",
        )
        .map_err(RequestError::persistence)?;
    let ids = statement
        .query_map(
            params![workspace_id.to_string(), now_epoch_seconds],
            cookie_id_from_row,
        )
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(ids)
}

fn load_collection_folders(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<CollectionFolder>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, parent_collection_id, name, position
             FROM collections
             WHERE workspace_id = ?1
             ORDER BY parent_collection_id, position, created_at, id",
        )
        .map_err(RequestError::persistence)?;
    let folders = statement
        .query_map(
            params![workspace_id.to_string()],
            collection_folder_from_row,
        )
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(folders)
}

fn load_collection_folder(
    connection: &Connection,
    workspace_id: WorkspaceId,
    collection_id: CollectionId,
) -> Result<CollectionFolder, RequestError> {
    connection
        .query_row(
            "SELECT id, workspace_id, parent_collection_id, name, position
             FROM collections
             WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), collection_id.to_string()],
            collection_folder_from_row,
        )
        .optional()
        .map_err(RequestError::persistence)?
        .ok_or(RequestError::NotFound)
}

fn load_environments(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<Environment>, RequestError> {
    let selected_id: Option<String> = connection
        .query_row(
            "SELECT environment_id FROM selected_environments WHERE workspace_id = ?1",
            params![workspace_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(RequestError::persistence)?
        .flatten();
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, name, position
             FROM environments
             WHERE workspace_id = ?1
             ORDER BY position, created_at, id",
        )
        .map_err(RequestError::persistence)?;
    let environments = statement
        .query_map(params![workspace_id.to_string()], |row| {
            let id = environment_id_from_row(row)?;
            let position: i64 = row.get(3)?;
            Ok(Environment {
                id,
                workspace_id: workspace_id_from_row_index(row, 1)?,
                name: row.get(2)?,
                position: u32::try_from(position).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                is_selected: selected_id.as_deref() == Some(&id.to_string()),
            })
        })
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(environments)
}

fn load_collection_variables(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<CollectionVariable>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT workspace_id, name, plain_value, secret_ref
             FROM collection_variables
             WHERE workspace_id = ?1
             ORDER BY name",
        )
        .map_err(RequestError::persistence)?;
    let variables = statement
        .query_map(params![workspace_id.to_string()], |row| {
            Ok(CollectionVariable {
                workspace_id: workspace_id_from_row_index(row, 0)?,
                variable: variable_from_row(row, 1, 2, 3)?,
            })
        })
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(variables)
}

fn load_environment_variables(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<EnvironmentVariable>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT environment_id, workspace_id, name, plain_value, secret_ref
             FROM environment_variables
             WHERE workspace_id = ?1
             ORDER BY environment_id, name",
        )
        .map_err(RequestError::persistence)?;
    let variables = statement
        .query_map(params![workspace_id.to_string()], |row| {
            Ok(EnvironmentVariable {
                environment_id: environment_id_from_row(row)?,
                workspace_id: workspace_id_from_row_index(row, 1)?,
                variable: variable_from_row(row, 2, 3, 4)?,
            })
        })
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(variables)
}

fn load_child_collections(
    connection: &Connection,
    workspace_id: WorkspaceId,
    parent_collection_id: CollectionId,
) -> Result<Vec<CollectionFolder>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, parent_collection_id, name, position
             FROM collections
             WHERE workspace_id = ?1 AND parent_collection_id = ?2
             ORDER BY position, created_at, id",
        )
        .map_err(RequestError::persistence)?;
    let folders = statement
        .query_map(
            params![workspace_id.to_string(), parent_collection_id.to_string()],
            collection_folder_from_row,
        )
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(folders)
}

fn load_saved_requests(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<SavedRequest>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, collection_id, name, method, url, body,
                    auth, redirect_policy, tls_policy, transport_policy, position
             FROM saved_requests
             WHERE workspace_id = ?1
             ORDER BY collection_id, position, created_at, id",
        )
        .map_err(RequestError::persistence)?;
    let rows = statement
        .query_map(params![workspace_id.to_string()], |row| {
            let id = saved_request_id_from_row(row)?;
            let row_workspace_id = workspace_id_from_row_index(row, 1)?;
            let collection_id = optional_collection_id_from_row(row, 2)?;
            let content = RequestContent {
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
            };
            Ok(SavedRequest {
                id,
                workspace_id: row_workspace_id,
                collection_id,
                position: u32::try_from(row.get::<_, i64>(11)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        11,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                content,
            })
        })
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;

    rows.into_iter()
        .map(|mut request| {
            request.content.query = load_fields(
                connection,
                "saved_request_query_rows",
                "saved_request_id",
                &request.id.to_string(),
            )?;
            request.content.headers = load_fields(
                connection,
                "saved_request_header_rows",
                "saved_request_id",
                &request.id.to_string(),
            )?;
            Ok(request)
        })
        .collect()
}

fn load_saved_requests_in_collection(
    connection: &Connection,
    workspace_id: WorkspaceId,
    collection_id: Option<CollectionId>,
) -> Result<Vec<SavedRequest>, RequestError> {
    Ok(load_saved_requests(connection, workspace_id)?
        .into_iter()
        .filter(|request| request.collection_id == collection_id)
        .collect())
}

fn load_saved_request(
    connection: &Connection,
    workspace_id: WorkspaceId,
    saved_request_id: SavedRequestId,
) -> Result<SavedRequest, RequestError> {
    load_saved_requests(connection, workspace_id)?
        .into_iter()
        .find(|request| request.id == saved_request_id)
        .ok_or(RequestError::NotFound)
}

fn load_open_drafts(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<RequestDraft>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT d.id, d.workspace_id, d.saved_request_id, d.name, d.method, d.url,
                    d.body, d.auth, d.redirect_policy, d.tls_policy, d.transport_policy, d.is_dirty
             FROM request_drafts d
             INNER JOIN request_tabs t ON t.draft_id = d.id
             WHERE d.workspace_id = ?1
             ORDER BY t.position, d.created_at, d.id",
        )
        .map_err(RequestError::persistence)?;
    let rows = statement
        .query_map(params![workspace_id.to_string()], draft_from_row)
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;

    rows.into_iter()
        .map(|mut draft| {
            draft.content.query = load_fields(
                connection,
                "request_draft_query_rows",
                "draft_id",
                &draft.id.to_string(),
            )?;
            draft.content.headers = load_fields(
                connection,
                "request_draft_header_rows",
                "draft_id",
                &draft.id.to_string(),
            )?;
            Ok(draft)
        })
        .collect()
}

fn load_draft(
    connection: &Connection,
    workspace_id: WorkspaceId,
    draft_id: RequestDraftId,
) -> Result<RequestDraft, RequestError> {
    let mut draft = connection
        .query_row(
            "SELECT id, workspace_id, saved_request_id, name, method, url, body,
                    auth, redirect_policy, tls_policy, transport_policy, is_dirty
             FROM request_drafts
             WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), draft_id.to_string()],
            draft_from_row,
        )
        .optional()
        .map_err(RequestError::persistence)?
        .ok_or(RequestError::NotFound)?;
    draft.content.query = load_fields(
        connection,
        "request_draft_query_rows",
        "draft_id",
        &draft.id.to_string(),
    )?;
    draft.content.headers = load_fields(
        connection,
        "request_draft_header_rows",
        "draft_id",
        &draft.id.to_string(),
    )?;
    Ok(draft)
}

fn load_tabs(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<RequestTab>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, saved_request_id, draft_id, position, title, is_active
             FROM request_tabs
             WHERE workspace_id = ?1
             ORDER BY position, created_at, id",
        )
        .map_err(RequestError::persistence)?;

    let tabs = statement
        .query_map(params![workspace_id.to_string()], tab_from_row)
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(tabs)
}

fn load_fields(
    connection: &Connection,
    table: &str,
    owner_column: &str,
    owner_id: &str,
) -> Result<Vec<OrderedField>, RequestError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT row_order, enabled, name, value
             FROM {table}
             WHERE {owner_column} = ?1
             ORDER BY row_order"
        ))
        .map_err(RequestError::persistence)?;

    let fields = statement
        .query_map(params![owner_id], |row| {
            let order: i64 = row.get(0)?;
            let enabled: i64 = row.get(1)?;
            Ok(OrderedField {
                enabled: enabled != 0,
                order: u32::try_from(order).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
                name: row.get(2)?,
                value: row.get(3)?,
            })
        })
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    Ok(fields)
}
