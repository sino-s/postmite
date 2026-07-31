fn clear_session_cookie_metadata(connection: &Connection) -> Result<(), WorkspaceError> {
    connection
        .execute(
            "DELETE FROM workspace_cookies
             WHERE session = 1",
            [],
        )
        .map_err(WorkspaceError::persistence)?;
    Ok(())
}

fn insert_workspace(tx: &Transaction<'_>, workspace: &Workspace) -> Result<(), WorkspaceError> {
    tx.execute(
        "INSERT INTO workspaces (id, name) VALUES (?1, ?2)",
        params![workspace.id.to_string(), workspace.name.as_str()],
    )
    .map(|_| ())
    .map_err(map_sqlite_error)
}

fn select_workspace(tx: &Transaction<'_>, id: WorkspaceId) -> Result<(), WorkspaceError> {
    tx.execute(
        "INSERT INTO workspace_state (singleton, selected_workspace_id)
         VALUES (1, ?1)
         ON CONFLICT(singleton) DO UPDATE SET
             selected_workspace_id = excluded.selected_workspace_id,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        params![id.to_string()],
    )
    .map(|_| ())
    .map_err(map_sqlite_error)
}

fn load_snapshot(connection: &Connection) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let selected_workspace_id = selected_workspace_id(connection)?;
    let mut statement = connection
        .prepare("SELECT id, name, base_directory FROM workspaces ORDER BY created_at, id")
        .map_err(WorkspaceError::persistence)?;

    let workspaces = statement
        .query_map([], |row| {
            let id = workspace_id_from_row(row)?;
            let name: String = row.get(1)?;
            Ok(WorkspaceSummary {
                id,
                name: WorkspaceName::new(name).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
                is_selected: id == selected_workspace_id,
                base_directory: row.get(2)?,
            })
        })
        .map_err(WorkspaceError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(WorkspaceError::persistence)?;

    Ok(WorkspaceSnapshot {
        selected_workspace_id,
        workspaces,
    })
}

fn selected_workspace_id(connection: &Connection) -> Result<WorkspaceId, WorkspaceError> {
    connection
        .query_row(
            "SELECT selected_workspace_id FROM workspace_state WHERE singleton = 1",
            [],
            workspace_id_from_row,
        )
        .map_err(WorkspaceError::persistence)
}

fn ensure_workspace_exists(connection: &Connection, id: WorkspaceId) -> Result<(), WorkspaceError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM workspaces WHERE id = ?1",
            params![id.to_string()],
            |_| Ok(()),
        )
        .optional()
        .map_err(WorkspaceError::persistence)?;

    exists.ok_or(WorkspaceError::NotFound)
}

fn workspace_id_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceId> {
    let id: String = row.get(0)?;
    WorkspaceId::from_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn map_sqlite_error(error: rusqlite::Error) -> WorkspaceError {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == ErrorCode::ConstraintViolation =>
        {
            WorkspaceError::AlreadyExists
        }
        _ => WorkspaceError::persistence(error),
    }
}

fn ensure_request_workspace_exists(
    connection: &Connection,
    id: WorkspaceId,
) -> Result<(), RequestError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM workspaces WHERE id = ?1",
            params![id.to_string()],
            |_| Ok(()),
        )
        .optional()
        .map_err(RequestError::persistence)?;

    exists.ok_or(RequestError::WorkspaceNotFound)
}

fn collection_exists(
    connection: &Connection,
    workspace_id: WorkspaceId,
    collection_id: CollectionId,
) -> Result<bool, RequestError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM collections WHERE workspace_id = ?1 AND id = ?2)",
            params![workspace_id.to_string(), collection_id.to_string()],
            |row| row.get(0),
        )
        .map_err(RequestError::persistence)
}

fn ensure_draft_in_workspace(
    connection: &Connection,
    workspace_id: WorkspaceId,
    draft_id: RequestDraftId,
) -> Result<(), RequestError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM request_drafts WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), draft_id.to_string()],
            |_| Ok(()),
        )
        .optional()
        .map_err(RequestError::persistence)?;

    exists.ok_or(RequestError::NotFound)
}

