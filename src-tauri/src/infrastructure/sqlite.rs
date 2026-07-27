use std::{path::Path, str::FromStr, time::Duration};

use rusqlite::{params, Connection, ErrorCode, OptionalExtension, Transaction};

use crate::{
    application::workspace::{
        WorkspaceError, WorkspaceRepository, WorkspaceSnapshot, WorkspaceSummary,
    },
    domain::workspace::{Workspace, WorkspaceId, WorkspaceName, DEFAULT_WORKSPACE_NAME},
};

const MIGRATIONS: &[Migration] = &[Migration {
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
}];

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

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;
    use crate::application::workspace::WorkspaceRepository;

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
}
