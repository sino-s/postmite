pub fn handle_list_workspaces<R>(
    service: MutexGuard<'_, WorkspaceService<R>>,
) -> Result<WorkspaceSnapshotDto, IpcError>
where
    R: crate::application::workspace::WorkspaceRepository,
{
    service
        .list_workspaces()
        .map(WorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Workspace(error).into())
}

pub fn handle_create_workspace<R>(
    mut service: MutexGuard<'_, WorkspaceService<R>>,
    input: CreateWorkspaceInput,
) -> Result<WorkspaceSnapshotDto, IpcError>
where
    R: crate::application::workspace::WorkspaceRepository,
{
    service
        .create_workspace(input.name)
        .map(WorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Workspace(error).into())
}

pub fn handle_rename_workspace<R>(
    mut service: MutexGuard<'_, WorkspaceService<R>>,
    input: RenameWorkspaceInput,
) -> Result<WorkspaceSnapshotDto, IpcError>
where
    R: crate::application::workspace::WorkspaceRepository,
{
    let id = parse_workspace_id(&input.workspace_id)?;
    service
        .rename_workspace(id, input.name)
        .map(WorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Workspace(error).into())
}

pub fn handle_set_workspace_base_directory<R>(
    mut service: MutexGuard<'_, WorkspaceService<R>>,
    input: SetWorkspaceBaseDirectoryInput,
) -> Result<WorkspaceSnapshotDto, IpcError>
where
    R: crate::application::workspace::WorkspaceRepository,
{
    let id = parse_workspace_id(&input.workspace_id)?;
    service
        .set_workspace_base_directory(id, input.base_directory)
        .map(WorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Workspace(error).into())
}

pub fn handle_switch_workspace<R>(
    mut service: MutexGuard<'_, WorkspaceService<R>>,
    input: WorkspaceIdInput,
) -> Result<WorkspaceSnapshotDto, IpcError>
where
    R: crate::application::workspace::WorkspaceRepository,
{
    let id = parse_workspace_id(&input.workspace_id)?;
    service
        .switch_workspace(id)
        .map(WorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Workspace(error).into())
}

pub fn handle_delete_workspace<R>(
    mut service: MutexGuard<'_, WorkspaceService<R>>,
    input: WorkspaceIdInput,
) -> Result<WorkspaceSnapshotDto, IpcError>
where
    R: crate::application::workspace::WorkspaceRepository,
{
    let id = parse_workspace_id(&input.workspace_id)?;
    service
        .delete_workspace(id)
        .map(WorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Workspace(error).into())
}