fn ensure_saved_request_in_workspace(
    connection: &Connection,
    workspace_id: WorkspaceId,
    saved_request_id: SavedRequestId,
) -> Result<(), RequestError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM saved_requests WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), saved_request_id.to_string()],
            |_| Ok(()),
        )
        .optional()
        .map_err(RequestError::persistence)?;

    exists.ok_or(RequestError::NotFound)
}

fn ensure_collection_in_workspace(
    connection: &Connection,
    workspace_id: WorkspaceId,
    collection_id: CollectionId,
) -> Result<(), RequestError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM collections WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), collection_id.to_string()],
            |_| Ok(()),
        )
        .optional()
        .map_err(RequestError::persistence)?;

    exists.ok_or(RequestError::NotFound)
}

fn ensure_collection_parent(
    connection: &Connection,
    workspace_id: WorkspaceId,
    parent_collection_id: Option<CollectionId>,
) -> Result<(), RequestError> {
    match parent_collection_id {
        Some(collection_id) => {
            ensure_collection_in_workspace(connection, workspace_id, collection_id)
        }
        None => Ok(()),
    }
}

fn ensure_environment_in_workspace(
    connection: &Connection,
    workspace_id: WorkspaceId,
    environment_id: EnvironmentId,
) -> Result<(), RequestError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM environments WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), environment_id.to_string()],
            |_| Ok(()),
        )
        .optional()
        .map_err(RequestError::persistence)?;

    exists.ok_or(RequestError::NotFound)
}

fn insert_saved_request(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    saved_request_id: SavedRequestId,
    collection_id: Option<CollectionId>,
    content: &RequestContent,
) -> Result<(), RequestError> {
    let position = next_saved_request_position(tx, workspace_id, collection_id)?;
    insert_saved_request_at(
        tx,
        workspace_id,
        saved_request_id,
        collection_id,
        u32::try_from(position)
            .map_err(|_| RequestError::InvalidInput("position.tooLarge".to_owned()))?,
        content,
    )
}

fn insert_saved_request_at(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    saved_request_id: SavedRequestId,
    collection_id: Option<CollectionId>,
    position: u32,
    content: &RequestContent,
) -> Result<(), RequestError> {
    validate_request_content(content)?;
    ensure_collection_parent(tx, workspace_id, collection_id)?;
    tx.execute(
        "INSERT INTO saved_requests
            (id, workspace_id, collection_id, name, method, url, body, auth, redirect_policy,
             tls_policy, transport_policy, position)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            saved_request_id.to_string(),
            workspace_id.to_string(),
            collection_id.map(|id| id.to_string()),
            content.name.as_str(),
            content.method.as_str(),
            content.url.as_str(),
            request_body_to_sql(&content.body)?,
            request_auth_to_sql(&content.auth)?,
            redirect_policy_to_sql(&content.redirect)?,
            tls_policy_to_sql(&content.tls)?,
            transport_policy_to_sql(&content.transport)?,
            i64::from(position),
        ],
    )
    .map_err(map_request_sqlite_error)?;
    replace_fields(
        tx,
        "saved_request_query_rows",
        "saved_request_id",
        &saved_request_id.to_string(),
        &content.query,
    )?;
    replace_fields(
        tx,
        "saved_request_header_rows",
        "saved_request_id",
        &saved_request_id.to_string(),
        &content.headers,
    )
}

