use std::{path::Path, str::FromStr, time::Duration};

use rusqlite::{params, Connection, ErrorCode, OptionalExtension, Transaction};

use crate::{
    application::request::{RequestError, RequestRepository, RequestWorkspaceSnapshot},
    application::workspace::{
        WorkspaceError, WorkspaceRepository, WorkspaceSnapshot, WorkspaceSummary,
    },
    domain::{
        request::{
            CollectionId, OrderedField, RequestContent, RequestDraft, RequestDraftId, RequestTab,
            RequestTabId, SavedRequest, SavedRequestId,
        },
        workspace::{Workspace, WorkspaceId, WorkspaceName, DEFAULT_WORKSPACE_NAME},
    },
};

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "create_workspace_tables",
        sql: r#"
CREATE TABLE workspaces (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE workspace_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    selected_workspace_id TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (selected_workspace_id) REFERENCES workspaces(id)
        ON UPDATE CASCADE
        ON DELETE RESTRICT
);
"#,
    },
    Migration {
        version: 2,
        name: "create_request_tables",
        sql: r#"
CREATE TABLE collections (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE saved_requests (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    collection_id TEXT,
    name TEXT NOT NULL CHECK (length(name) > 0),
    method TEXT NOT NULL CHECK (length(method) > 0),
    url TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id, workspace_id) REFERENCES collections(id, workspace_id)
);

CREATE TABLE saved_request_query_rows (
    saved_request_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (saved_request_id, row_order),
    FOREIGN KEY (saved_request_id) REFERENCES saved_requests(id) ON DELETE CASCADE
);

CREATE TABLE saved_request_header_rows (
    saved_request_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (saved_request_id, row_order),
    FOREIGN KEY (saved_request_id) REFERENCES saved_requests(id) ON DELETE CASCADE
);

CREATE TABLE request_drafts (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    saved_request_id TEXT,
    name TEXT NOT NULL CHECK (length(name) > 0),
    method TEXT NOT NULL CHECK (length(method) > 0),
    url TEXT NOT NULL,
    is_dirty INTEGER NOT NULL CHECK (is_dirty IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (saved_request_id, workspace_id) REFERENCES saved_requests(id, workspace_id)
);

CREATE TABLE request_draft_query_rows (
    draft_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (draft_id, row_order),
    FOREIGN KEY (draft_id) REFERENCES request_drafts(id) ON DELETE CASCADE
);

CREATE TABLE request_draft_header_rows (
    draft_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (draft_id, row_order),
    FOREIGN KEY (draft_id) REFERENCES request_drafts(id) ON DELETE CASCADE
);

CREATE TABLE request_tabs (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    saved_request_id TEXT,
    draft_id TEXT NOT NULL UNIQUE,
    position INTEGER NOT NULL CHECK (position >= 0),
    title TEXT NOT NULL CHECK (length(title) > 0),
    is_active INTEGER NOT NULL CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (saved_request_id, workspace_id) REFERENCES saved_requests(id, workspace_id),
    FOREIGN KEY (draft_id, workspace_id) REFERENCES request_drafts(id, workspace_id)
        ON DELETE CASCADE
);

CREATE UNIQUE INDEX request_tabs_one_saved_request_per_workspace
    ON request_tabs(workspace_id, saved_request_id)
    WHERE saved_request_id IS NOT NULL;
"#,
    },
    Migration {
        version: 3,
        name: "add_raw_request_body",
        sql: r#"
ALTER TABLE saved_requests ADD COLUMN body TEXT NOT NULL DEFAULT '';
ALTER TABLE request_drafts ADD COLUMN body TEXT NOT NULL DEFAULT '';
"#,
    },
];

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

pub struct SqliteWorkspaceRepository {
    connection: Connection,
}

impl SqliteWorkspaceRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let connection = Connection::open(path).map_err(WorkspaceError::persistence)?;
        configure_connection(&connection)?;
        apply_migrations(&connection)?;

        Ok(Self { connection })
    }

    #[cfg(test)]
    fn connection(&self) -> &Connection {
        &self.connection
    }
}