pub fn handle_list_request_workspace<R>(
    service: MutexGuard<'_, RequestService<R>>,
    input: WorkspaceIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .list_request_workspace(workspace_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_open_unsaved_request_tab<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: WorkspaceIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .open_unsaved_tab(workspace_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_create_saved_request<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: CreateSavedRequestInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .create_saved_request(workspace_id, RequestContent::from(input.content))
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_create_collection_folder<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: CreateCollectionFolderInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let parent_collection_id = parse_optional_collection_id(input.parent_collection_id)?;
    service
        .create_collection_folder(workspace_id, parent_collection_id, input.name)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_select_environment<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: SelectEnvironmentInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let environment_id = parse_optional_environment_id(input.environment_id)?;
    service
        .select_environment(workspace_id, environment_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_create_environment<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: CreateEnvironmentInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .create_environment(workspace_id, input.name)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_update_environment<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: UpdateEnvironmentInput,
) -> Result<EnvironmentMutationResultDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let environment_id = parse_environment_id(&input.environment_id)?;
    service
        .update_environment(
            workspace_id,
            environment_id,
            input.name,
            input.variables
                .into_iter()
                .map(EnvironmentVariableDraft::from)
                .collect(),
        )
        .map(EnvironmentMutationResultDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_delete_environment<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: EnvironmentIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let environment_id = parse_environment_id(&input.environment_id)?;
    service
        .delete_environment(workspace_id, environment_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_resolve_request_content<R>(
    service: MutexGuard<'_, RequestService<R>>,
    input: ResolveRequestContentInput,
) -> Result<ResolvedRequestContentDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .resolve_request_content(workspace_id, &RequestContent::from(input.content))
        .map(ResolvedRequestContentDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_rename_collection_folder<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: RenameCollectionFolderInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let collection_id = parse_collection_id(&input.collection_id)?;
    service
        .rename_collection_folder(workspace_id, collection_id, input.name)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_move_collection_folder<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: MoveCollectionFolderInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let collection_id = parse_collection_id(&input.collection_id)?;
    service
        .move_collection_folder(
            workspace_id,
            collection_id,
            CollectionLocation::try_from(input.location)?,
        )
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_duplicate_collection_folder<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: CollectionFolderIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let collection_id = parse_collection_id(&input.collection_id)?;
    service
        .duplicate_collection_folder(workspace_id, collection_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_delete_collection_folder<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: CollectionFolderIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let collection_id = parse_collection_id(&input.collection_id)?;
    service
        .delete_collection_folder(workspace_id, collection_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_move_saved_request<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: MoveSavedRequestInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let saved_request_id = parse_saved_request_id(&input.saved_request_id)?;
    service
        .move_saved_request(
            workspace_id,
            saved_request_id,
            CollectionLocation::try_from(input.location)?,
        )
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_duplicate_saved_request<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: SavedRequestIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let saved_request_id = parse_saved_request_id(&input.saved_request_id)?;
    service
        .duplicate_saved_request(workspace_id, saved_request_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_delete_saved_request<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: SavedRequestIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let saved_request_id = parse_saved_request_id(&input.saved_request_id)?;
    service
        .delete_saved_request(workspace_id, saved_request_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_open_saved_request_tab<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: OpenSavedRequestTabInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let saved_request_id = parse_saved_request_id(&input.saved_request_id)?;
    service
        .open_saved_request_tab(workspace_id, saved_request_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_update_request_draft<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: UpdateRequestDraftInput,
) -> Result<(), IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let draft_id = parse_request_draft_id(&input.draft_id)?;
    service.queue_draft_update(workspace_id, draft_id, RequestContent::from(input.content));
    Ok(())
}

pub fn handle_flush_request_drafts<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
) -> Result<(), IpcError>
where
    R: RequestRepository,
{
    service
        .flush_pending_drafts()
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_save_request_draft<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: RequestDraftIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let draft_id = parse_request_draft_id(&input.draft_id)?;
    service
        .save_draft(workspace_id, draft_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_close_request_tab<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: CloseRequestTabInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let tab_id = parse_request_tab_id(&input.tab_id)?;
    service
        .close_tab(workspace_id, tab_id, CloseTabDecision::from(input.decision))
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_list_execution_history<R>(
    service: MutexGuard<'_, RequestService<R>>,
    input: WorkspaceIdInput,
) -> Result<ExecutionHistorySnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .list_execution_history(workspace_id)
        .map(ExecutionHistorySnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_set_execution_history_disabled<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: SetExecutionHistoryDisabledInput,
) -> Result<ExecutionHistorySnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .set_execution_history_disabled(workspace_id, input.disabled)
        .map(ExecutionHistorySnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_set_execution_record_pinned<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: SetExecutionRecordPinnedInput,
) -> Result<ExecutionHistorySnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let record_id = parse_execution_record_id(&input.record_id)?;
    service
        .set_execution_record_pinned(workspace_id, record_id, input.pinned)
        .map(ExecutionHistorySnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_open_execution_record_as_draft<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: ExecutionRecordIdInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let record_id = parse_execution_record_id(&input.record_id)?;
    service
        .open_execution_record_as_draft(workspace_id, record_id)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_list_cookies<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: WorkspaceIdInput,
) -> Result<CookieJarSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .list_cookies(workspace_id)
        .map(CookieJarSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_upsert_cookie<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: UpsertCookieInput,
) -> Result<CookieJarSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let draft = CookieDraft::try_from(input)?;
    service
        .upsert_cookie(draft)
        .map(CookieJarSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_delete_cookie<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: CookieIdInput,
) -> Result<CookieJarSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let cookie_id = parse_cookie_id(&input.cookie_id)?;
    service
        .delete_cookie(workspace_id, cookie_id)
        .map(CookieJarSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_clear_cookies<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: WorkspaceIdInput,
) -> Result<CookieJarSnapshotDto, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    service
        .clear_cookies(workspace_id)
        .map(CookieJarSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_reveal_cookie_value<R>(
    mut service: MutexGuard<'_, RequestService<R>>,
    input: CookieIdInput,
) -> Result<RevealCookieValueOutput, IpcError>
where
    R: RequestRepository,
{
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let cookie_id = parse_cookie_id(&input.cookie_id)?;
    service
        .reveal_cookie_value(workspace_id, cookie_id)
        .map(|value| RevealCookieValueOutput { value })
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_describe_body_file(
    state: State<'_, AppState>,
    input: DescribeBodyFileInput,
) -> Result<BodyFileReferenceDto, IpcError> {
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let path = PathBuf::from(&input.path);
    if !path.is_absolute() {
        return Err(IpcError::from(BoundaryError::Request(
            RequestError::InvalidInput("body.file.path.absoluteRequired".to_owned()),
        )));
    }
    let workspace_base_directory = {
        let workspaces = state.workspaces.lock().map_err(map_poison_error)?;
        workspaces
            .list_workspaces()
            .map_err(|error| IpcError::from(BoundaryError::Workspace(error)))?
            .workspaces
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .and_then(|workspace| workspace.base_directory)
    };
    describe_body_file_reference(&path, workspace_base_directory.as_deref())
        .map(BodyFileReferenceDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_relink_body_files(
    state: State<'_, AppState>,
    input: RelinkBodyFilesInput,
) -> Result<RequestWorkspaceSnapshotDto, IpcError> {
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let replacement_path = PathBuf::from(&input.replacement_path);
    if !replacement_path.is_absolute() {
        return Err(IpcError::from(BoundaryError::Request(
            RequestError::InvalidInput("body.file.path.absoluteRequired".to_owned()),
        )));
    }
    let workspace_base_directory = {
        let workspaces = state.workspaces.lock().map_err(map_poison_error)?;
        workspaces
            .list_workspaces()
            .map_err(|error| IpcError::from(BoundaryError::Workspace(error)))?
            .workspaces
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .and_then(|workspace| workspace.base_directory)
    };
    let replacement =
        describe_body_file_reference(&replacement_path, workspace_base_directory.as_deref())
            .map_err(|error| IpcError::from(BoundaryError::Request(error)))?;
    let mut requests = state.requests.lock().map_err(map_poison_error)?;
    requests
        .relink_body_files(workspace_id, input.from_path, replacement)
        .map(RequestWorkspaceSnapshotDto::from)
        .map_err(|error| BoundaryError::Request(error).into())
}

pub fn handle_preview_postman_import<R>(
    service: MutexGuard<'_, PostmanImportService<R>>,
    input: PostmanImportInput,
) -> Result<PostmanImportPreviewDto, IpcError>
where
    R: PostmanImportRepository,
{
    let input = ApplicationPostmanImportInput::try_from(input)?;
    service
        .preview(&input)
        .map(PostmanImportPreviewDto::from)
        .map_err(IpcError::from)
}

pub fn handle_import_postman<R>(
    mut service: MutexGuard<'_, PostmanImportService<R>>,
    input: PostmanImportInput,
) -> Result<PostmanImportResultDto, IpcError>
where
    R: PostmanImportRepository,
{
    let input = ApplicationPostmanImportInput::try_from(input)?;
    service
        .import(input)
        .map(PostmanImportResultDto::from)
        .map_err(IpcError::from)
}

pub fn handle_export_postman<R>(
    service: MutexGuard<'_, PostmanImportService<R>>,
    input: PostmanExportInput,
) -> Result<PostmanExportResultDto, IpcError>
where
    R: PostmanImportRepository,
{
    let input = ApplicationPostmanExportInput::try_from(input)?;
    service
        .export(&input)
        .map(PostmanExportResultDto::from)
        .map_err(IpcError::from)
}

pub fn handle_preview_postman_reimport<R>(
    service: MutexGuard<'_, PostmanImportService<R>>,
    input: PostmanImportInput,
) -> Result<PostmanReimportPreviewDto, IpcError>
where
    R: PostmanImportRepository,
{
    let input = ApplicationPostmanImportInput::try_from(input)?;
    service
        .preview_reimport(&input)
        .map(PostmanReimportPreviewDto::from)
        .map_err(IpcError::from)
}

pub fn handle_reimport_postman<R>(
    mut service: MutexGuard<'_, PostmanImportService<R>>,
    input: PostmanReimportInput,
) -> Result<PostmanReimportResultDto, IpcError>
where
    R: PostmanImportRepository,
{
    let input = ApplicationPostmanReimportInput::try_from(input)?;
    service
        .reimport(input)
        .map(PostmanReimportResultDto::from)
        .map_err(IpcError::from)
}

pub fn handle_export_native_backup<R>(
    service: MutexGuard<'_, NativeBackupService<R>>,
    input: NativeBackupExportInput,
) -> Result<NativeBackupExportResultDto, IpcError>
where
    R: NativeBackupRepository,
{
    let input = ApplicationNativeBackupExportInput::try_from(input)?;
    service
        .export(input)
        .map(NativeBackupExportResultDto::from)
        .map_err(|error| BoundaryError::NativeBackup(error).into())
}

pub fn handle_preview_native_backup_restore<R>(
    service: MutexGuard<'_, NativeBackupService<R>>,
    input: NativeBackupRestorePreviewInput,
) -> Result<NativeBackupRestorePreviewDto, IpcError>
where
    R: NativeBackupRepository,
{
    service
        .preview_restore(ApplicationNativeBackupRestorePreviewInput {
            backup_path: input.backup_path,
        })
        .map(NativeBackupRestorePreviewDto::from)
        .map_err(|error| BoundaryError::NativeBackup(error).into())
}

pub fn handle_restore_native_backup<R>(
    service: &mut NativeBackupService<R>,
    input: NativeBackupRestoreInput,
) -> Result<NativeBackupRestoreResultDto, IpcError>
where
    R: NativeBackupRepository,
{
    service
        .restore(ApplicationNativeBackupRestoreInput {
            backup_path: input.backup_path,
            workspace_name: input.workspace_name,
            body_files_directory: input.body_files_directory,
        })
        .map(NativeBackupRestoreResultDto::from)
        .map_err(|error| BoundaryError::NativeBackup(error).into())
}

pub fn handle_start_request_execution(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartRequestExecutionInput,
) -> Result<StartRequestExecutionOutput, IpcError> {
    let workspace_id = parse_workspace_id(&input.workspace_id)?;
    let draft_id = parse_request_draft_id(&input.draft_id)?;
    let execution_id = parse_execution_id(&input.execution_id)?;
    state
        .diagnostics
        .record_execution_stage(execution_id, "ipc.start.received", None);
    let workspace_base_directory = {
        let workspaces = state.workspaces.lock().map_err(map_poison_error)?;
        workspaces
            .list_workspaces()
            .map_err(|error| IpcError::from(BoundaryError::Workspace(error)))?
            .workspaces
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .and_then(|workspace| workspace.base_directory)
    };
    let (content, environment_id) = {
        let mut requests = state.requests.lock().map_err(map_poison_error)?;
        let content = requests
            .materialize_request_content(workspace_id, RequestContent::from(input.content))
            .map_err(|error| IpcError::from(BoundaryError::Request(error)))?;
        let content = requests
            .attach_matching_cookies(workspace_id, content)
            .map_err(|error| IpcError::from(BoundaryError::Request(error)))?;
        let environment_id = requests
            .selected_environment_id(workspace_id)
            .map_err(|error| IpcError::from(BoundaryError::Request(error)))?;
        (content, environment_id)
    };
    let request = ExecutionRequest {
        draft_id,
        workspace_base_directory,
        content: content.clone(),
    };
    let request_url_for_cookie_capture = content.url.clone();
    let observer = Arc::new(std::sync::Mutex::new(ExecutionHistoryObserver::new(
        workspace_id,
        content,
        Instant::now(),
    )));
    let observer_for_sink = Arc::clone(&observer);
    let initial_events = Arc::new(std::sync::Mutex::new(InitialExecutionEvents::open()));
    let initial_events_for_sink = Arc::clone(&initial_events);
    let app_for_sink = app.clone();
    let sink = Arc::new(move |event: ExecutionEvent| {
        let sequence = event.sequence;
        let diagnostic_stage = execution_event_diagnostic_stage(&event.kind);
        let event_dto = ExecutionEventDto::from(event.clone());
        if let ExecutionEventKind::ResponseHeaders { headers, .. } = &event.kind {
            let app_state = app_for_sink.state::<AppState>();
            if let Ok(mut requests) = app_state.requests.lock() {
                let response_headers = headers
                    .iter()
                    .enumerate()
                    .map(|(order, header)| OrderedField {
                        enabled: true,
                        order: u32::try_from(order).unwrap_or(u32::MAX),
                        name: header.name.clone(),
                        value: header.value.clone(),
                    })
                    .collect::<Vec<_>>();
                let _ = requests.capture_set_cookie_headers(
                    workspace_id,
                    &request_url_for_cookie_capture,
                    &response_headers,
                );
            };
        }
        if let Ok(mut observer) = observer_for_sink.lock() {
            if let Some(record) = observer.observe(&event) {
                let app_state = app_for_sink.state::<AppState>();
                let result = if let Ok(mut requests) = app_state.requests.lock() {
                    let _ = requests.record_execution(
                        record.workspace_id,
                        record.content,
                        record.response,
                        record.completed_at_epoch_seconds,
                    );
                    Ok(())
                } else {
                    Err(())
                };
                let _ = result;
            }
        }
        if let Ok(mut initial_events) = initial_events_for_sink.lock() {
            initial_events.push(event_dto.clone());
        }
        let app_state = app_for_sink.state::<AppState>();
        if let Some(stage) = diagnostic_stage {
            app_state
                .diagnostics
                .record_execution_stage(execution_id, stage, Some(sequence));
        }
        let emit_stage = if app_for_sink
            .emit(REQUEST_EXECUTION_EVENT, event_dto)
            .is_ok()
        {
            "event.emit.succeeded"
        } else {
            "event.emit.failed"
        };
        app_state.diagnostics.record_execution_stage(
            execution_id,
            emit_stage,
            Some(sequence),
        );
    });

    let oauth = Arc::clone(&state.oauth);
    let secrets = Arc::clone(&state.secrets);
    let app_for_queue = app.clone();
    let app_for_running = app.clone();
    let result = state
        .executions
        .start_with_id_observed(
            execution_id,
            request,
            sink,
            move |execution_id| {
                app_for_queue
                    .state::<AppState>()
                    .diagnostics
                    .record_execution_stage(execution_id, "coordinator.queued", None);
            },
            move |execution_id| {
                app_for_running
                    .state::<AppState>()
                    .diagnostics
                    .record_execution_stage(execution_id, "coordinator.running", None);
            },
            move |execution_id, request, cancellation, coordinator, sink| {
                let oauth = Arc::clone(&oauth);
                let secrets = Arc::clone(&secrets);
                async move {
                    let content = oauth
                        .apply_client_credentials_token(
                            workspace_id,
                            environment_id,
                            request.content,
                            secrets,
                        )
                        .await;
                    let content = match content {
                        Ok(content) => content,
                        Err(error) => {
                            if let Some(event) = coordinator.record_event(
                                execution_id,
                                ExecutionEventKind::Failed {
                                    message: IpcError::from(error).message,
                                },
                            ) {
                                sink(event);
                            }
                            return;
                        }
                    };
                    crate::infrastructure::http::run_http_execution(
                        execution_id,
                        ExecutionRequest { content, ..request },
                        cancellation,
                        coordinator,
                        sink,
                    )
                    .await;
                }
            },
        )
        .map_err(|error| IpcError::from(BoundaryError::Execution(error)))?;
    let initial_events = initial_events
        .lock()
        .map(|mut events| events.close())
        .unwrap_or_default();
    state
        .diagnostics
        .record_execution_stage(execution_id, "ipc.start.returned", None);
    Ok(StartRequestExecutionOutput {
        execution_id: result.execution_id.to_string(),
        initial_events,
    })
}

fn execution_event_diagnostic_stage(kind: &ExecutionEventKind) -> Option<&'static str> {
    match kind {
        ExecutionEventKind::Started { .. } => Some("http.started"),
        ExecutionEventKind::ResponseHeaders { .. } => Some("http.response-headers"),
        ExecutionEventKind::Completed { .. } => Some("http.completed"),
        ExecutionEventKind::Failed { .. } => Some("http.failed"),
        ExecutionEventKind::Cancelled => Some("http.cancelled"),
        ExecutionEventKind::Redirected { .. }
        | ExecutionEventKind::UploadProgress { .. }
        | ExecutionEventKind::DownloadProgress { .. } => None,
    }
}

struct InitialExecutionEvents {
    open: bool,
    events: Vec<ExecutionEventDto>,
}

impl InitialExecutionEvents {
    fn open() -> Self {
        Self {
            open: true,
            events: Vec::new(),
        }
    }

    fn push(&mut self, event: ExecutionEventDto) {
        if self.open {
            self.events.push(event);
        }
    }

    fn close(&mut self) -> Vec<ExecutionEventDto> {
        self.open = false;
        self.events.clone()
    }
}

struct ExecutionHistoryObserver {
    workspace_id: WorkspaceId,
    content: RequestContent,
    response_headers: Vec<OrderedField>,
    started_at: Instant,
    recorded: bool,
}

struct ObservedExecutionRecord {
    workspace_id: WorkspaceId,
    content: RequestContent,
    response: ExecutionRecordResponse,
    completed_at_epoch_seconds: i64,
}

impl ExecutionHistoryObserver {
    fn new(workspace_id: WorkspaceId, content: RequestContent, started_at: Instant) -> Self {
        Self {
            workspace_id,
            content,
            response_headers: Vec::new(),
            started_at,
            recorded: false,
        }
    }

    fn observe(&mut self, event: &ExecutionEvent) -> Option<ObservedExecutionRecord> {
        match &event.kind {
            ExecutionEventKind::ResponseHeaders { headers, .. } => {
                self.response_headers = headers
                    .iter()
                    .enumerate()
                    .map(|(order, header)| OrderedField {
                        enabled: true,
                        order: u32::try_from(order).unwrap_or(u32::MAX),
                        name: header.name.clone(),
                        value: header.value.clone(),
                    })
                    .collect();
                None
            }
            ExecutionEventKind::Completed {
                status,
                body_preview,
                body_truncated,
                ..
            } => self.record(Some(*status), body_preview.clone(), *body_truncated, None),
            ExecutionEventKind::Failed { message } => {
                self.record(None, String::new(), false, Some(message.clone()))
            }
            ExecutionEventKind::Cancelled => {
                self.record(None, String::new(), false, Some("cancelled".to_owned()))
            }
            _ => None,
        }
    }

    fn record(
        &mut self,
        status: Option<u16>,
        body_preview: String,
        body_truncated: bool,
        error: Option<String>,
    ) -> Option<ObservedExecutionRecord> {
        if self.recorded {
            return None;
        }
        self.recorded = true;
        Some(ObservedExecutionRecord {
            workspace_id: self.workspace_id,
            content: self.content.clone(),
            response: ExecutionRecordResponse {
                status,
                headers: self.response_headers.clone(),
                body_preview,
                body_truncated,
                error,
                duration_ms: Some(self.started_at.elapsed().as_millis() as u64),
            },
            completed_at_epoch_seconds: current_epoch_seconds(),
        })
    }
}

fn current_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn describe_body_file_reference(
    path: &Path,
    base_directory: Option<&str>,
) -> Result<BodyFileReference, RequestError> {
    let metadata = std::fs::metadata(path).map_err(RequestError::persistence)?;
    if !metadata.is_file() {
        return Err(RequestError::InvalidInput(
            "body.file.path.notFile".to_owned(),
        ));
    }
    let canonical_path = path.canonicalize().map_err(RequestError::persistence)?;
    let file_name = canonical_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RequestError::InvalidInput("body.file.name.invalid".to_owned()))?
        .to_owned();
    let modified_at_epoch_seconds = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_secs()).ok());
    let path = match base_directory {
        Some(base_directory) => {
            let base = PathBuf::from(base_directory)
                .canonicalize()
                .map_err(RequestError::persistence)?;
            if let Ok(relative) = canonical_path.strip_prefix(&base) {
                let relative = relative
                    .to_str()
                    .ok_or_else(|| {
                        RequestError::InvalidInput("body.file.relative.invalid".to_owned())
                    })?
                    .to_owned();
                if path_has_unsafe_components(&relative) {
                    return Err(RequestError::InvalidInput(
                        "body.file.relative.invalid".to_owned(),
                    ));
                }
                BodyFilePath::Relative { path: relative }
            } else {
                BodyFilePath::Absolute {
                    path: canonical_path.to_string_lossy().into_owned(),
                }
            }
        }
        None => BodyFilePath::Absolute {
            path: canonical_path.to_string_lossy().into_owned(),
        },
    };

    Ok(BodyFileReference {
        path,
        file_name,
        size: metadata.len(),
        modified_at_epoch_seconds,
        sha256: sha256_file(&canonical_path)?,
    })
}

fn sha256_file(path: &Path) -> Result<String, RequestError> {
    let mut file = File::open(path).map_err(RequestError::persistence)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(RequestError::persistence)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn path_has_unsafe_components(path: &str) -> bool {
    let path = Path::new(path);
    path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

pub fn handle_cancel_request_execution(
    state: State<'_, AppState>,
    input: CancelRequestExecutionInput,
) -> Result<CancelRequestExecutionOutput, IpcError> {
    let execution_id = parse_execution_id(&input.execution_id)?;
    state
        .executions
        .cancel(execution_id)
        .map(CancelRequestExecutionOutput::from)
        .map_err(|error| BoundaryError::Execution(error).into())
}

pub fn handle_save_response_file(
    input: SaveResponseFileInput,
) -> Result<SaveResponseFileOutput, IpcError> {
    if input.source_path.trim().is_empty() || input.destination_path.trim().is_empty() {
        return Err(IpcError {
            code: IpcErrorCode::InvalidInput,
            message: "Response file path is invalid.".to_owned(),
            details: Some("responseFile".to_owned()),
            retryable: false,
        });
    }

    let byte_count = crate::infrastructure::http::save_response_file(
        Path::new(&input.source_path),
        Path::new(&input.destination_path),
        SystemTime::now(),
    )
    .map_err(map_http_response_file_error)?;

    Ok(SaveResponseFileOutput {
        destination_path: input.destination_path,
        byte_count,
    })
}

fn map_http_response_file_error(
    error: crate::infrastructure::http::HttpExecutionError,
) -> IpcError {
    let details = error.safe_message();
    match error {
        crate::infrastructure::http::HttpExecutionError::InvalidInput(_) => IpcError {
            code: IpcErrorCode::InvalidInput,
            message: "Response file path is invalid.".to_owned(),
            details: Some(details),
            retryable: false,
        },
        _ => IpcError {
            code: IpcErrorCode::PersistenceUnavailable,
            message: "Response file could not be saved.".to_owned(),
            details: Some(details),
            retryable: true,
        },
    }
}

pub async fn handle_start_oauth_authorization(
    state: State<'_, AppState>,
    input: StartOAuthAuthorizationInput,
) -> Result<OAuthAuthorizationResultDto, IpcError> {
    let request = StartOAuthAuthorizationRequest::try_from(input)?;
    state
        .oauth
        .start(request)
        .await
        .map(OAuthAuthorizationResultDto::from)
        .map_err(|error| BoundaryError::OAuth(error).into())
}

pub fn handle_cancel_oauth_authorization(
    state: State<'_, AppState>,
    input: CancelOAuthAuthorizationInput,
) -> Result<CancelOAuthAuthorizationOutput, IpcError> {
    let flow_id = parse_oauth_flow_id(&input.flow_id)?;
    state
        .oauth
        .cancel(flow_id)
        .map(CancelOAuthAuthorizationOutput::from)
        .map_err(|error| BoundaryError::OAuth(error).into())
}