fn replace_saved_request_content(
    tx: &Transaction<'_>,
    saved_request_id: SavedRequestId,
    content: &RequestContent,
) -> Result<(), RequestError> {
    validate_request_content(content)?;
    tx.execute(
        "UPDATE saved_requests
         SET name = ?1, method = ?2, url = ?3, body = ?4, auth = ?5,
             redirect_policy = ?6, tls_policy = ?7, transport_policy = ?8,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?9",
        params![
            content.name.as_str(),
            content.method.as_str(),
            content.url.as_str(),
            request_body_to_sql(&content.body)?,
            request_auth_to_sql(&content.auth)?,
            redirect_policy_to_sql(&content.redirect)?,
            tls_policy_to_sql(&content.tls)?,
            transport_policy_to_sql(&content.transport)?,
            saved_request_id.to_string()
        ],
    )
    .map_err(map_request_sqlite_error)?;
    replace_fields(
        tx,
        "saved_request_query_rows",
        "saved_request_id",
        &saved_request_id.to_string(),
        &content.query,
    )?;
    replace_fields(
        tx,
        "saved_request_header_rows",
        "saved_request_id",
        &saved_request_id.to_string(),
        &content.headers,
    )
}

fn insert_draft(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    draft_id: RequestDraftId,
    saved_request_id: Option<SavedRequestId>,
    content: &RequestContent,
    is_dirty: bool,
) -> Result<(), RequestError> {
    validate_request_content(content)?;
    tx.execute(
        "INSERT INTO request_drafts
            (id, workspace_id, saved_request_id, name, method, url, body, auth,
             redirect_policy, tls_policy, transport_policy, is_dirty)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            draft_id.to_string(),
            workspace_id.to_string(),
            saved_request_id.map(|id| id.to_string()),
            content.name.as_str(),
            content.method.as_str(),
            content.url.as_str(),
            request_body_to_sql(&content.body)?,
            request_auth_to_sql(&content.auth)?,
            redirect_policy_to_sql(&content.redirect)?,
            tls_policy_to_sql(&content.tls)?,
            transport_policy_to_sql(&content.transport)?,
            bool_to_i64(is_dirty),
        ],
    )
    .map_err(map_request_sqlite_error)?;
    replace_fields(
        tx,
        "request_draft_query_rows",
        "draft_id",
        &draft_id.to_string(),
        &content.query,
    )?;
    replace_fields(
        tx,
        "request_draft_header_rows",
        "draft_id",
        &draft_id.to_string(),
        &content.headers,
    )
}

fn replace_draft_content(
    tx: &Transaction<'_>,
    draft_id: RequestDraftId,
    content: &RequestContent,
    is_dirty: bool,
) -> Result<(), RequestError> {
    validate_request_content(content)?;
    let changed = tx
        .execute(
            "UPDATE request_drafts
             SET name = ?1, method = ?2, url = ?3, body = ?4, auth = ?5,
                 redirect_policy = ?6, tls_policy = ?7, transport_policy = ?8, is_dirty = ?9,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?10",
            params![
                content.name.as_str(),
                content.method.as_str(),
                content.url.as_str(),
                request_body_to_sql(&content.body)?,
                request_auth_to_sql(&content.auth)?,
                redirect_policy_to_sql(&content.redirect)?,
                tls_policy_to_sql(&content.tls)?,
                transport_policy_to_sql(&content.transport)?,
                bool_to_i64(is_dirty),
                draft_id.to_string()
            ],
        )
        .map_err(map_request_sqlite_error)?;
    if changed == 0 {
        return Err(RequestError::NotFound);
    }
    replace_fields(
        tx,
        "request_draft_query_rows",
        "draft_id",
        &draft_id.to_string(),
        &content.query,
    )?;
    replace_fields(
        tx,
        "request_draft_header_rows",
        "draft_id",
        &draft_id.to_string(),
        &content.headers,
    )
}

