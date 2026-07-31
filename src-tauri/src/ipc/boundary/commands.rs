#[tauri::command]
pub fn list_workspaces(state: State<'_, AppState>) -> Result<WorkspaceSnapshotDto, IpcError> {
    let service = state.workspaces.lock().map_err(map_poison_error)?;
    handle_list_workspaces(service)
}

#[tauri::command]
pub fn create_workspace(
    state: State<'_, AppState>,
    input: CreateWorkspaceInput,
) -> Result<WorkspaceSnapshotDto, IpcError> {
    let service = state.workspaces.lock().map_err(map_poison_error)?;
    handle_create_workspace(service, input)
}

#[tauri::command]
pub fn rename_workspace(
    state: State<'_, AppState>,
    input: RenameWorkspaceInput,
) -> Result<WorkspaceSnapshotDto, IpcError> {
    let service = state.workspaces.lock().map_err(map_poison_error)?;
    handle_rename_workspace(service, input)
}

#[tauri::command]
pub fn set_workspace_base_directory(
    state: State<'_, AppState>,
    input: SetWorkspaceBaseDirectoryInput,
) -> Result<WorkspaceSnapshotDto, IpcError> {
    let service = state.workspaces.lock().map_err(map_poison_error)?;
    handle_set_workspace_base_directory(service, input)
}

#[tauri::command]
pub fn switch_workspace(
    state: State<'_, AppState>,
    input: WorkspaceIdInput,
) -> Result<WorkspaceSnapshotDto, IpcError> {
    let service = state.workspaces.lock().map_err(map_poison_error)?;
    handle_switch_workspace(service, input)
}

#[tauri::command]
pub fn delete_workspace(
    state: State<'_, AppState>,
    input: WorkspaceIdInput,
) -> Result<WorkspaceSnapshotDto, IpcError> {
    let service = state.workspaces.lock().map_err(map_poison_error)?;
    handle_delete_workspace(service, input)
}

#[tauri::command]
pub fn list_request_workspace(
    state: State<'_, AppState>,
    input: WorkspaceIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_list_request_workspace(service, input)
}

#[tauri::command]
pub fn open_unsaved_request_tab(
    state: State<'_, AppState>,
    input: WorkspaceIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_open_unsaved_request_tab(service, input)
}

#[tauri::command]
pub fn create_saved_request(
    state: State<'_, AppState>,
    input: CreateSavedRequestInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_create_saved_request(service, input)
}

#[tauri::command]
pub fn create_collection_folder(
    state: State<'_, AppState>,
    input: CreateCollectionFolderInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_create_collection_folder(service, input)
}

#[tauri::command]
pub fn select_environment(
    state: State<'_, AppState>,
    input: SelectEnvironmentInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_select_environment(service, input)
}

#[tauri::command]
pub fn create_environment(
    state: State<'_, AppState>,
    input: CreateEnvironmentInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_create_environment(service, input)
}

#[tauri::command]
pub fn update_environment(
    state: State<'_, AppState>,
    input: UpdateEnvironmentInput,
) -> Result<EnvironmentMutationResultDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_update_environment(service, input)
}

#[tauri::command]
pub fn delete_environment(
    state: State<'_, AppState>,
    input: EnvironmentIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_delete_environment(service, input)
}

#[tauri::command]
pub fn resolve_request_content(
    state: State<'_, AppState>,
    input: ResolveRequestContentInput,
) -> Result<ResolvedRequestContentDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_resolve_request_content(service, input)
}

#[tauri::command]
pub fn rename_collection_folder(
    state: State<'_, AppState>,
    input: RenameCollectionFolderInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_rename_collection_folder(service, input)
}

#[tauri::command]
pub fn move_collection_folder(
    state: State<'_, AppState>,
    input: MoveCollectionFolderInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_move_collection_folder(service, input)
}

#[tauri::command]
pub fn duplicate_collection_folder(
    state: State<'_, AppState>,
    input: CollectionFolderIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_duplicate_collection_folder(service, input)
}

#[tauri::command]
pub fn delete_collection_folder(
    state: State<'_, AppState>,
    input: CollectionFolderIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_delete_collection_folder(service, input)
}