impl WorkspaceRepository for SqliteWorkspaceRepository {
    fn initialize(&mut self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let tx = self
            .connection
            .transaction()
            .map_err(WorkspaceError::persistence)?;

        let has_workspace: bool = tx
            .query_row("SELECT EXISTS(SELECT 1 FROM workspaces)", [], |row| {
                row.get(0)
            })
            .map_err(WorkspaceError::persistence)?;

        if !has_workspace {
            let workspace = Workspace::new(
                WorkspaceName::new(DEFAULT_WORKSPACE_NAME).map_err(WorkspaceError::InvalidName)?,
            );
            insert_workspace(&tx, &workspace)?;
            select_workspace(&tx, workspace.id)?;
        }

        let snapshot = load_snapshot(&tx)?;
        tx.commit().map_err(WorkspaceError::persistence)?;
        Ok(snapshot)
    }

    fn create_workspace(
        &mut self,
        name: WorkspaceName,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let tx = self
            .connection
            .transaction()
            .map_err(WorkspaceError::persistence)?;
        let workspace = Workspace::new(name);

        insert_workspace(&tx, &workspace)?;
        select_workspace(&tx, workspace.id)?;

        let snapshot = load_snapshot(&tx)?;
        tx.commit().map_err(WorkspaceError::persistence)?;
        Ok(snapshot)
    }

    fn list_workspaces(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        load_snapshot(&self.connection)
    }

    fn rename_workspace(
        &mut self,
        id: WorkspaceId,
        name: WorkspaceName,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let tx = self
            .connection
            .transaction()
            .map_err(WorkspaceError::persistence)?;

        let changed = tx
            .execute(
                "UPDATE workspaces
                 SET name = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?2",
                params![name.as_str(), id.to_string()],
            )
            .map_err(map_sqlite_error)?;

        if changed == 0 {
            return Err(WorkspaceError::NotFound);
        }

        let snapshot = load_snapshot(&tx)?;
        tx.commit().map_err(WorkspaceError::persistence)?;
        Ok(snapshot)
    }

    fn switch_workspace(&mut self, id: WorkspaceId) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let tx = self
            .connection
            .transaction()
            .map_err(WorkspaceError::persistence)?;

        ensure_workspace_exists(&tx, id)?;
        select_workspace(&tx, id)?;

        let snapshot = load_snapshot(&tx)?;
        tx.commit().map_err(WorkspaceError::persistence)?;
        Ok(snapshot)
    }

    fn delete_workspace(&mut self, id: WorkspaceId) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let tx = self
            .connection
            .transaction()
            .map_err(WorkspaceError::persistence)?;

        ensure_workspace_exists(&tx, id)?;

        let workspace_count: i64 = tx
            .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
            .map_err(WorkspaceError::persistence)?;
        if workspace_count <= 1 {
            return Err(WorkspaceError::CannotDeleteLastWorkspace);
        }

        let selected_workspace_id = selected_workspace_id(&tx)?;
        if selected_workspace_id == id {
            let replacement = tx
                .query_row(
                    "SELECT id FROM workspaces WHERE id <> ?1 ORDER BY created_at, id LIMIT 1",
                    params![id.to_string()],
                    workspace_id_from_row,
                )
                .map_err(WorkspaceError::persistence)?;
            select_workspace(&tx, replacement)?;
        }

        tx.execute(
            "DELETE FROM workspaces WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(map_sqlite_error)?;

        let snapshot = load_snapshot(&tx)?;
        tx.commit().map_err(WorkspaceError::persistence)?;
        Ok(snapshot)
    }
}