fn insert_tab(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    tab_id: RequestTabId,
    saved_request_id: Option<SavedRequestId>,
    draft_id: RequestDraftId,
    title: String,
) -> Result<(), RequestError> {
    let position = next_tab_position(tx, workspace_id)?;
    deactivate_tabs(tx, workspace_id)?;
    tx.execute(
        "INSERT INTO request_tabs
            (id, workspace_id, saved_request_id, draft_id, position, title, is_active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        params![
            tab_id.to_string(),
            workspace_id.to_string(),
            saved_request_id.map(|id| id.to_string()),
            draft_id.to_string(),
            position,
            title,
        ],
    )
    .map(|_| ())
    .map_err(map_request_sqlite_error)
}

fn activate_tab(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    tab_id: RequestTabId,
) -> Result<(), RequestError> {
    deactivate_tabs(tx, workspace_id)?;
    tx.execute(
        "UPDATE request_tabs
         SET is_active = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE workspace_id = ?1 AND id = ?2",
        params![workspace_id.to_string(), tab_id.to_string()],
    )
    .map(|_| ())
    .map_err(map_request_sqlite_error)
}

fn deactivate_tabs(tx: &Transaction<'_>, workspace_id: WorkspaceId) -> Result<(), RequestError> {
    tx.execute(
        "UPDATE request_tabs SET is_active = 0 WHERE workspace_id = ?1",
        params![workspace_id.to_string()],
    )
    .map(|_| ())
    .map_err(map_request_sqlite_error)
}

fn compact_tab_positions(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
) -> Result<(), RequestError> {
    let mut statement = tx
        .prepare(
            "SELECT id FROM request_tabs WHERE workspace_id = ?1 ORDER BY position, created_at, id",
        )
        .map_err(RequestError::persistence)?;
    let tab_ids = statement
        .query_map(params![workspace_id.to_string()], request_tab_id_from_row)
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    drop(statement);

    for (position, tab_id) in tab_ids.into_iter().enumerate() {
        tx.execute(
            "UPDATE request_tabs SET position = ?1 WHERE id = ?2",
            params![position as i64, tab_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
    }
    Ok(())
}

fn next_tab_position(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<i64, RequestError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM request_tabs WHERE workspace_id = ?1",
            params![workspace_id.to_string()],
            |row| row.get(0),
        )
        .map_err(RequestError::persistence)
}

fn next_collection_position(
    connection: &Connection,
    workspace_id: WorkspaceId,
    parent_collection_id: Option<CollectionId>,
) -> Result<i64, RequestError> {
    let parent = parent_collection_id.map(|id| id.to_string());
    connection
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0)
             FROM collections
             WHERE workspace_id = ?1 AND parent_collection_id IS ?2",
            params![workspace_id.to_string(), parent],
            |row| row.get(0),
        )
        .map_err(RequestError::persistence)
}

fn next_saved_request_position(
    connection: &Connection,
    workspace_id: WorkspaceId,
    collection_id: Option<CollectionId>,
) -> Result<i64, RequestError> {
    let collection = collection_id.map(|id| id.to_string());
    connection
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0)
             FROM saved_requests
             WHERE workspace_id = ?1 AND collection_id IS ?2",
            params![workspace_id.to_string(), collection],
            |row| row.get(0),
        )
        .map_err(RequestError::persistence)
}

fn next_environment_position(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<i64, PostmanImportError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0)
             FROM environments
             WHERE workspace_id = ?1",
            params![workspace_id.to_string()],
            |row| row.get(0),
        )
        .map_err(PostmanImportError::persistence)
}

fn insert_environment_variable(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    environment_id: EnvironmentId,
    name: &str,
    value: &VariableValue,
) -> Result<(), PostmanImportError> {
    if name.trim().is_empty() {
        return Err(PostmanImportError::InvalidInput(
            "postman.environment.variable.name.required".to_owned(),
        ));
    }
    let (plain_value, secret_ref) = match value {
        VariableValue::Plain(value) => (Some(value.as_str()), None),
        VariableValue::SecretReference(reference) => (None, Some(reference.as_str())),
    };
    let position: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0)
             FROM environment_variables
             WHERE workspace_id = ?1 AND environment_id = ?2",
            params![workspace_id.to_string(), environment_id.to_string()],
            |row| row.get(0),
        )
        .map_err(PostmanImportError::persistence)?;
    tx.execute(
        "INSERT INTO environment_variables
            (environment_id, workspace_id, name, plain_value, secret_ref, position)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            environment_id.to_string(),
            workspace_id.to_string(),
            name.trim(),
            plain_value,
            secret_ref,
            position,
        ],
    )
    .map(|_| ())
    .map_err(map_postman_sqlite_error)
}