#[tauri::command]
pub fn move_saved_request(
    state: State<'_, AppState>,
    input: MoveSavedRequestInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_move_saved_request(service, input)
}

#[tauri::command]
pub fn duplicate_saved_request(
    state: State<'_, AppState>,
    input: SavedRequestIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_duplicate_saved_request(service, input)
}

#[tauri::command]
pub fn delete_saved_request(
    state: State<'_, AppState>,
    input: SavedRequestIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_delete_saved_request(service, input)
}

#[tauri::command]
pub fn open_saved_request_tab(
    state: State<'_, AppState>,
    input: OpenSavedRequestTabInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_open_saved_request_tab(service, input)
}

#[tauri::command]
pub fn update_request_draft(
    state: State<'_, AppState>,
    input: UpdateRequestDraftInput,
) -> Result<(), IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_update_request_draft(service, input)
}

#[tauri::command]
pub fn flush_request_drafts(state: State<'_, AppState>) -> Result<(), IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_flush_request_drafts(service)
}

#[tauri::command]
pub fn save_request_draft(
    state: State<'_, AppState>,
    input: RequestDraftIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_save_request_draft(service, input)
}

#[tauri::command]
pub fn close_request_tab(
    state: State<'_, AppState>,
    input: CloseRequestTabInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_close_request_tab(service, input)
}

#[tauri::command]
pub fn list_execution_history(
    state: State<'_, AppState>,
    input: WorkspaceIdInput,
) -> Result<ExecutionHistorySnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_list_execution_history(service, input)
}

#[tauri::command]
pub fn set_execution_history_disabled(
    state: State<'_, AppState>,
    input: SetExecutionHistoryDisabledInput,
) -> Result<ExecutionHistorySnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_set_execution_history_disabled(service, input)
}

#[tauri::command]
pub fn set_execution_record_pinned(
    state: State<'_, AppState>,
    input: SetExecutionRecordPinnedInput,
) -> Result<ExecutionHistorySnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_set_execution_record_pinned(service, input)
}

#[tauri::command]
pub fn open_execution_record_as_draft(
    state: State<'_, AppState>,
    input: ExecutionRecordIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_open_execution_record_as_draft(service, input)
}

#[tauri::command]
pub fn list_cookies(
    state: State<'_, AppState>,
    input: WorkspaceIdInput,
) -> Result<CookieJarSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_list_cookies(service, input)
}

#[tauri::command]
pub fn upsert_cookie(
    state: State<'_, AppState>,
    input: UpsertCookieInput,
) -> Result<CookieJarSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_upsert_cookie(service, input)
}

#[tauri::command]
pub fn delete_cookie(
    state: State<'_, AppState>,
    input: CookieIdInput,
) -> Result<CookieJarSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_delete_cookie(service, input)
}

#[tauri::command]
pub fn clear_cookies(
    state: State<'_, AppState>,
    input: WorkspaceIdInput,
) -> Result<CookieJarSnapshotDto, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_clear_cookies(service, input)
}

#[tauri::command]
pub fn reveal_cookie_value(
    state: State<'_, AppState>,
    input: CookieIdInput,
) -> Result<RevealCookieValueOutput, IpcError> {
    let service = state.requests.lock().map_err(map_poison_error)?;
    handle_reveal_cookie_value(service, input)
}

#[tauri::command]
pub fn describe_body_file(
    state: State<'_, AppState>,
    input: DescribeBodyFileInput,
) -> Result<BodyFileReferenceDto, IpcError> {
    handle_describe_body_file(state, input)
}

#[tauri::command]
pub fn relink_body_files(
    state: State<'_, AppState>,
    input: RelinkBodyFilesInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    handle_relink_body_files(state, input)
}

#[tauri::command]
pub fn preview_postman_import(
    state: State<'_, AppState>,
    input: PostmanImportInput,
) -> Result<PostmanImportPreviewDto, IpcError> {
    let service = state.postman_imports.lock().map_err(map_poison_error)?;
    handle_preview_postman_import(service, input)
}

