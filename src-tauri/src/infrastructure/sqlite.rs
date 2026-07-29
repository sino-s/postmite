use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, ErrorCode, OpenFlags, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{
    application::backup::{
        NativeBackupData, NativeBackupError, NativeBackupRepository, NativeBackupWorkspace,
    },
    application::postman_import::{
        ConvertedCollection, ConvertedPostmanImport, PostmanImportError, PostmanImportRepository,
        StoredPostmanImportRecord,
    },
    application::request::{
        CollectionLocation, ExecutionHistorySnapshot, ExecutionRecordDraft, RequestError,
        RequestRepository, RequestWorkspaceSnapshot, EXECUTION_HISTORY_RETENTION_DAYS,
        EXECUTION_HISTORY_RETENTION_LIMIT,
    },
    application::workspace::{
        WorkspaceError, WorkspaceRepository, WorkspaceSnapshot, WorkspaceSummary,
    },
    domain::{
        request::{
            BodyFilePath, BodyFileReference, CollectionFolder, CollectionId, CollectionVariable,
            CookieDraft, CookieId, CookieSameSite, Environment, EnvironmentId, EnvironmentVariable,
            ExecutionRecord, ExecutionRecordId, ExecutionRecordResponse, MultipartPart,
            OrderedField, RedirectPolicy, RequestAuth, RequestBody, RequestContent, RequestDraft,
            RequestDraftId, RequestTab, RequestTabId, SavedRequest, SavedRequestId, TlsPolicy,
            TransportPolicy, Variable, VariableValue, WorkspaceCookie,
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
    Migration {
        version: 4,
        name: "add_collection_tree_ordering",
        sql: r#"
ALTER TABLE collections ADD COLUMN parent_collection_id TEXT;
ALTER TABLE collections ADD COLUMN position INTEGER NOT NULL DEFAULT 0 CHECK (position >= 0);
ALTER TABLE saved_requests ADD COLUMN position INTEGER NOT NULL DEFAULT 0 CHECK (position >= 0);

CREATE INDEX collections_workspace_parent_position
    ON collections(workspace_id, parent_collection_id, position, created_at, id);
CREATE INDEX saved_requests_workspace_collection_position
    ON saved_requests(workspace_id, collection_id, position, created_at, id);
"#,
    },
    Migration {
        version: 5,
        name: "create_environment_variable_tables",
        sql: r#"
CREATE TABLE environments (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    UNIQUE (workspace_id, name),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE selected_environments (
    workspace_id TEXT PRIMARY KEY,
    environment_id TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (environment_id, workspace_id) REFERENCES environments(id, workspace_id)
        ON DELETE SET NULL
);

CREATE TABLE collection_variables (
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    plain_value TEXT,
    secret_ref TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (workspace_id, name),
    CHECK ((plain_value IS NOT NULL AND secret_ref IS NULL) OR (plain_value IS NULL AND secret_ref IS NOT NULL)),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE environment_variables (
    environment_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    plain_value TEXT,
    secret_ref TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (environment_id, name),
    CHECK ((plain_value IS NOT NULL AND secret_ref IS NULL) OR (plain_value IS NULL AND secret_ref IS NOT NULL)),
    FOREIGN KEY (environment_id, workspace_id) REFERENCES environments(id, workspace_id)
        ON DELETE CASCADE
);

CREATE INDEX environments_workspace_position
    ON environments(workspace_id, position, created_at, id);
CREATE INDEX environment_variables_workspace
    ON environment_variables(workspace_id, environment_id, name);
"#,
    },
    Migration {
        version: 6,
        name: "create_execution_history_tables",
        sql: r#"
CREATE TABLE execution_history_settings (
    workspace_id TEXT PRIMARY KEY,
    disabled INTEGER NOT NULL CHECK (disabled IN (0, 1)),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE execution_records (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    created_at_epoch_seconds INTEGER NOT NULL,
    pinned INTEGER NOT NULL CHECK (pinned IN (0, 1)),
    name TEXT NOT NULL CHECK (length(name) > 0),
    method TEXT NOT NULL CHECK (length(method) > 0),
    url TEXT NOT NULL,
    body TEXT NOT NULL,
    response_status INTEGER,
    response_body_preview TEXT NOT NULL,
    response_body_truncated INTEGER NOT NULL CHECK (response_body_truncated IN (0, 1)),
    response_error TEXT,
    response_duration_ms INTEGER,
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE execution_record_query_rows (
    execution_record_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (execution_record_id, row_order),
    FOREIGN KEY (execution_record_id) REFERENCES execution_records(id) ON DELETE CASCADE
);

CREATE TABLE execution_record_header_rows (
    execution_record_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (execution_record_id, row_order),
    FOREIGN KEY (execution_record_id) REFERENCES execution_records(id) ON DELETE CASCADE
);

CREATE TABLE execution_record_response_header_rows (
    execution_record_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (execution_record_id, row_order),
    FOREIGN KEY (execution_record_id) REFERENCES execution_records(id) ON DELETE CASCADE
);

CREATE INDEX execution_records_workspace_created
    ON execution_records(workspace_id, created_at_epoch_seconds DESC, id);
CREATE INDEX execution_records_workspace_unpinned_created
    ON execution_records(workspace_id, pinned, created_at_epoch_seconds, id);
"#,
    },
    Migration {
        version: 7,
        name: "create_workspace_cookie_metadata",
        sql: r#"
CREATE TABLE workspace_cookies (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    domain TEXT NOT NULL CHECK (length(domain) > 0),
    path TEXT NOT NULL CHECK (length(path) > 0),
    secure INTEGER NOT NULL CHECK (secure IN (0, 1)),
    http_only INTEGER NOT NULL CHECK (http_only IN (0, 1)),
    same_site TEXT CHECK (same_site IN ('strict', 'lax', 'none')),
    expires_at_epoch_seconds INTEGER,
    session INTEGER NOT NULL CHECK (session IN (0, 1)),
    has_value INTEGER NOT NULL CHECK (has_value IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    UNIQUE (workspace_id, name, domain, path),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX workspace_cookies_workspace_scope
    ON workspace_cookies(workspace_id, domain, path, secure, expires_at_epoch_seconds);
"#,
    },
    Migration {
        version: 8,
        name: "add_workspace_base_directory",
        sql: r#"
ALTER TABLE workspaces ADD COLUMN base_directory TEXT;
"#,
    },
    Migration {
        version: 9,
        name: "add_request_security_policy",
        sql: r#"
ALTER TABLE saved_requests ADD COLUMN auth TEXT NOT NULL DEFAULT '{"type":"NONE"}';
ALTER TABLE saved_requests ADD COLUMN redirect_policy TEXT NOT NULL DEFAULT '{"enabled":true,"maxRedirects":10}';
ALTER TABLE saved_requests ADD COLUMN tls_policy TEXT NOT NULL DEFAULT '{"verify":true,"customCaReference":null,"clientCertificateReference":null,"clientKeyReference":null}';

ALTER TABLE request_drafts ADD COLUMN auth TEXT NOT NULL DEFAULT '{"type":"NONE"}';
ALTER TABLE request_drafts ADD COLUMN redirect_policy TEXT NOT NULL DEFAULT '{"enabled":true,"maxRedirects":10}';
ALTER TABLE request_drafts ADD COLUMN tls_policy TEXT NOT NULL DEFAULT '{"verify":true,"customCaReference":null,"clientCertificateReference":null,"clientKeyReference":null}';

ALTER TABLE execution_records ADD COLUMN auth TEXT NOT NULL DEFAULT '{"type":"NONE"}';
ALTER TABLE execution_records ADD COLUMN redirect_policy TEXT NOT NULL DEFAULT '{"enabled":true,"maxRedirects":10}';
ALTER TABLE execution_records ADD COLUMN tls_policy TEXT NOT NULL DEFAULT '{"verify":true,"customCaReference":null,"clientCertificateReference":null,"clientKeyReference":null}';
"#,
    },
    Migration {
        version: 10,
        name: "add_request_transport_policy",
        sql: r#"
ALTER TABLE saved_requests ADD COLUMN transport_policy TEXT NOT NULL DEFAULT '{"proxy":{"source":"PROCESS_ENVIRONMENT","url":null,"noProxy":[]},"timeouts":{"connectMs":10000,"overallMs":300000,"idleMs":60000}}';
ALTER TABLE request_drafts ADD COLUMN transport_policy TEXT NOT NULL DEFAULT '{"proxy":{"source":"PROCESS_ENVIRONMENT","url":null,"noProxy":[]},"timeouts":{"connectMs":10000,"overallMs":300000,"idleMs":60000}}';
ALTER TABLE execution_records ADD COLUMN transport_policy TEXT NOT NULL DEFAULT '{"proxy":{"source":"PROCESS_ENVIRONMENT","url":null,"noProxy":[]},"timeouts":{"connectMs":10000,"overallMs":300000,"idleMs":60000}}';
"#,
    },
    Migration {
        version: 11,
        name: "add_cookie_secret_references",
        sql: r#"
ALTER TABLE workspace_cookies ADD COLUMN secret_ref TEXT;
"#,
    },
    Migration {
        version: 12,
        name: "create_postman_import_records",
        sql: r#"
CREATE TABLE postman_import_records (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    source_id TEXT NOT NULL CHECK (length(source_id) > 0),
    source_name TEXT NOT NULL CHECK (length(source_name) > 0),
    source_hash TEXT NOT NULL CHECK (length(source_hash) > 0),
    collection_json_sha256 TEXT NOT NULL CHECK (length(collection_json_sha256) > 0),
    environment_json_sha256 TEXT,
    warning_count INTEGER NOT NULL CHECK (warning_count >= 0),
    unsupported_count INTEGER NOT NULL CHECK (unsupported_count >= 0),
    warnings_json TEXT NOT NULL,
    unsupported_json TEXT NOT NULL,
    imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX postman_import_records_workspace_imported
    ON postman_import_records(workspace_id, imported_at DESC, id);
"#,
    },
    Migration {
        version: 13,
        name: "track_postman_import_entities",
        sql: r#"
ALTER TABLE postman_import_records ADD COLUMN collection_ids_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE postman_import_records ADD COLUMN environment_ids_json TEXT NOT NULL DEFAULT '[]';
"#,
    },
];

#[derive(Clone, Copy)]
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const PRE_MIGRATION_SNAPSHOT_RETENTION: usize = 3;
const REDACTED_RECOVERY_VALUE: &str = "excluded";
const NEWER_SCHEMA_MESSAGE: &str = "database schema is newer than this Postmite build";

pub struct SqliteWorkspaceRepository {
    connection: Connection,
    recovery: DatabaseRecoveryState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRecoveryState {
    pub mode: DatabaseRecoveryMode,
    pub reason: Option<String>,
    pub snapshots: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DatabaseRecoveryMode {
    Normal,
    Safe,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverableDatabaseExport {
    pub export_path: String,
    pub source_path: String,
    pub repaired_copy_path: String,
    pub table_count: u32,
    pub row_count: u32,
    pub redacted_value_count: u32,
}

impl SqliteWorkspaceRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        Self::open_with_migrations(path, MIGRATIONS)
    }

    fn open_with_migrations(
        path: impl AsRef<Path>,
        migrations: &[Migration],
    ) -> Result<Self, WorkspaceError> {
        let path = path.as_ref();
        let pending = match inspect_database_before_migration(path, migrations) {
            Ok(pending) => pending,
            Err(WorkspaceError::Persistence(message)) if message == NEWER_SCHEMA_MESSAGE => {
                return Err(WorkspaceError::Persistence(message));
            }
            Err(error) => {
                return Self::open_safe(path, format!("preflight failed: {error}"), Vec::new())
            }
        };
        let mut snapshots = Vec::new();
        if pending {
            snapshots = create_pre_migration_snapshot(path)?;
        }

        let connection = Connection::open(path).map_err(WorkspaceError::persistence)?;
        configure_migration_connection(&connection)?;
        if let Err(error) = apply_migrations(&connection, migrations) {
            return Self::open_safe(path, format!("migration failed: {error}"), snapshots);
        }
        configure_connection(&connection)?;
        if let Err(error) = clear_session_cookie_metadata(&connection) {
            return Self::open_safe(path, format!("startup cleanup failed: {error}"), snapshots);
        }

        Ok(Self {
            connection,
            recovery: DatabaseRecoveryState {
                mode: DatabaseRecoveryMode::Normal,
                reason: None,
                snapshots: snapshots_to_strings(snapshots),
            },
        })
    }

    fn open_safe(
        path: &Path,
        reason: String,
        snapshots: Vec<PathBuf>,
    ) -> Result<Self, WorkspaceError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(WorkspaceError::persistence)?;
        configure_readonly_connection(&connection)?;
        Ok(Self {
            connection,
            recovery: DatabaseRecoveryState {
                mode: DatabaseRecoveryMode::Safe,
                reason: Some(reason),
                snapshots: snapshots_to_strings(snapshots),
            },
        })
    }

    pub fn recovery_state(&self) -> DatabaseRecoveryState {
        self.recovery.clone()
    }

    pub fn export_recoverable_database(
        source_path: impl AsRef<Path>,
        export_path: impl AsRef<Path>,
    ) -> Result<RecoverableDatabaseExport, WorkspaceError> {
        export_recoverable_database(source_path.as_ref(), export_path.as_ref())
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

    fn set_workspace_base_directory(
        &mut self,
        id: WorkspaceId,
        base_directory: Option<String>,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let tx = self
            .connection
            .transaction()
            .map_err(WorkspaceError::persistence)?;
        ensure_workspace_exists(&tx, id)?;
        tx.execute(
            "UPDATE workspaces
             SET base_directory = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2",
            params![base_directory.as_deref(), id.to_string()],
        )
        .map_err(map_sqlite_error)?;
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
        self.open_unsaved_tab_with_content(workspace_id, RequestContent::blank())
    }

    fn open_unsaved_tab_with_content(
        &mut self,
        workspace_id: WorkspaceId,
        content: RequestContent,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;

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

    fn create_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        parent_collection_id: Option<CollectionId>,
        name: String,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        ensure_collection_parent(&tx, workspace_id, parent_collection_id)?;
        validate_collection_name(&name)?;
        let position = next_collection_position(&tx, workspace_id, parent_collection_id)?;
        tx.execute(
            "INSERT INTO collections
                (id, workspace_id, parent_collection_id, name, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                CollectionId::new().to_string(),
                workspace_id.to_string(),
                parent_collection_id.map(|id| id.to_string()),
                name.trim(),
                position,
            ],
        )
        .map_err(map_request_sqlite_error)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn select_environment(
        &mut self,
        workspace_id: WorkspaceId,
        environment_id: Option<EnvironmentId>,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        if let Some(environment_id) = environment_id {
            ensure_environment_in_workspace(&tx, workspace_id, environment_id)?;
        }
        tx.execute(
            "INSERT INTO selected_environments (workspace_id, environment_id)
             VALUES (?1, ?2)
             ON CONFLICT(workspace_id) DO UPDATE SET
                 environment_id = excluded.environment_id,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![
                workspace_id.to_string(),
                environment_id.map(|id| id.to_string())
            ],
        )
        .map_err(map_request_sqlite_error)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn rename_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
        name: String,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        validate_collection_name(&name)?;
        let changed = tx
            .execute(
                "UPDATE collections
                 SET name = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE workspace_id = ?2 AND id = ?3",
                params![
                    name.trim(),
                    workspace_id.to_string(),
                    collection_id.to_string()
                ],
            )
            .map_err(map_request_sqlite_error)?;
        if changed == 0 {
            return Err(RequestError::NotFound);
        }

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn move_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
        location: CollectionLocation,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        let folder = load_collection_folder(&tx, workspace_id, collection_id)?;
        ensure_collection_parent(&tx, workspace_id, location.collection_id)?;
        if location.collection_id == Some(collection_id)
            || collection_descends_from(&tx, workspace_id, location.collection_id, collection_id)?
        {
            return Err(RequestError::InvalidInput("collection.cycle".to_owned()));
        }

        shift_collection_position(
            &tx,
            workspace_id,
            folder.parent_collection_id,
            folder.position,
            location.collection_id,
            location.position,
        )?;
        tx.execute(
            "UPDATE collections
             SET parent_collection_id = ?1, position = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE workspace_id = ?3 AND id = ?4",
            params![
                location.collection_id.map(|id| id.to_string()),
                i64::from(location.position),
                workspace_id.to_string(),
                collection_id.to_string()
            ],
        )
        .map_err(map_request_sqlite_error)?;
        compact_collection_positions(&tx, workspace_id)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn duplicate_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        duplicate_collection_subtree(&tx, workspace_id, collection_id, None)?;
        compact_collection_positions(&tx, workspace_id)?;
        compact_saved_request_positions(&tx, workspace_id)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn delete_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        ensure_collection_in_workspace(&tx, workspace_id, collection_id)?;
        delete_collection_subtree(&tx, workspace_id, collection_id)?;
        compact_collection_positions(&tx, workspace_id)?;
        compact_saved_request_positions(&tx, workspace_id)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn move_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
        location: CollectionLocation,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        let saved_request = load_saved_request(&tx, workspace_id, saved_request_id)?;
        ensure_collection_parent(&tx, workspace_id, location.collection_id)?;
        shift_saved_request_position(
            &tx,
            workspace_id,
            saved_request.collection_id,
            saved_request.position,
            location.collection_id,
            location.position,
        )?;
        tx.execute(
            "UPDATE saved_requests
             SET collection_id = ?1, position = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE workspace_id = ?3 AND id = ?4",
            params![
                location.collection_id.map(|id| id.to_string()),
                i64::from(location.position),
                workspace_id.to_string(),
                saved_request_id.to_string(),
            ],
        )
        .map_err(map_request_sqlite_error)?;
        compact_saved_request_positions(&tx, workspace_id)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn duplicate_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        let saved_request = load_saved_request(&tx, workspace_id, saved_request_id)?;
        let location = CollectionLocation {
            collection_id: saved_request.collection_id,
            position: saved_request.position.saturating_add(1),
        };
        let mut content = saved_request.content.clone();
        content.name = format!("{} Copy", content.name);
        make_saved_request_position_space(
            &tx,
            workspace_id,
            location.collection_id,
            location.position,
        )?;
        insert_saved_request_at(
            &tx,
            workspace_id,
            SavedRequestId::new(),
            location.collection_id,
            location.position,
            &content,
        )?;
        compact_saved_request_positions(&tx, workspace_id)?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn delete_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        ensure_saved_request_in_workspace(&tx, workspace_id, saved_request_id)?;
        tx.execute(
            "DELETE FROM request_tabs
             WHERE workspace_id = ?1 AND saved_request_id = ?2",
            params![workspace_id.to_string(), saved_request_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
        tx.execute(
            "DELETE FROM request_drafts
             WHERE workspace_id = ?1 AND saved_request_id = ?2",
            params![workspace_id.to_string(), saved_request_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
        tx.execute(
            "DELETE FROM saved_requests WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id.to_string(), saved_request_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
        compact_tab_positions(&tx, workspace_id)?;
        compact_saved_request_positions(&tx, workspace_id)?;

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

    fn list_execution_history(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<ExecutionHistorySnapshot, RequestError> {
        ensure_request_workspace_exists(&self.connection, workspace_id)?;
        load_execution_history_snapshot(&self.connection, workspace_id)
    }

    fn set_execution_history_disabled(
        &mut self,
        workspace_id: WorkspaceId,
        disabled: bool,
    ) -> Result<ExecutionHistorySnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        tx.execute(
            "INSERT INTO execution_history_settings (workspace_id, disabled)
             VALUES (?1, ?2)
             ON CONFLICT(workspace_id) DO UPDATE SET
                 disabled = excluded.disabled,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![workspace_id.to_string(), bool_to_i64(disabled)],
        )
        .map_err(map_request_sqlite_error)?;
        let snapshot = load_execution_history_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn set_execution_record_pinned(
        &mut self,
        workspace_id: WorkspaceId,
        record_id: ExecutionRecordId,
        pinned: bool,
    ) -> Result<ExecutionHistorySnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        let changed = tx
            .execute(
                "UPDATE execution_records
                 SET pinned = ?1
                 WHERE workspace_id = ?2 AND id = ?3",
                params![
                    bool_to_i64(pinned),
                    workspace_id.to_string(),
                    record_id.to_string()
                ],
            )
            .map_err(map_request_sqlite_error)?;
        if changed == 0 {
            return Err(RequestError::NotFound);
        }
        cleanup_execution_records(&tx, workspace_id, None)?;
        let snapshot = load_execution_history_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn insert_execution_record(&mut self, draft: ExecutionRecordDraft) -> Result<(), RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, draft.workspace_id)?;
        if execution_history_disabled(&tx, draft.workspace_id)? {
            tx.commit().map_err(RequestError::persistence)?;
            return Ok(());
        }
        insert_execution_record(&tx, draft)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(())
    }

    fn open_execution_record_as_draft(
        &mut self,
        workspace_id: WorkspaceId,
        record_id: ExecutionRecordId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        let record = load_execution_record(&tx, workspace_id, record_id)?;
        let draft_id = RequestDraftId::new();
        insert_draft(&tx, workspace_id, draft_id, None, &record.request, true)?;
        insert_tab(
            &tx,
            workspace_id,
            RequestTabId::new(),
            None,
            draft_id,
            record.request.name,
        )?;

        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }

    fn list_cookies(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<WorkspaceCookie>, RequestError> {
        ensure_request_workspace_exists(&self.connection, workspace_id)?;
        load_workspace_cookies(&self.connection, workspace_id)
    }

    fn upsert_cookie_metadata(
        &mut self,
        draft: CookieDraft,
        has_value: bool,
        secret_reference: Option<&str>,
        now_epoch_seconds: i64,
    ) -> Result<WorkspaceCookie, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, draft.workspace_id)?;
        validate_cookie_metadata(&draft)?;
        let session = draft.expires_at_epoch_seconds.is_none();
        if let Some(expires_at) = draft.expires_at_epoch_seconds {
            if expires_at <= now_epoch_seconds {
                tx.execute(
                    "DELETE FROM workspace_cookies
                     WHERE workspace_id = ?1 AND name = ?2 AND domain = ?3 AND path = ?4",
                    params![
                        draft.workspace_id.to_string(),
                        draft.name.trim(),
                        normalize_cookie_domain(&draft.domain),
                        draft.path.as_str()
                    ],
                )
                .map_err(map_request_sqlite_error)?;
                tx.commit().map_err(RequestError::persistence)?;
                return Err(RequestError::NotFound);
            }
        }

        let id = draft.id.unwrap_or_default();
        tx.execute(
            "INSERT INTO workspace_cookies
                (id, workspace_id, name, domain, path, secure, http_only, same_site,
                 expires_at_epoch_seconds, session, has_value, secret_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(workspace_id, name, domain, path) DO UPDATE SET
                 secure = excluded.secure,
                 http_only = excluded.http_only,
                 same_site = excluded.same_site,
                 expires_at_epoch_seconds = excluded.expires_at_epoch_seconds,
                 session = excluded.session,
                 has_value = excluded.has_value,
                 secret_ref = excluded.secret_ref,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![
                id.to_string(),
                draft.workspace_id.to_string(),
                draft.name.trim(),
                normalize_cookie_domain(&draft.domain),
                draft.path.as_str(),
                bool_to_i64(draft.secure),
                bool_to_i64(draft.http_only),
                draft.same_site.map(cookie_same_site_to_sql),
                draft.expires_at_epoch_seconds,
                bool_to_i64(session),
                bool_to_i64(has_value),
                secret_reference,
            ],
        )
        .map_err(map_request_sqlite_error)?;

        let cookie = load_workspace_cookie_by_scope(
            &tx,
            draft.workspace_id,
            draft.name.trim(),
            &normalize_cookie_domain(&draft.domain),
            &draft.path,
        )?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(cookie)
    }

    fn delete_cookie(
        &mut self,
        workspace_id: WorkspaceId,
        cookie_id: CookieId,
    ) -> Result<(), RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        let changed = tx
            .execute(
                "DELETE FROM workspace_cookies WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id.to_string(), cookie_id.to_string()],
            )
            .map_err(map_request_sqlite_error)?;
        if changed == 0 {
            return Err(RequestError::NotFound);
        }
        tx.commit().map_err(RequestError::persistence)?;
        Ok(())
    }

    fn clear_cookies(&mut self, workspace_id: WorkspaceId) -> Result<(), RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        tx.execute(
            "DELETE FROM workspace_cookies WHERE workspace_id = ?1",
            params![workspace_id.to_string()],
        )
        .map_err(map_request_sqlite_error)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(())
    }

    fn cleanup_expired_cookies(
        &mut self,
        workspace_id: WorkspaceId,
        now_epoch_seconds: i64,
    ) -> Result<Vec<CookieId>, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        let removed = load_expired_cookie_ids(&tx, workspace_id, now_epoch_seconds)?;
        tx.execute(
            "DELETE FROM workspace_cookies
             WHERE workspace_id = ?1
               AND expires_at_epoch_seconds IS NOT NULL
               AND expires_at_epoch_seconds <= ?2",
            params![workspace_id.to_string(), now_epoch_seconds],
        )
        .map_err(map_request_sqlite_error)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(removed)
    }

    fn relink_body_files(
        &mut self,
        workspace_id: WorkspaceId,
        from_path: String,
        replacement: BodyFileReference,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let tx = self
            .connection
            .transaction()
            .map_err(RequestError::persistence)?;
        ensure_request_workspace_exists(&tx, workspace_id)?;
        let saved_requests = load_saved_requests(&tx, workspace_id)?;
        for mut request in saved_requests {
            if replace_body_file_reference(&mut request.content.body, &from_path, &replacement) {
                replace_saved_request_content(&tx, request.id, &request.content)?;
            }
        }
        let drafts = load_open_drafts(&tx, workspace_id)?;
        for mut draft in drafts {
            if replace_body_file_reference(&mut draft.content.body, &from_path, &replacement) {
                replace_draft_content(&tx, draft.id, &draft.content, true)?;
            }
        }
        let snapshot = load_request_snapshot(&tx, workspace_id)?;
        tx.commit().map_err(RequestError::persistence)?;
        Ok(snapshot)
    }
}

impl NativeBackupRepository for SqliteWorkspaceRepository {
    fn export_native_backup(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<NativeBackupData, NativeBackupError> {
        ensure_request_workspace_exists(&self.connection, workspace_id)
            .map_err(native_backup_request_error)?;
        let workspace = load_backup_workspace(&self.connection, workspace_id)?;
        Ok(NativeBackupData {
            workspace,
            requests: load_request_snapshot(&self.connection, workspace_id)
                .map_err(native_backup_request_error)?,
            execution_history: load_execution_history_snapshot(&self.connection, workspace_id)
                .map_err(native_backup_request_error)?,
            cookies: load_workspace_cookies(&self.connection, workspace_id)
                .map_err(native_backup_request_error)?,
        })
    }

    fn restore_native_backup(
        &mut self,
        backup: NativeBackupData,
        workspace_name: WorkspaceName,
    ) -> Result<(WorkspaceSnapshot, RequestWorkspaceSnapshot), NativeBackupError> {
        let tx = self
            .connection
            .transaction()
            .map_err(NativeBackupError::persistence)?;
        let workspace = Workspace::new(workspace_name);
        insert_workspace(&tx, &workspace).map_err(native_backup_workspace_error)?;
        tx.execute(
            "UPDATE workspaces
             SET base_directory = ?1
             WHERE id = ?2",
            params![
                backup.workspace.base_directory.as_deref(),
                workspace.id.to_string()
            ],
        )
        .map_err(native_backup_sqlite_error)?;

        let id_map = restore_backup_requests(&tx, workspace.id, backup.requests)?;
        restore_backup_execution_history(&tx, workspace.id, backup.execution_history)?;
        restore_backup_cookies(&tx, workspace.id, backup.cookies)?;
        let _ = id_map;
        select_workspace(&tx, workspace.id).map_err(native_backup_workspace_error)?;
        let workspace_snapshot = load_snapshot(&tx).map_err(native_backup_workspace_error)?;
        let request_snapshot =
            load_request_snapshot(&tx, workspace.id).map_err(native_backup_request_error)?;
        tx.commit().map_err(NativeBackupError::persistence)?;
        Ok((workspace_snapshot, request_snapshot))
    }
}

impl PostmanImportRepository for SqliteWorkspaceRepository {
    fn list_postman_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RequestWorkspaceSnapshot, PostmanImportError> {
        ensure_request_workspace_exists(&self.connection, workspace_id)
            .map_err(postman_request_error)?;
        load_request_snapshot(&self.connection, workspace_id).map_err(postman_request_error)
    }

    fn find_latest_postman_import(
        &self,
        workspace_id: WorkspaceId,
        source_id: &str,
    ) -> Result<Option<StoredPostmanImportRecord>, PostmanImportError> {
        self.connection
            .query_row(
                "SELECT id, source_id, source_name, source_hash,
                        collection_ids_json, environment_ids_json
                 FROM postman_import_records
                 WHERE workspace_id = ?1 AND source_id = ?2
                 ORDER BY imported_at DESC, id DESC
                 LIMIT 1",
                params![workspace_id.to_string(), source_id],
                postman_import_record_from_row,
            )
            .optional()
            .map_err(PostmanImportError::persistence)
    }

    fn import_postman(
        &mut self,
        import: ConvertedPostmanImport,
    ) -> Result<RequestWorkspaceSnapshot, PostmanImportError> {
        let tx = self
            .connection
            .transaction()
            .map_err(PostmanImportError::persistence)?;
        ensure_request_workspace_exists(&tx, import.workspace_id).map_err(postman_request_error)?;
        insert_converted_postman_import(&tx, &import)?;
        let snapshot =
            load_request_snapshot(&tx, import.workspace_id).map_err(postman_request_error)?;
        tx.commit().map_err(PostmanImportError::persistence)?;
        Ok(snapshot)
    }

    fn update_postman_import(
        &mut self,
        prior: &StoredPostmanImportRecord,
        import: ConvertedPostmanImport,
    ) -> Result<RequestWorkspaceSnapshot, PostmanImportError> {
        let tx = self
            .connection
            .transaction()
            .map_err(PostmanImportError::persistence)?;
        ensure_request_workspace_exists(&tx, import.workspace_id).map_err(postman_request_error)?;

        for environment_id in &prior.environment_ids {
            tx.execute(
                "DELETE FROM selected_environments
                 WHERE workspace_id = ?1 AND environment_id = ?2",
                params![import.workspace_id.to_string(), environment_id.to_string()],
            )
            .map_err(map_postman_sqlite_error)?;
            tx.execute(
                "DELETE FROM environments WHERE workspace_id = ?1 AND id = ?2",
                params![import.workspace_id.to_string(), environment_id.to_string()],
            )
            .map_err(map_postman_sqlite_error)?;
        }
        for collection_id in &prior.collection_ids {
            if collection_exists(&tx, import.workspace_id, *collection_id)
                .map_err(postman_request_error)?
            {
                delete_collection_subtree(&tx, import.workspace_id, *collection_id)
                    .map_err(postman_request_error)?;
            }
        }

        insert_converted_postman_import(&tx, &import)?;

        let snapshot =
            load_request_snapshot(&tx, import.workspace_id).map_err(postman_request_error)?;
        tx.commit().map_err(PostmanImportError::persistence)?;
        Ok(snapshot)
    }
}

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
    tx.execute(
        "INSERT INTO environment_variables
            (environment_id, workspace_id, name, plain_value, secret_ref)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            environment_id.to_string(),
            workspace_id.to_string(),
            variable.variable.name,
            plain_value,
            secret_ref,
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

fn configure_readonly_connection(connection: &Connection) -> Result<(), WorkspaceError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(WorkspaceError::persistence)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(WorkspaceError::persistence)?;
    Ok(())
}

fn configure_migration_connection(connection: &Connection) -> Result<(), WorkspaceError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(WorkspaceError::persistence)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(WorkspaceError::persistence)?;
    Ok(())
}

fn inspect_database_before_migration(
    path: &Path,
    migrations: &[Migration],
) -> Result<bool, WorkspaceError> {
    if !path.exists()
        || fs::metadata(path)
            .map_err(WorkspaceError::persistence)?
            .len()
            == 0
    {
        return Ok(false);
    }

    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(WorkspaceError::persistence)?;
    configure_readonly_connection(&connection)?;
    ensure_integrity(&connection)?;

    let latest = latest_migration_version(migrations);
    let applied = applied_migration_versions(&connection)?;
    if applied.iter().any(|version| *version > latest) {
        return Err(WorkspaceError::Persistence(NEWER_SCHEMA_MESSAGE.to_owned()));
    }

    Ok(migrations
        .iter()
        .any(|migration| !applied.contains(&migration.version)))
}

fn ensure_integrity(connection: &Connection) -> Result<(), WorkspaceError> {
    let status: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(WorkspaceError::persistence)?;
    if status == "ok" {
        Ok(())
    } else {
        Err(WorkspaceError::Persistence(format!(
            "database integrity check failed: {status}"
        )))
    }
}

fn latest_migration_version(migrations: &[Migration]) -> i64 {
    migrations
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0)
}

fn applied_migration_versions(connection: &Connection) -> Result<Vec<i64>, WorkspaceError> {
    let has_table = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(WorkspaceError::persistence)?
        .is_some();
    if !has_table {
        return Ok(Vec::new());
    }

    let mut statement = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .map_err(WorkspaceError::persistence)?;
    let versions = statement
        .query_map([], |row| row.get(0))
        .map_err(WorkspaceError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(WorkspaceError::persistence)?;
    Ok(versions)
}

fn create_pre_migration_snapshot(path: &Path) -> Result<Vec<PathBuf>, WorkspaceError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let snapshot_path = next_snapshot_path(path)?;
    fs::copy(path, &snapshot_path).map_err(WorkspaceError::persistence)?;
    rotate_pre_migration_snapshots(path)
}

fn next_snapshot_path(path: &Path) -> Result<PathBuf, WorkspaceError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| WorkspaceError::Persistence("database path is invalid".to_owned()))?;
    let now = current_epoch_millis();
    for index in 0..100_u32 {
        let candidate = parent.join(format!("{file_name}.pre-migration-{now}-{index}.snapshot"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(WorkspaceError::Persistence(
        "could not allocate migration snapshot path".to_owned(),
    ))
}

fn rotate_pre_migration_snapshots(path: &Path) -> Result<Vec<PathBuf>, WorkspaceError> {
    let mut snapshots = list_pre_migration_snapshots(path)?;
    if snapshots.len() > PRE_MIGRATION_SNAPSHOT_RETENTION {
        let remove_count = snapshots.len() - PRE_MIGRATION_SNAPSHOT_RETENTION;
        for snapshot in snapshots.iter().take(remove_count) {
            fs::remove_file(snapshot).map_err(WorkspaceError::persistence)?;
        }
        snapshots = list_pre_migration_snapshots(path)?;
    }
    Ok(snapshots)
}

fn list_pre_migration_snapshots(path: &Path) -> Result<Vec<PathBuf>, WorkspaceError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| WorkspaceError::Persistence("database path is invalid".to_owned()))?;
    let prefix = format!("{file_name}.pre-migration-");
    let mut snapshots = Vec::new();
    for entry in fs::read_dir(parent).map_err(WorkspaceError::persistence)? {
        let entry = entry.map_err(WorkspaceError::persistence)?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with(&prefix) && name.ends_with(".snapshot") {
            snapshots.push(path);
        }
    }
    snapshots.sort();
    Ok(snapshots)
}

fn snapshots_to_strings(snapshots: Vec<PathBuf>) -> Vec<String> {
    snapshots
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn current_epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn apply_migrations(
    connection: &Connection,
    migrations: &[Migration],
) -> Result<(), WorkspaceError> {
    let tx = connection
        .unchecked_transaction()
        .map_err(WorkspaceError::persistence)?;
    tx.execute_batch(
        r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
"#,
    )
    .map_err(WorkspaceError::persistence)?;

    for migration in migrations {
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
    }

    tx.commit().map_err(WorkspaceError::persistence)?;
    Ok(())
}

fn export_recoverable_database(
    source_path: &Path,
    export_path: &Path,
) -> Result<RecoverableDatabaseExport, WorkspaceError> {
    if let Some(parent) = export_path.parent() {
        fs::create_dir_all(parent).map_err(WorkspaceError::persistence)?;
    }
    let copy_path = export_path.with_extension("repair-copy.sqlite3");
    fs::copy(source_path, &copy_path).map_err(WorkspaceError::persistence)?;

    let connection = Connection::open(&copy_path).map_err(WorkspaceError::persistence)?;
    connection
        .execute_batch("PRAGMA writable_schema = OFF; VACUUM;")
        .map_err(WorkspaceError::persistence)?;
    let (tables, redacted_value_count) = recover_tables_as_json(&connection)?;
    let row_count = tables
        .values()
        .map(|rows| rows.as_array().map(|rows| rows.len() as u32).unwrap_or(0))
        .sum();
    let table_count = tables.len() as u32;
    let export_json = json!({
        "format": "postmite.recoverable-data",
        "sourcePath": source_path.to_string_lossy(),
        "repairedCopyPath": copy_path.to_string_lossy(),
        "tables": tables,
    });
    let mut file = fs::File::create(export_path).map_err(WorkspaceError::persistence)?;
    file.write_all(export_json.to_string().as_bytes())
        .map_err(WorkspaceError::persistence)?;

    Ok(RecoverableDatabaseExport {
        export_path: export_path.to_string_lossy().into_owned(),
        source_path: source_path.to_string_lossy().into_owned(),
        repaired_copy_path: copy_path.to_string_lossy().into_owned(),
        table_count,
        row_count,
        redacted_value_count,
    })
}

fn recover_tables_as_json(
    connection: &Connection,
) -> Result<(Map<String, Value>, u32), WorkspaceError> {
    let mut tables = Map::new();
    let mut redacted = 0;
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .map_err(WorkspaceError::persistence)?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(WorkspaceError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(WorkspaceError::persistence)?;

    for table in table_names {
        let columns = table_columns(connection, &table)?;
        let sql = format!("SELECT * FROM {}", quote_identifier(&table));
        let mut statement = connection
            .prepare(&sql)
            .map_err(WorkspaceError::persistence)?;
        let rows = statement
            .query_map([], |row| {
                let mut object = Map::new();
                let mut row_redacted = 0;
                for (index, column) in columns.iter().enumerate() {
                    let mut value = sqlite_value_to_json(row.get_ref(index)?);
                    if is_secret_column(column) && !matches!(value, Value::Null) {
                        value = Value::String(REDACTED_RECOVERY_VALUE.to_owned());
                        row_redacted += 1;
                    }
                    object.insert(column.clone(), value);
                }
                Ok((Value::Object(object), row_redacted))
            })
            .map_err(WorkspaceError::persistence)?;
        let mut values = Vec::new();
        for row in rows {
            let (value, row_redacted) = row.map_err(WorkspaceError::persistence)?;
            values.push(value);
            redacted += row_redacted;
        }
        tables.insert(table, Value::Array(values));
    }

    Ok((tables, redacted))
}

fn table_columns(connection: &Connection, table: &str) -> Result<Vec<String>, WorkspaceError> {
    let sql = format!("PRAGMA table_info({})", quote_string_literal(table));
    let mut statement = connection
        .prepare(&sql)
        .map_err(WorkspaceError::persistence)?;
    let columns = statement
        .query_map([], |row| row.get(1))
        .map_err(WorkspaceError::persistence)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(WorkspaceError::persistence)?;
    Ok(columns)
}

fn sqlite_value_to_json(value: rusqlite::types::ValueRef<'_>) -> Value {
    match value {
        rusqlite::types::ValueRef::Null => Value::Null,
        rusqlite::types::ValueRef::Integer(value) => Value::Number(value.into()),
        rusqlite::types::ValueRef::Real(value) => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        rusqlite::types::ValueRef::Text(value) => {
            Value::String(String::from_utf8_lossy(value).into_owned())
        }
        rusqlite::types::ValueRef::Blob(value) => {
            Value::String(format!("<{} bytes blob>", value.len()))
        }
    }
}

fn is_secret_column(column: &str) -> bool {
    matches!(
        column,
        "password"
            | "token"
            | "client_secret"
            | "secret_ref"
            | "client_key_reference"
            | "custom_ca_reference"
            | "client_certificate_reference"
    )
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn quote_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

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
    tx.execute(
        "INSERT INTO environment_variables
            (environment_id, workspace_id, name, plain_value, secret_ref)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            environment_id.to_string(),
            workspace_id.to_string(),
            name.trim(),
            plain_value,
            secret_ref,
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

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use tempfile::{NamedTempFile, TempDir};

    use super::*;
    use crate::{
        application::backup::NativeBackupRepository,
        application::postman_import::PostmanImportService,
        application::request::{RequestRepository, RequestService},
        application::workspace::WorkspaceRepository,
        domain::request::{
            CookieDraft, CookieSameSite, EnvironmentId, OrderedField, RequestBody, RequestContent,
            RequestDraftId, VariableValue,
        },
        domain::workspace::WorkspaceName,
    };

    fn repository() -> SqliteWorkspaceRepository {
        let db = NamedTempFile::new().expect("temporary database");
        SqliteWorkspaceRepository::open(db.path()).expect("open database")
    }

    fn sha256_path(path: &Path) -> String {
        let bytes = fs::read(path).expect("read database bytes");
        format!("{:x}", Sha256::digest(bytes))
    }

    fn initialize_old_database(path: &Path) {
        {
            let mut repository =
                SqliteWorkspaceRepository::open_with_migrations(path, &MIGRATIONS[..12])
                    .expect("open old database");
            repository.initialize().expect("initialize old database");
        }
        let connection = Connection::open(path).expect("open old database for checkpoint");
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint old database");
    }

    #[test]
    fn failed_migration_reopens_unchanged_database_in_safe_mode() {
        let db = NamedTempFile::new().expect("temporary database");
        initialize_old_database(db.path());
        let before = sha256_path(db.path());
        let mut destructive = MIGRATIONS[..12].to_vec();
        destructive.push(Migration {
            version: 13,
            name: "destructive_failure",
            sql: "DROP TABLE workspaces; SELECT missing_column FROM missing_table;",
        });

        let repository = SqliteWorkspaceRepository::open_with_migrations(db.path(), &destructive)
            .expect("open safe database after failed migration");

        assert_eq!(repository.recovery_state().mode, DatabaseRecoveryMode::Safe);
        assert_eq!(sha256_path(db.path()), before);
        let workspace_count: i64 = repository
            .connection()
            .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
            .expect("query safe database");
        assert_eq!(workspace_count, 1);
    }

    #[test]
    fn newer_schema_is_rejected_without_writes() {
        let db = NamedTempFile::new().expect("temporary database");
        {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            repository.initialize().expect("initialize");
            repository
                .connection()
                .execute(
                    "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                    params![latest_migration_version(MIGRATIONS) + 1, "future"],
                )
                .expect("mark future schema");
        }
        let connection = Connection::open(db.path()).expect("open database for checkpoint");
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint database");
        drop(connection);
        let before = sha256_path(db.path());

        let result = SqliteWorkspaceRepository::open(db.path());

        assert!(matches!(
            result,
            Err(WorkspaceError::Persistence(message)) if message == NEWER_SCHEMA_MESSAGE
        ));
        assert_eq!(sha256_path(db.path()), before);
    }

    #[test]
    fn pre_migration_snapshots_rotate_to_three_newest() {
        let db = NamedTempFile::new().expect("temporary database");
        initialize_old_database(db.path());
        let mut failing = MIGRATIONS[..12].to_vec();
        failing.push(Migration {
            version: 13,
            name: "always_fails",
            sql: "SELECT missing_column FROM missing_table;",
        });

        for _ in 0..5 {
            let repository = SqliteWorkspaceRepository::open_with_migrations(db.path(), &failing)
                .expect("open safe database");
            assert_eq!(repository.recovery_state().mode, DatabaseRecoveryMode::Safe);
        }

        let snapshots = list_pre_migration_snapshots(db.path()).expect("list snapshots");
        assert_eq!(snapshots.len(), 3);
    }

    #[test]
    fn recoverable_export_uses_copy_and_redacts_secret_values() {
        let db = NamedTempFile::new().expect("temporary database");
        let export_dir = TempDir::new().expect("export directory");
        let export_path = export_dir.path().join("recoverable.json");
        let workspace_id = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            let workspace_id = repository
                .initialize()
                .expect("initialize")
                .selected_workspace_id;
            repository
                .connection()
                .execute(
                    "INSERT INTO collection_variables (workspace_id, name, plain_value, secret_ref)
                     VALUES (?1, 'token', NULL, 'secret://token-value')",
                    params![workspace_id.to_string()],
                )
                .expect("insert secret reference");
            workspace_id
        };
        assert!(!workspace_id.to_string().is_empty());
        let connection = Connection::open(db.path()).expect("open database for checkpoint");
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint database");
        drop(connection);
        let before = sha256_path(db.path());

        let result =
            SqliteWorkspaceRepository::export_recoverable_database(db.path(), &export_path)
                .expect("export recoverable database");

        assert_ne!(result.repaired_copy_path, db.path().to_string_lossy());
        assert_eq!(sha256_path(db.path()), before);
        assert!(result.redacted_value_count > 0);
        let export = fs::read_to_string(export_path).expect("read export");
        assert!(!export.contains("secret://token-value"));
        assert!(export.contains(REDACTED_RECOVERY_VALUE));
    }

    #[test]
    fn recoverable_export_write_failure_preserves_source_database() {
        let db = NamedTempFile::new().expect("temporary database");
        let export_dir = TempDir::new().expect("export directory");
        {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            repository.initialize().expect("initialize");
        }
        let connection = Connection::open(db.path()).expect("open database for checkpoint");
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint database");
        drop(connection);
        let before = sha256_path(db.path());

        let result =
            SqliteWorkspaceRepository::export_recoverable_database(db.path(), export_dir.path());

        assert!(result.is_err());
        assert_eq!(sha256_path(db.path()), before);
    }

    #[test]
    fn recoverable_export_corruption_fixture_preserves_source_database() {
        let db = NamedTempFile::new().expect("temporary database");
        let export_dir = TempDir::new().expect("export directory");
        fs::write(db.path(), b"not a sqlite database").expect("write corrupt database");
        let before = sha256_path(db.path());

        let result = SqliteWorkspaceRepository::export_recoverable_database(
            db.path(),
            export_dir.path().join("recoverable.json"),
        );

        assert!(result.is_err());
        assert_eq!(sha256_path(db.path()), before);
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
    fn collection_tree_order_survives_restart() {
        let db = NamedTempFile::new().expect("temporary database");
        let workspace_id = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            let workspace_id = repository
                .initialize()
                .expect("initialize")
                .selected_workspace_id;
            let first = repository
                .create_collection_folder(workspace_id, None, "First".to_owned())
                .expect("create first")
                .collection_folders[0]
                .id;
            let snapshot = repository
                .create_collection_folder(workspace_id, None, "Second".to_owned())
                .expect("create second");
            let second = snapshot
                .collection_folders
                .iter()
                .find(|folder| folder.name == "Second")
                .expect("second folder")
                .id;
            let saved = repository
                .create_saved_request(workspace_id, request_content("Root", "root-url"))
                .expect("create root request")
                .saved_requests[0]
                .id;
            repository
                .move_collection_folder(
                    workspace_id,
                    second,
                    CollectionLocation {
                        collection_id: None,
                        position: 0,
                    },
                )
                .expect("move second before first");
            repository
                .move_saved_request(
                    workspace_id,
                    saved,
                    CollectionLocation {
                        collection_id: Some(first),
                        position: 0,
                    },
                )
                .expect("move request into first");
            workspace_id
        };

        let mut reopened = SqliteWorkspaceRepository::open(db.path()).expect("reopen database");
        reopened.initialize().expect("initialize after restart");
        let snapshot = reopened
            .list_request_workspace(workspace_id)
            .expect("load request workspace");

        assert_eq!(
            snapshot
                .collection_folders
                .iter()
                .filter(|folder| folder.parent_collection_id.is_none())
                .map(|folder| folder.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Second", "First"]
        );
        assert_eq!(snapshot.saved_requests[0].content.name, "Root");
        assert_eq!(
            snapshot.saved_requests[0].collection_id,
            Some(
                snapshot
                    .collection_folders
                    .iter()
                    .find(|folder| folder.name == "First")
                    .expect("first folder")
                    .id
            )
        );
    }

    #[test]
    fn collection_deletes_are_transactional_and_remove_descendants() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let folder_id = repository
            .create_collection_folder(workspace_id, None, "Folder".to_owned())
            .expect("create folder")
            .collection_folders[0]
            .id;
        let child_id = repository
            .create_collection_folder(workspace_id, Some(folder_id), "Child".to_owned())
            .expect("create child")
            .collection_folders
            .iter()
            .find(|folder| folder.name == "Child")
            .expect("child")
            .id;
        let request_id = repository
            .create_saved_request(workspace_id, request_content("Saved", "url"))
            .expect("create request")
            .saved_requests[0]
            .id;
        repository
            .move_saved_request(
                workspace_id,
                request_id,
                CollectionLocation {
                    collection_id: Some(child_id),
                    position: 0,
                },
            )
            .expect("move request into child");

        let snapshot = repository
            .delete_collection_folder(workspace_id, folder_id)
            .expect("delete folder tree");

        assert!(snapshot.collection_folders.is_empty());
        assert!(snapshot.saved_requests.is_empty());
        assert!(snapshot.tabs.is_empty());
    }

    #[test]
    fn moving_saved_request_rejects_cross_workspace_collection() {
        let mut repository = repository();
        let first_workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let second_workspace_id = repository
            .create_workspace(WorkspaceName::new("Second").expect("valid name"))
            .expect("create workspace")
            .selected_workspace_id;
        let request_id = repository
            .create_saved_request(first_workspace_id, request_content("Saved", "url"))
            .expect("create request")
            .saved_requests[0]
            .id;
        let other_collection_id = repository
            .create_collection_folder(second_workspace_id, None, "Other".to_owned())
            .expect("create other collection")
            .collection_folders[0]
            .id;

        let result = repository.move_saved_request(
            first_workspace_id,
            request_id,
            CollectionLocation {
                collection_id: Some(other_collection_id),
                position: 0,
            },
        );

        assert!(matches!(result, Err(RequestError::NotFound)));
        assert_eq!(
            repository
                .list_request_workspace(first_workspace_id)
                .expect("first snapshot")
                .saved_requests[0]
                .collection_id,
            None
        );
    }

    #[test]
    fn environment_selection_and_protected_values_round_trip_without_secret_value() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let environment_id = EnvironmentId::new();
        repository
            .connection()
            .execute(
                "INSERT INTO environments (id, workspace_id, name, position)
                 VALUES (?1, ?2, 'Production', 0)",
                params![environment_id.to_string(), workspace_id.to_string()],
            )
            .expect("insert environment");
        repository
            .connection()
            .execute(
                "INSERT INTO collection_variables (workspace_id, name, plain_value, secret_ref)
                 VALUES (?1, 'baseUrl', 'https://collection.example.test', NULL)",
                params![workspace_id.to_string()],
            )
            .expect("insert collection variable");
        repository
            .connection()
            .execute(
                "INSERT INTO environment_variables
                    (environment_id, workspace_id, name, plain_value, secret_ref)
                 VALUES (?1, ?2, 'token', NULL, 'secret://token-prod')",
                params![environment_id.to_string(), workspace_id.to_string()],
            )
            .expect("insert secret reference");

        let snapshot = repository
            .select_environment(workspace_id, Some(environment_id))
            .expect("select environment");

        assert!(snapshot.environments[0].is_selected);
        assert_eq!(snapshot.collection_variables[0].variable.name, "baseUrl");
        assert!(matches!(
            snapshot.environment_variables[0].variable.value,
            VariableValue::SecretReference(ref reference) if reference == "secret://token-prod"
        ));
        let leaked: i64 = repository
            .connection()
            .query_row(
                "SELECT COUNT(*)
                 FROM environment_variables
                 WHERE plain_value LIKE '%token-prod%'",
                [],
                |row| row.get(0),
            )
            .expect("inspect protected value");
        assert_eq!(leaked, 0);
    }

    #[test]
    fn postman_import_persists_supported_model_and_metadata_transactionally() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let import = converted_postman_import(workspace_id, "postman-fixture");

        let snapshot = repository.import_postman(import).expect("import postman");

        assert_eq!(snapshot.collection_folders.len(), 1);
        assert_eq!(snapshot.collection_folders[0].name, "Imported");
        assert_eq!(snapshot.saved_requests.len(), 1);
        assert_eq!(snapshot.saved_requests[0].content.name, "Imported request");
        assert_eq!(snapshot.environments.len(), 1);
        assert_eq!(snapshot.environments[0].name, "Production");
        assert!(matches!(
            snapshot.environment_variables[0].variable.value,
            VariableValue::SecretReference(ref reference) if reference == "secret://postman-token"
        ));
        let records: i64 = repository
            .connection()
            .query_row("SELECT COUNT(*) FROM postman_import_records", [], |row| {
                row.get(0)
            })
            .expect("count import records");
        assert_eq!(records, 1);
        let stored = repository
            .find_latest_postman_import(workspace_id, "postman-fixture")
            .expect("find prior import")
            .expect("prior import");
        assert_eq!(stored.collection_ids.len(), 1);
        assert_eq!(stored.environment_ids.len(), 1);
    }

    #[test]
    fn postman_reimport_update_replaces_prior_imported_entities_transactionally() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        repository
            .import_postman(converted_postman_import(workspace_id, "postman-fixture"))
            .expect("initial import");
        let prior = repository
            .find_latest_postman_import(workspace_id, "postman-fixture")
            .expect("find prior import")
            .expect("prior import");
        let old_collection_id = prior.collection_ids[0];
        let mut replacement = converted_postman_import(workspace_id, "postman-fixture");
        replacement.collections[0].name = "Updated".to_owned();
        replacement.requests[0].content.name = "Updated request".to_owned();
        replacement.source_hash = "postman-fixture-updated-hash".to_owned();

        let snapshot = repository
            .update_postman_import(&prior, replacement)
            .expect("update import");

        assert_eq!(snapshot.collection_folders.len(), 1);
        assert_eq!(snapshot.collection_folders[0].name, "Updated");
        assert_ne!(snapshot.collection_folders[0].id, old_collection_id);
        assert_eq!(snapshot.saved_requests.len(), 1);
        assert_eq!(snapshot.saved_requests[0].content.name, "Updated request");
        assert_eq!(snapshot.environments.len(), 1);
        let records: i64 = repository
            .connection()
            .query_row("SELECT COUNT(*) FROM postman_import_records", [], |row| {
                row.get(0)
            })
            .expect("count import records");
        assert_eq!(records, 2);
    }

    #[test]
    fn postman_reimport_cancel_leaves_workspace_unchanged() {
        let db = NamedTempFile::new().expect("temporary database");
        let workspace_id = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            let workspace_id = repository
                .initialize()
                .expect("initialize")
                .selected_workspace_id;
            repository
                .import_postman(converted_postman_import(workspace_id, "postman-fixture"))
                .expect("initial import");
            workspace_id
        };
        let before = {
            let repository = SqliteWorkspaceRepository::open(db.path()).expect("open for before");
            repository
                .list_request_workspace(workspace_id)
                .expect("before snapshot")
        };
        let repository = SqliteWorkspaceRepository::open(db.path()).expect("open for service");
        let service = PostmanImportService::new(
            repository,
            std::sync::Arc::new(crate::application::secrets::SessionSecretStore::new()),
        );
        let mut service = service;
        let result = service
            .reimport(crate::application::postman_import::PostmanReimportInput {
                import: crate::application::postman_import::PostmanImportInput {
                    workspace_id,
                    source_name: "Fixture".to_owned(),
                    collection_json: r#"{
                      "info": {"name": "Demo", "_postman_id": "postman-fixture", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
                      "item": []
                    }"#
                    .to_owned(),
                    environment_json: None,
                },
                decision: crate::application::postman_import::PostmanReimportDecision::Cancel,
            })
            .expect("cancel reimport");

        assert_eq!(
            result.snapshot.collection_folders,
            before.collection_folders
        );
        assert_eq!(result.snapshot.saved_requests, before.saved_requests);
        assert_eq!(result.snapshot.environments, before.environments);
    }

    #[test]
    fn postman_import_rolls_back_after_forced_persistence_failure() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        repository
            .connection()
            .execute(
                "INSERT INTO environments (id, workspace_id, name, position)
                 VALUES (?1, ?2, 'Production', 0)",
                params![EnvironmentId::new().to_string(), workspace_id.to_string()],
            )
            .expect("seed conflicting environment");
        let before = repository
            .list_request_workspace(workspace_id)
            .expect("snapshot before rollback");

        let result =
            repository.import_postman(converted_postman_import(workspace_id, "postman-conflict"));

        assert!(matches!(result, Err(PostmanImportError::InvalidInput(_))));
        let after = repository
            .list_request_workspace(workspace_id)
            .expect("snapshot after rollback");
        assert_eq!(after.collection_folders, before.collection_folders);
        assert_eq!(after.saved_requests, before.saved_requests);
        assert_eq!(after.environments, before.environments);
        assert_eq!(after.environment_variables, before.environment_variables);
        let records: i64 = repository
            .connection()
            .query_row("SELECT COUNT(*) FROM postman_import_records", [], |row| {
                row.get(0)
            })
            .expect("count import records");
        assert_eq!(records, 0);
    }

    #[test]
    fn native_backup_restores_into_new_workspace_without_secret_cookie_values() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let snapshot = repository
            .create_saved_request(
                workspace_id,
                RequestContent {
                    name: "Backed up".to_owned(),
                    method: "POST".to_owned(),
                    url: "https://example.test".to_owned(),
                    body: RequestBody::Raw {
                        content: "body".to_owned(),
                    },
                    query: vec![OrderedField {
                        enabled: true,
                        order: 0,
                        name: "a".to_owned(),
                        value: "1".to_owned(),
                    }],
                    headers: Vec::new(),
                    ..RequestContent::blank()
                },
            )
            .expect("create saved request");
        repository
            .upsert_cookie_metadata(
                CookieDraft {
                    id: None,
                    workspace_id,
                    name: "sid".to_owned(),
                    value: "cookie-value".to_owned(),
                    domain: "example.test".to_owned(),
                    path: "/".to_owned(),
                    secure: true,
                    http_only: true,
                    same_site: Some(CookieSameSite::Lax),
                    expires_at_epoch_seconds: None,
                },
                true,
                Some("secret://cookie-value"),
                1_800_000_000,
            )
            .expect("insert cookie metadata");
        let original_request_id = snapshot.saved_requests[0].id;
        let backup = repository
            .export_native_backup(workspace_id)
            .expect("export backup");

        let (workspace_snapshot, restored) = repository
            .restore_native_backup(
                backup,
                WorkspaceName::new("Restored").expect("workspace name"),
            )
            .expect("restore backup");

        assert_eq!(workspace_snapshot.workspaces.len(), 2);
        assert_eq!(restored.saved_requests.len(), 1);
        assert_eq!(restored.saved_requests[0].content.name, "Backed up");
        assert_ne!(restored.saved_requests[0].id, original_request_id);
        let restored_cookies = repository
            .list_cookies(restored.workspace_id)
            .expect("list restored cookies");
        assert_eq!(restored_cookies.len(), 1);
        assert!(!restored_cookies[0].has_value);
        assert_eq!(restored_cookies[0].secret_reference, None);
    }

    #[test]
    fn execution_history_redaction_leaves_no_known_secret_markers_in_sqlite() {
        let db = NamedTempFile::new().expect("temporary database");
        let workspace_id = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            let workspace_id = repository
                .initialize()
                .expect("initialize")
                .selected_workspace_id;
            let environment_id = EnvironmentId::new();
            repository
                .connection()
                .execute(
                    "INSERT INTO environments (id, workspace_id, name, position)
                     VALUES (?1, ?2, 'Production', 0)",
                    params![environment_id.to_string(), workspace_id.to_string()],
                )
                .expect("insert environment");
            repository
                .connection()
                .execute(
                    "INSERT INTO environment_variables
                        (environment_id, workspace_id, name, plain_value, secret_ref)
                     VALUES (?1, ?2, 'token', NULL, 'secret://history-token')",
                    params![environment_id.to_string(), workspace_id.to_string()],
                )
                .expect("insert secret reference");
            repository
                .select_environment(workspace_id, Some(environment_id))
                .expect("select environment");
            workspace_id
        };
        {
            let repository = SqliteWorkspaceRepository::open(db.path()).expect("open requests");
            let mut service = RequestService::new_for_test(repository);
            service
                .record_execution(
                    workspace_id,
                    RequestContent {
                        name: "Secret".to_owned(),
                        method: "GET".to_owned(),
                        url: "https://example.test/{{token}}".to_owned(),
                        body: RequestBody::None,
                        query: Vec::new(),
                        headers: vec![OrderedField {
                            enabled: true,
                            order: 0,
                            name: "Authorization".to_owned(),
                            value: "Bearer plain-token-marker".to_owned(),
                        }],
                        ..RequestContent::blank()
                    },
                    history_response(Some(200)),
                    1_800_000_000,
                )
                .expect("record execution");
        }

        let connection = Connection::open(db.path()).expect("inspect database");
        for table in [
            "execution_records",
            "execution_record_query_rows",
            "execution_record_header_rows",
            "execution_record_response_header_rows",
        ] {
            let sql = format!(
                "SELECT COUNT(*) FROM {table}
                 WHERE CAST(id AS TEXT) LIKE '%plain-token-marker%'
                    OR CAST(workspace_id AS TEXT) LIKE '%plain-token-marker%'"
            );
            let count: i64 = if table == "execution_records" {
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM execution_records
                         WHERE name LIKE '%plain-token-marker%'
                            OR method LIKE '%plain-token-marker%'
                            OR url LIKE '%plain-token-marker%'
                            OR body LIKE '%plain-token-marker%'
                            OR response_body_preview LIKE '%plain-token-marker%'
                            OR response_error LIKE '%plain-token-marker%'",
                        [],
                        |row| row.get(0),
                    )
                    .expect("inspect records")
            } else {
                connection
                    .query_row(
                        &format!(
                            "SELECT COUNT(*) FROM {table}
                             WHERE name LIKE '%plain-token-marker%'
                                OR value LIKE '%plain-token-marker%'"
                        ),
                        [],
                        |row| row.get(0),
                    )
                    .expect("inspect field rows")
            };
            assert_eq!(count, 0, "{table} leaked a known secret marker via {sql}");
        }
    }

    #[test]
    fn execution_history_retention_removes_old_unpinned_entries_transactionally() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        repository
            .insert_execution_record(history_draft(workspace_id, "old", 1_000_000))
            .expect("insert old");
        repository
            .insert_execution_record(history_draft(
                workspace_id,
                "new",
                1_000_000 + 31 * 24 * 60 * 60,
            ))
            .expect("insert new");

        let history = repository
            .list_execution_history(workspace_id)
            .expect("list history");

        assert_eq!(history.records.len(), 1);
        assert_eq!(history.records[0].request.name, "new");
    }

    #[test]
    fn pinned_execution_history_entries_survive_cleanup() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        repository
            .insert_execution_record(history_draft(workspace_id, "old pinned", 1_000_000))
            .expect("insert old");
        let old_id = repository
            .list_execution_history(workspace_id)
            .expect("list history")
            .records[0]
            .id;
        repository
            .set_execution_record_pinned(workspace_id, old_id, true)
            .expect("pin old");
        repository
            .insert_execution_record(history_draft(
                workspace_id,
                "new",
                1_000_000 + 31 * 24 * 60 * 60,
            ))
            .expect("insert new");

        let history = repository
            .list_execution_history(workspace_id)
            .expect("list history");

        assert_eq!(history.records.len(), 2);
        assert!(history
            .records
            .iter()
            .any(|record| record.request.name == "old pinned" && record.pinned));
    }

    #[test]
    fn execution_history_limit_keeps_latest_thousand_unpinned_entries() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        for index in 0..1002 {
            repository
                .insert_execution_record(history_draft(
                    workspace_id,
                    &format!("request-{index}"),
                    2_000_000 + index,
                ))
                .expect("insert history");
        }

        let history = repository
            .list_execution_history(workspace_id)
            .expect("list history");

        assert_eq!(history.records.len(), 1000);
        assert!(history
            .records
            .iter()
            .any(|record| record.request.name == "request-1001"));
        assert!(!history
            .records
            .iter()
            .any(|record| record.request.name == "request-0"));
    }

    #[test]
    fn disabled_execution_history_skips_new_records() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        repository
            .set_execution_history_disabled(workspace_id, true)
            .expect("disable history");
        repository
            .insert_execution_record(history_draft(workspace_id, "skipped", 1_800_000_000))
            .expect("insert skipped");

        let history = repository
            .list_execution_history(workspace_id)
            .expect("list history");

        assert!(history.disabled);
        assert!(history.records.is_empty());
    }

    #[test]
    fn cookie_metadata_is_workspace_scoped_and_values_are_not_persisted() {
        let db = NamedTempFile::new().expect("temporary database");
        let (first_workspace_id, second_workspace_id) = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            let first_workspace_id = repository
                .initialize()
                .expect("initialize")
                .selected_workspace_id;
            let second_workspace_id = repository
                .create_workspace(WorkspaceName::new("Second").expect("valid name"))
                .expect("create second workspace")
                .selected_workspace_id;
            let mut service = RequestService::new_for_test(repository);
            let first = service
                .upsert_cookie(cookie_draft(
                    first_workspace_id,
                    "sid",
                    "first-cookie-marker",
                    "example.test",
                    "/",
                    Some(1_900_000_000),
                ))
                .expect("store first cookie");
            assert!(first.cookies[0].has_value);
            assert_eq!(
                service
                    .reveal_cookie_value(first_workspace_id, first.cookies[0].id)
                    .expect("reveal first cookie"),
                "first-cookie-marker"
            );
            service
                .upsert_cookie(cookie_draft(
                    second_workspace_id,
                    "sid",
                    "second-cookie-marker",
                    "example.test",
                    "/",
                    Some(1_900_000_000),
                ))
                .expect("store second cookie");
            (first_workspace_id, second_workspace_id)
        };

        let repository = SqliteWorkspaceRepository::open(db.path()).expect("reopen database");
        let mut service = RequestService::new_for_test(repository);
        let first = service
            .list_cookies(first_workspace_id)
            .expect("list first workspace cookies");
        let second = service
            .list_cookies(second_workspace_id)
            .expect("list second workspace cookies");

        assert_eq!(first.cookies.len(), 1);
        assert_eq!(second.cookies.len(), 1);
        assert_eq!(first.cookies[0].workspace_id, first_workspace_id);
        assert_eq!(second.cookies[0].workspace_id, second_workspace_id);
        assert!(!first.cookies[0].has_value);
        assert!(service
            .reveal_cookie_value(first_workspace_id, first.cookies[0].id)
            .is_err());

        let connection = Connection::open(db.path()).expect("inspect database");
        let secret_ref: String = connection
            .query_row(
                "SELECT secret_ref FROM workspace_cookies
                 WHERE workspace_id = ?1 AND name = 'sid'",
                params![first_workspace_id.to_string()],
                |row| row.get(0),
            )
            .expect("load cookie secret reference");
        assert!(secret_ref.starts_with("secret://postmite/"));
        assert!(!secret_ref.contains("first-cookie-marker"));
        let leaked: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM workspace_cookies
                 WHERE name LIKE '%cookie-marker%'
                    OR domain LIKE '%cookie-marker%'
                    OR path LIKE '%cookie-marker%'
                    OR secret_ref LIKE '%cookie-marker%'",
                [],
                |row| row.get(0),
            )
            .expect("inspect cookies");
        assert_eq!(leaked, 0);
    }

    #[test]
    fn session_cookies_disappear_on_restart_while_persistent_metadata_remains() {
        let db = NamedTempFile::new().expect("temporary database");
        let workspace_id = {
            let mut repository = SqliteWorkspaceRepository::open(db.path()).expect("open database");
            let workspace_id = repository
                .initialize()
                .expect("initialize")
                .selected_workspace_id;
            let mut service = RequestService::new_for_test(repository);
            service
                .upsert_cookie(cookie_draft(
                    workspace_id,
                    "session",
                    "session-value",
                    "example.test",
                    "/",
                    None,
                ))
                .expect("store session cookie");
            service
                .upsert_cookie(cookie_draft(
                    workspace_id,
                    "persistent",
                    "persistent-value",
                    "example.test",
                    "/",
                    Some(1_900_000_000),
                ))
                .expect("store persistent cookie");
            workspace_id
        };

        let repository = SqliteWorkspaceRepository::open(db.path()).expect("reopen database");
        let mut service = RequestService::new_for_test(repository);
        let snapshot = service.list_cookies(workspace_id).expect("list cookies");

        assert_eq!(snapshot.cookies.len(), 1);
        assert_eq!(snapshot.cookies[0].name, "persistent");
        assert!(!snapshot.cookies[0].session);
        assert!(!snapshot.cookies[0].has_value);
    }

    #[test]
    fn opening_execution_history_as_draft_does_not_mutate_collections() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        repository
            .create_saved_request(workspace_id, request_content("Saved", "saved-url"))
            .expect("create saved request");
        repository
            .insert_execution_record(history_draft(workspace_id, "Replay", 1_800_000_000))
            .expect("insert history");
        let record_id = repository
            .list_execution_history(workspace_id)
            .expect("list history")
            .records[0]
            .id;

        let snapshot = repository
            .open_execution_record_as_draft(workspace_id, record_id)
            .expect("open history draft");

        assert_eq!(snapshot.saved_requests.len(), 1);
        assert_eq!(snapshot.saved_requests[0].content.name, "Saved");
        assert_eq!(snapshot.drafts.len(), 1);
        assert_eq!(snapshot.drafts[0].content.name, "Replay");
        assert_eq!(snapshot.drafts[0].saved_request_id, None);
        assert!(snapshot.drafts[0].is_dirty);
    }

    #[test]
    fn selected_environment_rejects_cross_workspace_environment() {
        let mut repository = repository();
        let first_workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let second_workspace_id = repository
            .create_workspace(WorkspaceName::new("Second").expect("valid name"))
            .expect("create workspace")
            .selected_workspace_id;
        let environment_id = EnvironmentId::new();
        repository
            .connection()
            .execute(
                "INSERT INTO environments (id, workspace_id, name, position)
                 VALUES (?1, ?2, 'Second Env', 0)",
                params![environment_id.to_string(), second_workspace_id.to_string()],
            )
            .expect("insert environment");

        let result = repository.select_environment(first_workspace_id, Some(environment_id));

        assert!(matches!(result, Err(RequestError::NotFound)));
    }

    #[test]
    fn bulk_relink_updates_saved_requests_and_open_drafts() {
        let mut repository = repository();
        let workspace_id = repository
            .initialize()
            .expect("initialize")
            .selected_workspace_id;
        let original = body_file_reference("old.bin");
        let replacement = body_file_reference("new.bin");
        let content = RequestContent {
            body: RequestBody::Binary {
                file: original.clone(),
            },
            ..request_content("Binary", "https://example.test/upload")
        };
        repository
            .create_saved_request(workspace_id, content)
            .expect("create saved request");
        let snapshot = repository
            .open_unsaved_tab(workspace_id)
            .expect("open unsaved tab");
        let draft_id = snapshot.drafts[0].id;
        repository
            .persist_draft(
                workspace_id,
                draft_id,
                RequestContent {
                    body: RequestBody::Multipart {
                        parts: vec![MultipartPart::File {
                            enabled: true,
                            order: 0,
                            name: "file".to_owned(),
                            file: original,
                        }],
                    },
                    ..request_content("Multipart", "https://example.test/upload")
                },
            )
            .expect("persist draft");

        let snapshot = repository
            .relink_body_files(workspace_id, "old.bin".to_owned(), replacement.clone())
            .expect("bulk relink");

        assert!(matches!(
            &snapshot.saved_requests[0].content.body,
            RequestBody::Binary { file } if file == &replacement
        ));
        assert!(matches!(
            &snapshot.drafts[0].content.body,
            RequestBody::Multipart { parts } if matches!(
                &parts[0],
                MultipartPart::File { file, .. } if file == &replacement
            )
        ));
        assert!(snapshot.drafts[0].is_dirty);
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
            body: RequestBody::Raw {
                content: "{\"ok\":true}".to_owned(),
            },
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
            ..RequestContent::blank()
        };

        let snapshot = repository
            .create_saved_request(workspace_id, content.clone())
            .expect("create saved");

        assert_eq!(snapshot.saved_requests[0].content.query, content.query);
        assert_eq!(snapshot.saved_requests[0].content.headers, content.headers);
        assert_eq!(
            snapshot.saved_requests[0].content.body,
            RequestBody::Raw {
                content: "{\"ok\":true}".to_owned()
            }
        );
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
            let mut service = RequestService::new_for_test(repository);
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
            body: RequestBody::None,
            query: Vec::new(),
            headers: Vec::new(),
            ..RequestContent::blank()
        }
    }

    fn converted_postman_import(
        workspace_id: WorkspaceId,
        source_id: &str,
    ) -> ConvertedPostmanImport {
        ConvertedPostmanImport {
            workspace_id,
            source_id: source_id.to_owned(),
            source_name: "Fixture".to_owned(),
            source_hash: format!("{source_id}-hash"),
            collection_json_sha256: format!("{source_id}-collection"),
            environment_json_sha256: Some(format!("{source_id}-environment")),
            warnings: Vec::new(),
            unsupported: Vec::new(),
            collections: vec![ConvertedCollection {
                import_index: 0,
                parent_import_index: None,
                name: "Imported".to_owned(),
            }],
            requests: vec![crate::application::postman_import::ConvertedSavedRequest {
                collection_import_index: Some(0),
                content: request_content("Imported request", "https://example.test"),
            }],
            environments: vec![crate::application::postman_import::ConvertedEnvironment {
                name: "Production".to_owned(),
                variables: vec![crate::application::postman_import::ConvertedVariable {
                    name: "token".to_owned(),
                    value: VariableValue::SecretReference("secret://postman-token".to_owned()),
                }],
            }],
        }
    }

    fn body_file_reference(path: &str) -> BodyFileReference {
        BodyFileReference {
            path: BodyFilePath::Relative {
                path: path.to_owned(),
            },
            file_name: path.to_owned(),
            size: 1,
            modified_at_epoch_seconds: Some(1),
            sha256: format!("{path}-hash"),
        }
    }

    fn history_draft(
        workspace_id: WorkspaceId,
        name: &str,
        completed_at_epoch_seconds: i64,
    ) -> ExecutionRecordDraft {
        ExecutionRecordDraft {
            workspace_id,
            content: request_content(name, "https://history.example.test"),
            response: history_response(Some(200)),
            completed_at_epoch_seconds,
        }
    }

    fn history_response(status: Option<u16>) -> ExecutionRecordResponse {
        ExecutionRecordResponse {
            status,
            headers: vec![OrderedField {
                enabled: true,
                order: 0,
                name: "content-type".to_owned(),
                value: "application/json".to_owned(),
            }],
            body_preview: "{\"ok\":true}".to_owned(),
            body_truncated: false,
            error: None,
            duration_ms: Some(12),
        }
    }

    fn cookie_draft(
        workspace_id: WorkspaceId,
        name: &str,
        value: &str,
        domain: &str,
        path: &str,
        expires_at_epoch_seconds: Option<i64>,
    ) -> CookieDraft {
        CookieDraft {
            id: None,
            workspace_id,
            name: name.to_owned(),
            value: value.to_owned(),
            domain: domain.to_owned(),
            path: path.to_owned(),
            secure: false,
            http_only: true,
            same_site: Some(CookieSameSite::Lax),
            expires_at_epoch_seconds,
        }
    }
}