fn ensure_import_parent_present(
    collection: &ConvertedCollection,
    parent_id: Option<CollectionId>,
) -> Result<(), PostmanImportError> {
    if collection.parent_import_index.is_some() && parent_id.is_none() {
        return Err(PostmanImportError::InvalidInput(
            "postman.collection.parent.missing".to_owned(),
        ));
    }
    Ok(())
}

fn shift_collection_position(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    old_parent_collection_id: Option<CollectionId>,
    old_position: u32,
    new_parent_collection_id: Option<CollectionId>,
    new_position: u32,
) -> Result<(), RequestError> {
    shift_tree_positions(
        tx,
        "collections",
        "parent_collection_id",
        workspace_id,
        TreePositionMove {
            old_parent_id: old_parent_collection_id,
            old_position,
            new_parent_id: new_parent_collection_id,
            new_position,
        },
    )
}

fn shift_saved_request_position(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    old_collection_id: Option<CollectionId>,
    old_position: u32,
    new_collection_id: Option<CollectionId>,
    new_position: u32,
) -> Result<(), RequestError> {
    shift_tree_positions(
        tx,
        "saved_requests",
        "collection_id",
        workspace_id,
        TreePositionMove {
            old_parent_id: old_collection_id,
            old_position,
            new_parent_id: new_collection_id,
            new_position,
        },
    )
}

fn make_collection_position_space(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    parent_collection_id: Option<CollectionId>,
    position: u32,
) -> Result<(), RequestError> {
    make_tree_position_space(
        tx,
        "collections",
        "parent_collection_id",
        workspace_id,
        parent_collection_id,
        position,
    )
}

fn make_saved_request_position_space(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    collection_id: Option<CollectionId>,
    position: u32,
) -> Result<(), RequestError> {
    make_tree_position_space(
        tx,
        "saved_requests",
        "collection_id",
        workspace_id,
        collection_id,
        position,
    )
}

fn make_tree_position_space(
    tx: &Transaction<'_>,
    table: &str,
    parent_column: &str,
    workspace_id: WorkspaceId,
    parent_id: Option<CollectionId>,
    position: u32,
) -> Result<(), RequestError> {
    let parent = parent_id.map(|id| id.to_string());
    tx.execute(
        &format!(
            "UPDATE {table}
             SET position = position + 1
             WHERE workspace_id = ?1 AND {parent_column} IS ?2 AND position >= ?3"
        ),
        params![workspace_id.to_string(), parent, i64::from(position)],
    )
    .map(|_| ())
    .map_err(map_request_sqlite_error)
}

fn shift_tree_positions(
    tx: &Transaction<'_>,
    table: &str,
    parent_column: &str,
    workspace_id: WorkspaceId,
    position_move: TreePositionMove,
) -> Result<(), RequestError> {
    let old_parent = position_move.old_parent_id.map(|id| id.to_string());
    let new_parent = position_move.new_parent_id.map(|id| id.to_string());
    if old_parent == new_parent {
        if position_move.new_position < position_move.old_position {
            tx.execute(
                &format!(
                    "UPDATE {table}
                     SET position = position + 1
                     WHERE workspace_id = ?1 AND {parent_column} IS ?2
                       AND position >= ?3 AND position < ?4"
                ),
                params![
                    workspace_id.to_string(),
                    old_parent,
                    i64::from(position_move.new_position),
                    i64::from(position_move.old_position)
                ],
            )
            .map_err(map_request_sqlite_error)?;
        } else if position_move.new_position > position_move.old_position {
            tx.execute(
                &format!(
                    "UPDATE {table}
                     SET position = position - 1
                     WHERE workspace_id = ?1 AND {parent_column} IS ?2
                       AND position > ?3 AND position <= ?4"
                ),
                params![
                    workspace_id.to_string(),
                    old_parent,
                    i64::from(position_move.old_position),
                    i64::from(position_move.new_position)
                ],
            )
            .map_err(map_request_sqlite_error)?;
        }
        return Ok(());
    }

    tx.execute(
        &format!(
            "UPDATE {table}
             SET position = position - 1
             WHERE workspace_id = ?1 AND {parent_column} IS ?2 AND position > ?3"
        ),
        params![
            workspace_id.to_string(),
            old_parent,
            i64::from(position_move.old_position)
        ],
    )
    .map_err(map_request_sqlite_error)?;
    tx.execute(
        &format!(
            "UPDATE {table}
             SET position = position + 1
             WHERE workspace_id = ?1 AND {parent_column} IS ?2 AND position >= ?3"
        ),
        params![
            workspace_id.to_string(),
            new_parent,
            i64::from(position_move.new_position)
        ],
    )
    .map_err(map_request_sqlite_error)?;
    Ok(())
}

