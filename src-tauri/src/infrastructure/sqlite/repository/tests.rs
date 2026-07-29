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
