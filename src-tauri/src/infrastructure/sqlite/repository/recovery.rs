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