struct TreePositionMove {
    old_parent_id: Option<CollectionId>,
    old_position: u32,
    new_parent_id: Option<CollectionId>,
    new_position: u32,
}

fn compact_collection_positions(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
) -> Result<(), RequestError> {
    let parents = collection_parent_ids(tx, workspace_id)?;
    for parent in parents {
        let parent_text = parent.map(|id| id.to_string());
        let mut statement = tx
            .prepare(
                "SELECT id
                 FROM collections
                 WHERE workspace_id = ?1 AND parent_collection_id IS ?2
                 ORDER BY position, updated_at, created_at, id",
            )
            .map_err(RequestError::persistence)?;
        let ids = statement
            .query_map(
                params![workspace_id.to_string(), parent_text],
                collection_id_from_row,
            )
            .map_err(RequestError::persistence)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(RequestError::persistence)?;
        drop(statement);

        for (position, id) in ids.into_iter().enumerate() {
            tx.execute(
                "UPDATE collections SET position = ?1 WHERE workspace_id = ?2 AND id = ?3",
                params![position as i64, workspace_id.to_string(), id.to_string()],
            )
            .map_err(map_request_sqlite_error)?;
        }
    }
    Ok(())
}

fn compact_saved_request_positions(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
) -> Result<(), RequestError> {
    let collections = saved_request_collection_ids(tx, workspace_id)?;
    for collection_id in collections {
        let collection_text = collection_id.map(|id| id.to_string());
        let mut statement = tx
            .prepare(
                "SELECT id
                 FROM saved_requests
                 WHERE workspace_id = ?1 AND collection_id IS ?2
                 ORDER BY position, updated_at, created_at, id",
            )
            .map_err(RequestError::persistence)?;
        let ids = statement
            .query_map(
                params![workspace_id.to_string(), collection_text],
                saved_request_id_from_row,
            )
            .map_err(RequestError::persistence)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(RequestError::persistence)?;
        drop(statement);

        for (position, id) in ids.into_iter().enumerate() {
            tx.execute(
                "UPDATE saved_requests SET position = ?1 WHERE workspace_id = ?2 AND id = ?3",
                params![position as i64, workspace_id.to_string(), id.to_string()],
            )
            .map_err(map_request_sqlite_error)?;
        }
    }
    Ok(())
}

fn collection_parent_ids(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<Option<CollectionId>>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT parent_collection_id
             FROM collections
             WHERE workspace_id = ?1
             ORDER BY parent_collection_id",
        )
        .map_err(RequestError::persistence)?;
    let mut parents = statement
        .query_map(params![workspace_id.to_string()], |row| {
            optional_collection_id_from_row(row, 0)
        })
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    if !parents.iter().any(Option::is_none) {
        parents.push(None);
    }
    Ok(parents)
}

