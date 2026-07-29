
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