impl RequestRepository for SqliteWorkspaceRepository {
    fn list_request_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        ensure_request_workspace_exists(&self.connection, workspace_id)?;
        load_request_snapshot(&self.connection, workspace_id)
    }

    fn open_unsaved_tab(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;

        let content = RequestContent::blank();
        let draft_id = RequestDraftId::new();
        insert_draft(&tx, workspace_id, draft_id, None, &content, true)?;
        insert_tab(
            &tx,
            workspace_id,
            RequestTabId::new(),
            None,
            draft_id,
            content.name.clone(),
        )?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn create_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        content: RequestContent,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        insert_saved_request(&tx, workspace_id, SavedRequestId::new(), None, &content)?;
        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn open_saved_request_tab(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;

        let existing_tab = tx
            .query_row(
                "SELECT id FROM request_tabs
                 WHERE workspace_id = ?1 AND saved_request_id = ?2",
                params![workspace_id.to_string(), saved_request_id.to_string()],
                request_tab_id_from_row,
            )
            .optional()
            .map_err(RequestError::persistence)?;

        if let Some(tab_id) = existing_tab {
            activate_tab(&tx, workspace_id, tab_id)?;
            let snapshot = load_request_snapshot(&tx, workspace_id)?;
            tx.commit().map_err(RequestError::persistence)?;
            return Ok(snapshot);
        }

        let saved_request = load_saved_request(&tx, workspace_id, saved_request_id)?;
        let draft_id = RequestDraftId::new();
        insert_draft(
            &tx,
            workspace_id,
            draft_id,
            Some(saved_request.id),
            &saved_request.content,
            false,
        )?;
        insert_tab(
            &tx,
            workspace_id,
            RequestTabId::new(),
            Some(saved_request.id),
            draft_id,
            saved_request.content.name,
        )?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn persist_draft(
        &mut self,
        workspace_id: WorkspaceId,
        draft_id: RequestDraftId,
        content: RequestContent,
    ) -> Result<(), RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_draft_in_workspace(&tx, workspace_id, draft_id)?;
        replace_draft_content(&tx, draft_id, &content, true)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(())
    }

    fn save_draft(
        &mut self,
        workspace_id: WorkspaceId,
        draft_id: RequestDraftId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_draft_in_workspace(&tx, workspace_id, draft_id)?;

        let draft = load_draft(&tx, workspace_id, draft_id)?;
        let saved_request_id = match draft.saved_request_id {
            Some(saved_request_id) => {
                replace_saved_request_content(&tx, saved_request_id, &draft.content)?;
                saved_request_id
            }
            None => {
                let saved_request_id = SavedRequestId::new();
                insert_saved_request(&tx, workspace_id, saved_request_id, None, &draft.content)?;
                tx.execute(
                    "UPDATE request_drafts
                     SET saved_request_id = ?1
                     WHERE id = ?2",
                    params![saved_request_id.to_string(), draft_id.to_string()],
                )
                .map_err(map_request_sqlite_error)?;
                tx.execute(
                    "UPDATE request_tabs
                     SET saved_request_id = ?1, title = ?2,
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     WHERE draft_id = ?3",
                    params![
                        saved_request_id.to_string(),
                        draft.content.name.as_str(),
                        draft_id.to_string()
                    ],
                )
                .map_err(map_request_sqlite_error)?;
                saved_request_id
            }
        };

        tx.execute(
            "UPDATE request_drafts
             SET is_dirty = 0, saved_request_id = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2",
            params![saved_request_id.to_string(), draft_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn close_tab(
        &mut self,
        workspace_id: WorkspaceId,
        tab_id: RequestTabId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;

        let draft_id = tx
            .query_row(
                "SELECT draft_id FROM request_tabs WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id.to_string(), tab_id.to_string()],
                request_draft_id_from_row,
            )
            .optional()
            .map_err(RequestError::persistence)?
            .ok_or(RequestError::NotFound)?;

        tx.execute(
            "DELETE FROM request_tabs WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), tab_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
        tx.execute(
            "DELETE FROM request_drafts WHERE id = ?1",
            params![draft_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
        compact_tab_positions(&tx, workspace_id)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }
}

fn configure_connection(connection: &Connection) -> Result<(), WorkspaceError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(WorkspaceError::persistence)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(WorkspaceError::persistence)?;
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(WorkspaceError::persistence)?;
    Ok(())
}

fn apply_migrations(connection: &Connection) -> Result<(), WorkspaceError> {
    connection
        .execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
"#,
        )
        .map_err(WorkspaceError::persistence)?;

    for migration in MIGRATIONS {
        let tx = connection
            .unchecked_transaction()
            .map_err(WorkspaceError::persistence)?;
        let already_applied = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                params![migration.version],
                |row| row.get::<_, bool>(0),
            )
            .map_err(WorkspaceError::persistence)?;

        if !already_applied {
            tx.execute_batch(migration.sql)
                .map_err(WorkspaceError::persistence)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                params![migration.version, migration.name],
            )
            .map_err(WorkspaceError::persistence)?;
        }

        tx.commit().map_err(WorkspaceError::persistence)?;
    }

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
        .prepare("SELECT id, name FROM workspaces ORDER BY created_at, id")
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

fn insert_saved_request(
    tx: &Transaction<'_>,
    workspace_id: WorkspaceId,
    saved_request_id: SavedRequestId,
    collection_id: Option<CollectionId>,
    content: &RequestContent,
) -> Result<(), RequestError> {
    validate_request_content(content)?;
    tx.execute(
        "INSERT INTO saved_requests (id, workspace_id, collection_id, name, method, url, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            saved_request_id.to_string(),
            workspace_id.to_string(),
            collection_id.map(|id| id.to_string()),
            content.name.as_str(),
            content.method.as_str(),
            content.url.as_str(),
            content.body.as_str(),
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
         SET name = ?1, method = ?2, url = ?3, body = ?4,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?5",
        params![
            content.name.as_str(),
            content.method.as_str(),
            content.url.as_str(),
            content.body.as_str(),
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
            (id, workspace_id, saved_request_id, name, method, url, body, is_dirty)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            draft_id.to_string(),
            workspace_id.to_string(),
            saved_request_id.map(|id| id.to_string()),
            content.name.as_str(),
            content.method.as_str(),
            content.url.as_str(),
            content.body.as_str(),
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
             SET name = ?1, method = ?2, url = ?3, body = ?4, is_dirty = ?5,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?6",
            params![
                content.name.as_str(),
                content.method.as_str(),
                content.url.as_str(),
                content.body.as_str(),
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

fn load_request_snapshot(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<RequestWorkspaceSnapshot, RequestError> {
    Ok(RequestWorkspaceSnapshot {
        workspace_id,
        saved_requests: load_saved_requests(connection, workspace_id)?,
        drafts: load_open_drafts(connection, workspace_id)?,
        tabs: load_tabs(connection, workspace_id)?,
    })
}

fn load_saved_requests(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> Result<Vec<SavedRequest>, RequestError> {
    let mut statement = connection
        .prepare(
            "SELECT id, workspace_id, collection_id, name, method, url, body
             FROM saved_requests
             WHERE workspace_id = ?1
             ORDER BY created_at, id",
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
                body: row.get(6)?,
                query: Vec::new(),
                headers: Vec::new(),
            };
            Ok(SavedRequest {
                id,
                workspace_id: row_workspace_id,
                collection_id,
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
            "SELECT DISTINCT d.id, d.workspace_id, d.saved_request_id, d.name, d.method, d.url, d.body, d.is_dirty
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
            "SELECT id, workspace_id, saved_request_id, name, method, url, body, is_dirty
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

fn validate_request_content(content: &RequestContent) -> Result<(), RequestError> {
    if content.name.trim().is_empty() {
        return Err(RequestError::InvalidInput("name.required".to_owned()));
    }
    if content.method.trim().is_empty() {
        return Err(RequestError::InvalidInput("method.required".to_owned()));
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
            body: row.get(6)?,
            query: Vec::new(),
            headers: Vec::new(),
        },
        is_dirty: row.get::<_, i64>(7)? != 0,
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

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;
    use crate::{
        application::request::{RequestRepository, RequestService},
        application::workspace::WorkspaceRepository,
        domain::request::{OrderedField, RequestContent, RequestDraftId},
    };

    fn repository() -> SqliteWorkspaceRepository {
        let db = NamedTempFile::new().expect("temporary database");
        SqliteWorkspaceRepository::open(db.path()).expect("open database")
    }

    #[test]
    fn first_run_creates_default_selected_workspace() {
        let mut repository = repository();
        let snapshot = repository.initialize().expect("initialize");

        assert_eq!(snapshot.workspaces.len(), 1);
        assert_eq!(snapshot.workspaces[0].name.as_str(), DEFAULT_WORKSPACE_NAME);
        assert_eq!(snapshot.selected_workspace_id, snapshot.workspaces[0].id);
        assert!(snapshot.workspaces[0].is_selected);
    }

    #[test]
    fn two_workspaces_can_be_created_and_switched() {
        let mut repository = repository();
        repository.initialize().expect("initialize");

        let created = repository
            .create_workspace(WorkspaceName::new("Client").expect("valid name"))
            .expect("create workspace");
        assert_eq!(created.workspaces.len(), 2);
        let original = created
            .workspaces
            .iter()
            .find(|workspace| workspace.name.as_str() == DEFAULT_WORKSPACE_NAME)
            .expect("original workspace")
            .id;
        let client = created.selected_workspace_id;

        let switched = repository
            .switch_workspace(original)
            .expect("switch workspace");
        assert_eq!(switched.selected_workspace_id, original);
        assert_ne!(switched.selected_workspace_id, client);
    }

    #[test]
    fn restart_restores_selected_workspace() {
        let db = NamedTempFile::new().expect("temporary database");
        let selected = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            repository.initialize().expect("initialize");
            repository
                .create_workspace(WorkspaceName::new("Restarted").expect("valid name"))
                .expect("create workspace")
                .selected_workspace_id
        };

        let mut reopened = SqliteWorkspaceRepository::open(db.path()).expect("reopen database");
        let snapshot = reopened.initialize().expect("initialize after restart");
        assert_eq!(snapshot.selected_workspace_id, selected);
        assert!(snapshot
            .workspaces
            .iter()
            .any(|workspace| workspace.id == selected && workspace.is_selected));
    }

    #[test]
    fn duplicate_create_rolls_back_without_partial_rows() {
        let mut repository = repository();
        repository.initialize().expect("initialize");
        repository
            .create_workspace(WorkspaceName::new("Duplicate").expect("valid name"))
            .expect("create workspace");

        let result =
            repository.create_workspace(WorkspaceName::new("Duplicate").expect("valid name"));

        assert!(matches!(result, Err(WorkspaceError::AlreadyExists)));
        let snapshot = repository.list_workspaces().expect("list workspaces");
        assert_eq!(snapshot.workspaces.len(), 2);
        assert_eq!(
            snapshot
                .workspaces
                .iter()
                .filter(|workspace| workspace.name.as_str() == "Duplicate")
                .count(),
            1
        );
    }

    #[test]
    fn deleting_selected_workspace_selects_remaining_workspace_atomically() {
        let mut repository = repository();
        let initial = repository.initialize().expect("initialize");
        let initial_id = initial.selected_workspace_id;
        let created = repository
            .create_workspace(WorkspaceName::new("Temporary").expect("valid name"))
            .expect("create workspace");

        let after_delete = repository
            .delete_workspace(created.selected_workspace_id)
            .expect("delete workspace");

        assert_eq!(after_delete.workspaces.len(), 1);
        assert_eq!(after_delete.selected_workspace_id, initial_id);
        assert!(after_delete.workspaces[0].is_selected);
    }

    #[test]
    fn schema_enforces_workspace_ownership_for_selected_state() {
        let mut repository = repository();
        repository.initialize().expect("initialize");

        let result = repository.connection().execute(
            "INSERT INTO workspace_state (singleton, selected_workspace_id)
             VALUES (1, '00000000-0000-4000-8000-000000000000')
             ON CONFLICT(singleton) DO UPDATE SET selected_workspace_id = excluded.selected_workspace_id",
            [],
        );

        assert!(matches!(
            result,
            Err(rusqlite::Error::SqliteFailure(failure, _))
                if failure.code == ErrorCode::ConstraintViolation
        ));
    }

    #[test]
    fn schema_enforces_workspace_ownership_for_request_drafts() {
        let mut repository = repository();
        let first_workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let second_workspace_id = repository
            .create_workspace(WorkspaceName::new("Second").expect("valid name"))
            .expect("create workspace")
            .selected_workspace_id;
        let snapshot = repository
            .create_saved_request(first_workspace_id, request_content("Saved", "url"))
            .expect("create saved request");
        let saved_request_id = snapshot.saved_requests[0].id;

        let result = repository.connection().execute(
            "INSERT INTO request_drafts
                (id, workspace_id, saved_request_id, name, method, url, body, is_dirty)
             VALUES (?1, ?2, ?3, 'Cross workspace', 'GET', '', '', 1)",
            params![
                RequestDraftId::new().to_string(),
                second_workspace_id.to_string(),
                saved_request_id.to_string()
            ],
        );

        assert!(matches!(
            result,
            Err(rusqlite::Error::SqliteFailure(failure, _))
                if failure.code == ErrorCode::ConstraintViolation
        ));
    }

    #[test]
    fn connection_uses_wal_and_foreign_keys() {
        let repository = repository();
        let journal_mode: String = repository
            .connection()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");
        let foreign_keys: i64 = repository
            .connection()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign keys");

        assert_eq!(journal_mode, "wal");
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn saving_a_draft_does_not_mutate_saved_request_until_explicit_save() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let created = repository
            .create_saved_request(workspace_id, request_content("Saved", "original"))
            .expect("create saved");
        let saved_request_id = created.saved_requests[0].id;
        let opened = repository
            .open_saved_request_tab(workspace_id, saved_request_id)
            .expect("open saved");
        let draft_id = opened.drafts[0].id;

        repository
            .persist_draft(workspace_id, draft_id, request_content("Draft", "edited"))
            .expect("persist draft");
        let before_save = repository
            .list_request_workspace(workspace_id)
            .expect("snapshot before save");

        assert_eq!(before_save.saved_requests[0].content.name, "Saved");
        assert_eq!(before_save.saved_requests[0].content.url, "original");
        assert_eq!(before_save.drafts[0].content.name, "Draft");
        assert_eq!(before_save.drafts[0].content.url, "edited");

        let after_save = repository
            .save_draft(workspace_id, draft_id)
            .expect("save draft");
        assert_eq!(after_save.saved_requests[0].content.name, "Draft");
        assert_eq!(after_save.saved_requests[0].content.url, "edited");
        assert!(!after_save.drafts[0].is_dirty);
    }

    #[test]
    fn duplicate_ordered_fields_round_trip_through_sqlite() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let content = RequestContent {
            name: "Fields".to_owned(),
            method: "GET".to_owned(),
            url: "https://example.test".to_owned(),
            body: "{\"ok\":true}".to_owned(),
            query: vec![
                OrderedField {
                    enabled: true,
                    order: 0,
                    name: "tag".to_owned(),
                    value: "first".to_owned(),
                },
                OrderedField {
                    enabled: false,
                    order: 1,
                    name: "tag".to_owned(),
                    value: String::new(),
                },
            ],
            headers: vec![
                OrderedField {
                    enabled: true,
                    order: 0,
                    name: "X-Test".to_owned(),
                    value: "one".to_owned(),
                },
                OrderedField {
                    enabled: true,
                    order: 1,
                    name: "X-Test".to_owned(),
                    value: String::new(),
                },
            ],
        };

        let snapshot = repository
            .create_saved_request(workspace_id, content.clone())
            .expect("create saved");

        assert_eq!(snapshot.saved_requests[0].content.query, content.query);
        assert_eq!(snapshot.saved_requests[0].content.headers, content.headers);
        assert_eq!(snapshot.saved_requests[0].content.body, "{\"ok\":true}");
    }

    #[test]
    fn unsaved_tabs_and_drafts_restore_after_restart() {
        let db = NamedTempFile::new().expect("temporary database");
        let (workspace_id, draft_id, tab_id) = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            let workspace_id = repository
                .initialize()
                .expect("initialize")
                .selected_workspace_id;
            let opened = repository
                .open_unsaved_tab(workspace_id)
                .expect("open unsaved");
            let draft_id = opened.drafts[0].id;
            let tab_id = opened.tabs[0].id;
            repository
                .persist_draft(
                    workspace_id,
                    draft_id,
                    request_content("Unsaved", "draft-url"),
                )
                .expect("persist draft");
            (workspace_id, draft_id, tab_id)
        };

        let mut reopened = SqliteWorkspaceRepository::open(db.path()).expect("reopen database");
        reopened.initialize().expect("initialize after restart");
        let snapshot = reopened
            .list_request_workspace(workspace_id)
            .expect("restored request workspace");

        assert_eq!(snapshot.drafts[0].id, draft_id);
        assert_eq!(snapshot.drafts[0].content.name, "Unsaved");
        assert_eq!(snapshot.tabs[0].id, tab_id);
        assert_eq!(snapshot.tabs[0].draft_id, draft_id);
    }

    #[test]
    fn queued_drafts_flush_when_request_service_drops_for_clean_shutdown() {
        let db = NamedTempFile::new().expect("temporary database");
        let workspace_id = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            repository
                .initialize()
                .expect("initialize")
                .selected_workspace_id
        };
        let draft_id = {
            let repository = SqliteWorkspaceRepository::open(db.path()).expect("open requests");
            let mut service = RequestService::new(repository);
            let opened = service
                .open_unsaved_tab(workspace_id)
                .expect("open unsaved tab");
            let draft_id = opened.drafts[0].id;
            service.queue_draft_update(
                workspace_id,
                draft_id,
                request_content("Queued shutdown", "shutdown-url"),
            );
            draft_id
        };

        let repository = SqliteWorkspaceRepository::open(db.path()).expect("reopen database");
        let snapshot = repository
            .list_request_workspace(workspace_id)
            .expect("snapshot after service drop");

        assert_eq!(snapshot.drafts[0].id, draft_id);
        assert_eq!(snapshot.drafts[0].content.name, "Queued shutdown");
        assert_eq!(snapshot.drafts[0].content.url, "shutdown-url");
    }

    #[test]
    fn only_one_tab_opens_the_same_saved_request() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let created = repository
            .create_saved_request(workspace_id, request_content("Saved", "url"))
            .expect("create saved");
        let saved_request_id = created.saved_requests[0].id;

        repository
            .open_saved_request_tab(workspace_id, saved_request_id)
            .expect("open saved once");
        let reopened = repository
            .open_saved_request_tab(workspace_id, saved_request_id)
            .expect("open saved twice");

        assert_eq!(reopened.tabs.len(), 1);
        assert_eq!(reopened.tabs[0].saved_request_id, Some(saved_request_id));
    }

    #[test]
    fn close_decisions_save_discard_and_cancel_are_persisted_by_repository_steps() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let opened = repository
            .open_unsaved_tab(workspace_id)
            .expect("open unsaved");
        let draft_id = opened.drafts[0].id;
        let tab_id = opened.tabs[0].id;

        repository
            .persist_draft(
                workspace_id,
                draft_id,
                request_content("Saved from close", "url"),
            )
            .expect("persist draft");
        let saved = repository.save_draft(workspace_id, draft_id).expect("save");
        assert_eq!(saved.saved_requests.len(), 1);

        let closed = repository.close_tab(workspace_id, tab_id).expect("close");
        assert!(closed.tabs.is_empty());
        assert!(closed.drafts.is_empty());
        assert_eq!(closed.saved_requests.len(), 1);

        let discarded_opened = repository
            .open_unsaved_tab(workspace_id)
            .expect("open discarded tab");
        let discarded_tab_id = discarded_opened.tabs[0].id;
        let discarded = repository
            .close_tab(workspace_id, discarded_tab_id)
            .expect("discard close");
        assert!(discarded.tabs.is_empty());
        assert_eq!(discarded.saved_requests.len(), 1);
    }

    fn request_content(name: &str, url: &str) -> RequestContent {
        RequestContent {
            name: name.to_owned(),
            method: "GET".to_owned(),
            url: url.to_owned(),
            body: String::new(),
            query: Vec::new(),
            headers: Vec::new(),
        }
    }
}