#[tauri::command]
pub fn import_postman(
    state: State<'_, AppState>,
    input: PostmanImportInput,
) -> Result<PostmanImportResultDto, IpcError> {
    let service = state.postman_imports.lock().map_err(map_poison_error)?;
    handle_import_postman(service, input)
}

#[tauri::command]
pub fn export_postman(
    state: State<'_, AppState>,
    input: PostmanExportInput,
) -> Result<PostmanExportResultDto, IpcError> {
    let service = state.postman_imports.lock().map_err(map_poison_error)?;
    handle_export_postman(service, input)
}

#[tauri::command]
pub fn preview_postman_reimport(
    state: State<'_, AppState>,
    input: PostmanImportInput,
) -> Result<PostmanReimportPreviewDto, IpcError> {
    let service = state.postman_imports.lock().map_err(map_poison_error)?;
    handle_preview_postman_reimport(service, input)
}

#[tauri::command]
pub fn reimport_postman(
    state: State<'_, AppState>,
    input: PostmanReimportInput,
) -> Result<PostmanReimportResultDto, IpcError> {
    let service = state.postman_imports.lock().map_err(map_poison_error)?;
    handle_reimport_postman(service, input)
}

#[tauri::command]
pub fn export_native_backup(
    state: State<'_, AppState>,
    input: NativeBackupExportInput,
) -> Result<NativeBackupExportResultDto, IpcError> {
    let service = state.native_backups.lock().map_err(map_poison_error)?;
    handle_export_native_backup(service, input)
}

#[tauri::command]
pub fn preview_native_backup_restore(
    state: State<'_, AppState>,
    input: NativeBackupRestorePreviewInput,
) -> Result<NativeBackupRestorePreviewDto, IpcError> {
    let service = state.native_backups.lock().map_err(map_poison_error)?;
    handle_preview_native_backup_restore(service, input)
}

#[tauri::command]
pub fn restore_native_backup(
    state: State<'_, AppState>,
    input: NativeBackupRestoreInput,
) -> Result<NativeBackupRestoreResultDto, IpcError> {
    let mut service = state.native_backups.lock().map_err(map_poison_error)?;
    handle_restore_native_backup(&mut service, input)
}

#[tauri::command]
pub fn get_database_recovery_state(
    state: State<'_, AppState>,
) -> Result<DatabaseRecoveryStateDto, IpcError> {
    Ok(DatabaseRecoveryStateDto::from(
        state.database_recovery.clone(),
    ))
}

#[tauri::command]
pub fn export_recoverable_database(
    input: RecoverableDatabaseExportInput,
) -> Result<RecoverableDatabaseExportDto, IpcError> {
    SqliteWorkspaceRepository::export_recoverable_database(input.source_path, input.export_path)
        .map(RecoverableDatabaseExportDto::from)
        .map_err(|error| BoundaryError::Workspace(error).into())
}

#[tauri::command]
pub fn get_diagnostic_bundle_preview(
    state: State<'_, AppState>,
) -> Result<DiagnosticBundlePreviewDto, IpcError> {
    state
        .diagnostics
        .preview_bundle()
        .map(DiagnosticBundlePreviewDto::from)
        .map_err(IpcError::from)
}

#[tauri::command]
pub fn set_diagnostic_debug_logging(
    state: State<'_, AppState>,
    input: DiagnosticDebugLoggingInput,
) -> Result<DiagnosticDebugLoggingStatusDto, IpcError> {
    let started_at = Instant::now();
    let status = if input.enabled {
        state
            .diagnostics
            .set_debug_logging(input.duration_minutes.unwrap_or(15))
    } else {
        state.diagnostics.disable_debug_logging()
    }
    .map_err(IpcError::from)?;
    state
        .diagnostics
        .record_command("diagnostics", "debug.updated", started_at.elapsed());
    Ok(DiagnosticDebugLoggingStatusDto::from(status))
}

#[tauri::command]
pub fn record_frontend_execution_trace(
    state: State<'_, AppState>,
    input: FrontendExecutionTraceInput,
) -> Result<(), IpcError> {
    let execution_id = parse_execution_id(&input.execution_id)?;
    state
        .diagnostics
        .record_execution_stage(execution_id, input.stage.code(), input.sequence);
    Ok(())
}