fn saved_request_collection_ids(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<Option<CollectionId>>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT collection_id
             FROM saved_requests
             WHERE workspace_id = ?1
             ORDER BY collection_id",
        )
        .map_err(RequestError::persistence)?;
    let mut collections = statement
        .query_map(params![workspace_id.to_string()], |row| {
            optional_collection_id_from_row(row, 0)
        })
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    if !collections.iter().any(Option::is_none) {
        collections.push(None);
    }
    Ok(collections)
}

fn collection_descends_from(
    connection: &Connection,
    workspace_id: WorkspaceId,
    collection_id: Option<CollectionId>,
    ancestor_id: CollectionId,
) -> Result<bool, RequestError> {
    let Some(mut cursor) = collection_id else {
        return Ok(false);
    };
    loop {
        if cursor == ancestor_id {
            return Ok(true);
        }
        let parent = connection
            .query_row(
                "SELECT parent_collection_id
                 FROM collections
                 WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id.to_string(), cursor.to_string()],
                |row| optional_collection_id_from_row(row, 0),
            )
            .optional()
            .map_err(RequestError::persistence)?
            .ok_or(RequestError::NotFound)?;
        match parent {
            Some(parent_id) => cursor = parent_id,
            None => return Ok(false),
        }
    }
}

fn duplicate_collection_subtree(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    source_id: CollectionId,
    parent_override: Option<CollectionId>,
) -> Result<CollectionId, RequestError> {
    let source = load_collection_folder(tx, workspace_id, source_id)?;
    let new_id = CollectionId::new();
    let parent_collection_id = parent_override.or(source.parent_collection_id);
    let position = source.position.saturating_add(1);
    make_collection_position_space(tx, workspace_id, parent_collection_id, position)?;
    tx.execute(
        "INSERT INTO collections
            (id, workspace_id, parent_collection_id, name, position)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            new_id.to_string(),
            workspace_id.to_string(),
            parent_collection_id.map(|id| id.to_string()),
            format!("{} Copy", source.name),
            i64::from(position),
        ],
    )
    .map_err(map_request_sqlite_error)?;

    for request in load_saved_requests_in_collection(tx, workspace_id, Some(source_id))? {
        insert_saved_request_at(
            tx,
            workspace_id,
            SavedRequestId::new(),
            Some(new_id),
            request.position,
            &request.content,
        )?;
    }
    for child in load_child_collections(tx, workspace_id, source_id)? {
        duplicate_collection_subtree(tx, workspace_id, child.id, Some(new_id))?;
    }
    Ok(new_id)
}

fn delete_collection_subtree(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    collection_id: CollectionId,
) -> Result<(), RequestError> {
    for child in load_child_collections(tx, workspace_id, collection_id)? {
        delete_collection_subtree(tx, workspace_id, child.id)?;
    }
    let request_ids = load_saved_requests_in_collection(tx, workspace_id, Some(collection_id))?
        .into_iter()
        .map(|request| request.id)
        .collect::<Vec<_>>();
    for request_id in request_ids {
        tx.execute(
            "DELETE FROM request_tabs
             WHERE workspace_id = ?1 AND saved_request_id = ?2",
            params![workspace_id.to_string(), request_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
        tx.execute(
            "DELETE FROM request_drafts
             WHERE workspace_id = ?1 AND saved_request_id = ?2",
            params![workspace_id.to_string(), request_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
        tx.execute(
            "DELETE FROM saved_requests WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), request_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
    }
    tx.execute(
        "DELETE FROM collections WHERE workspace_id = ?1 AND id = ?2",
        params![workspace_id.to_string(), collection_id.to_string()],
    )
    .map_err(map_request_sqlite_error)?;
    compact_tab_positions(tx, workspace_id)
}

fn replace_fields(
    tx: &Transaction<'_>,
    table: &str,
    owner_column: &str,
    owner_id: &str,
    fields: &[OrderedField],
) -> Result<(), RequestError> {
    tx.execute(
        &format!("DELETE FROM {table} WHERE {owner_column} = ?1"),
        params![owner_id],
    )
    .map_err(map_request_sqlite_error)?;

    let sql = format!(
        "INSERT INTO {table} ({owner_column}, row_order, enabled, name, value)
         VALUES (?1, ?2, ?3, ?4, ?5)"
    );
    for field in fields {
        tx.execute(
            &sql,
            params![
                owner_id,
                i64::from(field.order),
                bool_to_i64(field.enabled),
                field.name.as_str(),
                field.value.as_str(),
            ],
        )
        .map_err(map_request_sqlite_error)?;
    }
    Ok(())
}

fn insert_execution_record(
    tx: &Transaction<'_>,
    draft: ExecutionRecordDraft,
) -> Result<(), RequestError> {
    let record_id = ExecutionRecordId::new();
    validate_request_content(&draft.content)?;
    tx.execute(
        "INSERT INTO execution_records
            (id, workspace_id, created_at_epoch_seconds, pinned, name, method, url, body,
             auth, redirect_policy, tls_policy, transport_policy, response_status, response_body_preview, response_body_truncated,
             response_error, response_duration_ms)
         VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            record_id.to_string(),
            draft.workspace_id.to_string(),
            draft.completed_at_epoch_seconds,
            draft.content.name.as_str(),
            draft.content.method.as_str(),
            draft.content.url.as_str(),
            request_body_to_sql(&draft.content.body)?,
            request_auth_to_sql(&draft.content.auth)?,
            redirect_policy_to_sql(&draft.content.redirect)?,
            tls_policy_to_sql(&draft.content.tls)?,
            transport_policy_to_sql(&draft.content.transport)?,
            draft.response.status.map(i64::from),
            draft.response.body_preview.as_str(),
            bool_to_i64(draft.response.body_truncated),
            draft.response.error.as_deref(),
            draft.response.duration_ms.map(|value| value as i64),
        ],
    )
    .map_err(map_request_sqlite_error)?;
    replace_fields(
        tx,
        "execution_record_query_rows",
        "execution_record_id",
        &record_id.to_string(),
        &draft.content.query,
    )?;
    replace_fields(
        tx,
        "execution_record_header_rows",
        "execution_record_id",
        &record_id.to_string(),
        &draft.content.headers,
    )?;
    replace_fields(
        tx,
        "execution_record_response_header_rows",
        "execution_record_id",
        &record_id.to_string(),
        &draft.response.headers,
    )?;
    cleanup_execution_records(
        tx,
        draft.workspace_id,
        Some(draft.completed_at_epoch_seconds),
    )
}

fn cleanup_execution_records(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    now_epoch_seconds: Option<i64>,
) -> Result<(), RequestError> {
    let now_epoch_seconds = now_epoch_seconds.unwrap_or_else(current_epoch_seconds);
    let cutoff_epoch_seconds = now_epoch_seconds - EXECUTION_HISTORY_RETENTION_DAYS * 24 * 60 * 60;
    tx.execute(
        "DELETE FROM execution_records
         WHERE workspace_id = ?1 AND pinned = 0 AND created_at_epoch_seconds < ?2",
        params![workspace_id.to_string(), cutoff_epoch_seconds],
    )
    .map_err(map_request_sqlite_error)?;

    let mut statement = tx
        .prepare(
            "SELECT id
             FROM execution_records
             WHERE workspace_id = ?1 AND pinned = 0
             ORDER BY created_at_epoch_seconds DESC, id DESC
             LIMIT -1 OFFSET ?2",
        )
        .map_err(RequestError::persistence)?;
    let ids = statement
        .query_map(
            params![
                workspace_id.to_string(),
                i64::try_from(EXECUTION_HISTORY_RETENTION_LIMIT)
                    .map_err(|_| RequestError::InvalidInput("history.limit".to_owned()))?,
            ],
            execution_record_id_from_row,
        )
        .map_err(RequestError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(RequestError::persistence)?;
    drop(statement);

    for id in ids {
        tx.execute(
            "DELETE FROM execution_records WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
    }
    Ok(())
}