#[tauri::command]
pub fn export_diagnostic_bundle(
    state: State<'_, AppState>,
    input: DiagnosticBundleExportInput,
) -> Result<DiagnosticBundleExportDto, IpcError> {
    let started_at = Instant::now();
    let result = state
        .diagnostics
        .export_bundle(&input.bundle_path)
        .map_err(IpcError::from)?;
    state
        .diagnostics
        .record_command("diagnostics", "bundle.exported", started_at.elapsed());
    Ok(DiagnosticBundleExportDto::from(result))
}

#[tauri::command]
pub async fn check_for_update() -> Result<UpdateCheckResultDto, IpcError> {
    crate::application::update::check_for_update(
        crate::application::update::DEFAULT_UPDATE_CHECK_URL,
        env!("CARGO_PKG_VERSION"),
    )
    .await
    .map(UpdateCheckResultDto::from)
    .map_err(|_| IpcError {
        code: IpcErrorCode::StateUnavailable,
        message: "Update checking is currently unavailable.".to_owned(),
        details: None,
        retryable: true,
    })
}

#[tauri::command]
pub fn preview_curl_import(input: CurlImportInput) -> Result<CurlImportPreviewDto, IpcError> {
    let input = ApplicationCurlImportInput::try_from(input)?;
    CurlService::preview(&input)
        .map(CurlImportPreviewDto::from)
        .map_err(IpcError::from)
}

#[tauri::command]
pub fn import_curl_as_draft(
    state: State<'_, AppState>,
    input: CurlImportInput,
) -> Result<CurlImportResultDto, IpcError> {
    let input = ApplicationCurlImportInput::try_from(input)?;
    let preview = CurlService::preview(&input)?;
    let mut requests = state.requests.lock().map_err(map_poison_error)?;
    let snapshot = CurlService::import_as_draft(&mut requests, input)?;
    Ok(CurlImportResultDto {
        preview: CurlImportPreviewDto::from(preview),
        snapshot: RequestWorkspaceSnapshotDto::from(snapshot),
    })
}

#[tauri::command]
pub fn generate_curl(
    state: State<'_, AppState>,
    input: CurlGenerateInput,
) -> Result<CurlGenerateResultDto, IpcError> {
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let environment_id = parse_optional_environment_id(input.environment_id)?;
    let content = RequestContent::from(input.content);
    let application_input = if input.include_secrets {
        let requests = state.requests.lock().map_err(map_poison_error)?;
        let content = requests.materialize_request_content_for_curl(
            workspace_id,
            environment_id,
            content,
        )?;
        ApplicationCurlGenerateInput {
            content,
            resolved: None,
            include_secrets: true,
        }
    } else {
        let resolved = input
            .resolved
            .map(|resolved| resolved_request_content_from_dto(resolved, &content));
        ApplicationCurlGenerateInput {
            content,
            resolved,
            include_secrets: false,
        }
    };
    CurlService::generate(application_input)
        .map(CurlGenerateResultDto::from)
        .map_err(IpcError::from)
}

#[tauri::command]
pub fn start_request_execution(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartRequestExecutionInput,
) -> Result<StartRequestExecutionOutput, IpcError> {
    handle_start_request_execution(app, state, input)
}

#[tauri::command]
pub fn cancel_request_execution(
    state: State<'_, AppState>,
    input: CancelRequestExecutionInput,
) -> Result<CancelRequestExecutionOutput, IpcError> {
    handle_cancel_request_execution(state, input)
}

#[tauri::command]
pub fn save_response_file(
    input: SaveResponseFileInput,
) -> Result<SaveResponseFileOutput, IpcError> {
    handle_save_response_file(input)
}

#[tauri::command]
pub async fn start_oauth_authorization(
    state: State<'_, AppState>,
    input: StartOAuthAuthorizationInput,
) -> Result<OAuthAuthorizationResultDto, IpcError> {
    handle_start_oauth_authorization(state, input).await
}

#[tauri::command]
pub fn cancel_oauth_authorization(
    state: State<'_, AppState>,
    input: CancelOAuthAuthorizationInput,
) -> Result<CancelOAuthAuthorizationOutput, IpcError> {
    handle_cancel_oauth_authorization(state, input)
}
