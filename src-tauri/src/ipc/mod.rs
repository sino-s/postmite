//! Typed Tauri IPC boundary.

use std::{
    str::FromStr,
    sync::Arc,
    sync::{MutexGuard, PoisonError},
};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use ts_rs::{Config, TS};

use crate::{
    application::execution::{
        CancelExecutionResult, ExecutionError, ExecutionEvent, ExecutionEventKind, ExecutionHeader,
        ExecutionId, ExecutionRequest, StartExecutionResult,
    },
    application::request::{
        CloseTabDecision, RequestError, RequestRepository, RequestService, RequestWorkspaceSnapshot,
    },
    application::workspace::{WorkspaceError, WorkspaceService, WorkspaceSnapshot},
    domain::{
        request::{
            OrderedField, RequestContent, RequestDraft, RequestDraftId, RequestTab, RequestTabId,
            SavedRequest, SavedRequestId,
        },
        workspace::{WorkspaceId, WorkspaceNameError},
    },
    AppState,
};

pub const LIST_WORKSPACES_COMMAND: &str = "list_workspaces";
pub const CREATE_WORKSPACE_COMMAND: &str = "create_workspace";
pub const RENAME_WORKSPACE_COMMAND: &str = "rename_workspace";
pub const SWITCH_WORKSPACE_COMMAND: &str = "switch_workspace";
pub const DELETE_WORKSPACE_COMMAND: &str = "delete_workspace";
pub const LIST_REQUEST_WORKSPACE_COMMAND: &str = "list_request_workspace";
pub const OPEN_UNSAVED_REQUEST_TAB_COMMAND: &str = "open_unsaved_request_tab";
pub const CREATE_SAVED_REQUEST_COMMAND: &str = "create_saved_request";
pub const OPEN_SAVED_REQUEST_TAB_COMMAND: &str = "open_saved_request_tab";
pub const UPDATE_REQUEST_DRAFT_COMMAND: &str = "update_request_draft";
pub const FLUSH_REQUEST_DRAFTS_COMMAND: &str = "flush_request_drafts";
pub const SAVE_REQUEST_DRAFT_COMMAND: &str = "save_request_draft";
pub const CLOSE_REQUEST_TAB_COMMAND: &str = "close_request_tab";
pub const START_REQUEST_EXECUTION_COMMAND: &str = "start_request_execution";
pub const CANCEL_REQUEST_EXECUTION_COMMAND: &str = "cancel_request_execution";
pub const REQUEST_EXECUTION_EVENT: &str = "request_execution_event";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummaryDto {
    pub id: String,
    pub name: String,
    pub is_selected: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshotDto {
    pub selected_workspace_id: String,
    pub workspaces: Vec<WorkspaceSummaryDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceInput {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameWorkspaceInput {
    pub workspace_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdInput {
    pub workspace_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OrderedFieldDto {
    pub enabled: bool,
    pub order: u32,
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestContentDto {
    pub name: String,
    pub method: String,
    pub url: String,
    pub body: String,
    pub query: Vec<OrderedFieldDto>,
    pub headers: Vec<OrderedFieldDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SavedRequestDto {
    pub id: String,
    pub workspace_id: String,
    pub collection_id: Option<String>,
    pub content: RequestContentDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestDraftDto {
    pub id: String,
    pub workspace_id: String,
    pub saved_request_id: Option<String>,
    pub content: RequestContentDto,
    pub is_dirty: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestTabDto {
    pub id: String,
    pub workspace_id: String,
    pub saved_request_id: Option<String>,
    pub draft_id: String,
    pub position: u32,
    pub title: String,
    pub is_active: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestWorkspaceSnapshotDto {
    pub workspace_id: String,
    pub saved_requests: Vec<SavedRequestDto>,
    pub drafts: Vec<RequestDraftDto>,
    pub tabs: Vec<RequestTabDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateSavedRequestInput {
    pub workspace_id: String,
    pub content: RequestContentDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OpenSavedRequestTabInput {
    pub workspace_id: String,
    pub saved_request_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequestDraftInput {
    pub workspace_id: String,
    pub draft_id: String,
    pub content: RequestContentDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RequestDraftIdInput {
    pub workspace_id: String,
    pub draft_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CloseTabDecisionDto {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloseRequestTabInput {
    pub workspace_id: String,
    pub tab_id: String,
    pub decision: CloseTabDecisionDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartRequestExecutionInput {
    pub workspace_id: String,
    pub draft_id: String,
    pub content: RequestContentDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartRequestExecutionOutput {
    pub execution_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequestExecutionInput {
    pub execution_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequestExecutionOutput {
    pub execution_id: String,
    pub cancelled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEventDto {
    pub execution_id: String,
    pub sequence: u64,
    pub kind: ExecutionEventKindDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum ExecutionEventKindDto {
    Started {
        method: String,
        url: String,
    },
    UploadProgress {
        sent_bytes: u64,
        total_bytes: u64,
    },
    ResponseHeaders {
        status: u16,
        headers: Vec<ExecutionHeaderDto>,
    },
    DownloadProgress {
        received_bytes: u64,
        total_bytes: Option<u64>,
    },
    Completed {
        status: u16,
        body_preview: String,
        body_truncated: bool,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionHeaderDto {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpcErrorCode {
    InvalidInput,
    WorkspaceNotFound,
    WorkspaceAlreadyExists,
    CannotDeleteLastWorkspace,
    RequestNotFound,
    SavedRequestAlreadyOpen,
    PersistenceUnavailable,
    StateUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
    pub details: Option<String>,
    pub retryable: bool,
}

#[derive(Debug)]
pub enum BoundaryError {
    Workspace(WorkspaceError),
    Request(RequestError),
    InvalidWorkspaceId,
    InvalidSavedRequestId,
    InvalidRequestDraftId,
    InvalidRequestTabId,
    InvalidExecutionId,
    Execution(ExecutionError),
    StateUnavailable,
}

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

pub fn handle_start_request_execution(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: StartRequestExecutionInput,
) -> Result<StartRequestExecutionOutput, IpcError> {
    let _workspace_id = parse_workspace_id(&input.workspace_id)?;
    let draft_id = parse_request_draft_id(&input.draft_id)?;
    let request = ExecutionRequest {
        draft_id,
        content: RequestContent::from(input.content),
    };
    let sink = Arc::new(move |event: ExecutionEvent| {
        let _ = app.emit(REQUEST_EXECUTION_EVENT, ExecutionEventDto::from(event));
    });

    state
        .executions
        .start(
            request,
            sink,
            crate::infrastructure::http::run_http_execution,
        )
        .map(StartRequestExecutionOutput::from)
        .map_err(|error| BoundaryError::Execution(error).into())
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

pub fn render_contract() -> Result<String, ts_rs::ExportError> {
    let cfg = Config::new();
    let mut contract = String::from(
        "// This file is generated by `pnpm ipc:generate`. Do not edit it by hand.\n\n",
    );

    for generated in [
        IpcErrorCode::export_to_string(&cfg)?,
        IpcError::export_to_string(&cfg)?,
        WorkspaceSummaryDto::export_to_string(&cfg)?,
        WorkspaceSnapshotDto::export_to_string(&cfg)?,
        CreateWorkspaceInput::export_to_string(&cfg)?,
        RenameWorkspaceInput::export_to_string(&cfg)?,
        WorkspaceIdInput::export_to_string(&cfg)?,
        OrderedFieldDto::export_to_string(&cfg)?,
        RequestContentDto::export_to_string(&cfg)?,
        SavedRequestDto::export_to_string(&cfg)?,
        RequestDraftDto::export_to_string(&cfg)?,
        RequestTabDto::export_to_string(&cfg)?,
        RequestWorkspaceSnapshotDto::export_to_string(&cfg)?,
        CreateSavedRequestInput::export_to_string(&cfg)?,
        OpenSavedRequestTabInput::export_to_string(&cfg)?,
        UpdateRequestDraftInput::export_to_string(&cfg)?,
        RequestDraftIdInput::export_to_string(&cfg)?,
        CloseTabDecisionDto::export_to_string(&cfg)?,
        CloseRequestTabInput::export_to_string(&cfg)?,
        StartRequestExecutionInput::export_to_string(&cfg)?,
        StartRequestExecutionOutput::export_to_string(&cfg)?,
        CancelRequestExecutionInput::export_to_string(&cfg)?,
        CancelRequestExecutionOutput::export_to_string(&cfg)?,
        ExecutionEventDto::export_to_string(&cfg)?,
        ExecutionEventKindDto::export_to_string(&cfg)?,
        ExecutionHeaderDto::export_to_string(&cfg)?,
    ] {
        let generated_without_imports = generated
            .lines()
            .filter(|line| !line.starts_with("import type "))
            .collect::<Vec<_>>()
            .join("\n");
        contract.push_str(generated_without_imports.trim());
        contract.push_str("\n\n");
    }

    contract.push_str(
        "export type WorkspaceCommandContracts = {\n\
         \tlist_workspaces: {\n\
         \t\tinput: undefined;\n\
         \t\toutput: WorkspaceSnapshotDto;\n\
         \t};\n\
         \tcreate_workspace: {\n\
         \t\tinput: CreateWorkspaceInput;\n\
         \t\toutput: WorkspaceSnapshotDto;\n\
         \t};\n\
         \trename_workspace: {\n\
         \t\tinput: RenameWorkspaceInput;\n\
         \t\toutput: WorkspaceSnapshotDto;\n\
         \t};\n\
         \tswitch_workspace: {\n\
         \t\tinput: WorkspaceIdInput;\n\
         \t\toutput: WorkspaceSnapshotDto;\n\
         \t};\n\
         \tdelete_workspace: {\n\
         \t\tinput: WorkspaceIdInput;\n\
         \t\toutput: WorkspaceSnapshotDto;\n\
         \t};\n\
         };\n",
    );
    contract.push_str(
        "\nexport type RequestCommandContracts = {\n\
         \tlist_request_workspace: {\n\
         \t\tinput: WorkspaceIdInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \topen_unsaved_request_tab: {\n\
         \t\tinput: WorkspaceIdInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tcreate_saved_request: {\n\
         \t\tinput: CreateSavedRequestInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \topen_saved_request_tab: {\n\
         \t\tinput: OpenSavedRequestTabInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tupdate_request_draft: {\n\
         \t\tinput: UpdateRequestDraftInput;\n\
         \t\toutput: undefined;\n\
         \t};\n\
         \tflush_request_drafts: {\n\
         \t\tinput: undefined;\n\
         \t\toutput: undefined;\n\
         \t};\n\
         \tsave_request_draft: {\n\
         \t\tinput: RequestDraftIdInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tclose_request_tab: {\n\
         \t\tinput: CloseRequestTabInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tstart_request_execution: {\n\
         \t\tinput: StartRequestExecutionInput;\n\
         \t\toutput: StartRequestExecutionOutput;\n\
         \t};\n\
         \tcancel_request_execution: {\n\
         \t\tinput: CancelRequestExecutionInput;\n\
         \t\toutput: CancelRequestExecutionOutput;\n\
         \t};\n\
         };\n\n\
         export type IpcCommandContracts = WorkspaceCommandContracts & RequestCommandContracts;\n",
    );

    Ok(contract)
}

fn parse_workspace_id(value: &str) -> Result<WorkspaceId, IpcError> {
    WorkspaceId::from_str(value).map_err(|_| BoundaryError::InvalidWorkspaceId.into())
}

fn parse_saved_request_id(value: &str) -> Result<SavedRequestId, IpcError> {
    SavedRequestId::from_str(value).map_err(|_| BoundaryError::InvalidSavedRequestId.into())
}

fn parse_request_draft_id(value: &str) -> Result<RequestDraftId, IpcError> {
    RequestDraftId::from_str(value).map_err(|_| BoundaryError::InvalidRequestDraftId.into())
}

fn parse_request_tab_id(value: &str) -> Result<RequestTabId, IpcError> {
    RequestTabId::from_str(value).map_err(|_| BoundaryError::InvalidRequestTabId.into())
}

fn parse_execution_id(value: &str) -> Result<ExecutionId, IpcError> {
    ExecutionId::from_str(value).map_err(|_| BoundaryError::InvalidExecutionId.into())
}

fn map_poison_error<T>(_error: PoisonError<T>) -> IpcError {
    BoundaryError::StateUnavailable.into()
}

impl From<WorkspaceSnapshot> for WorkspaceSnapshotDto {
    fn from(snapshot: WorkspaceSnapshot) -> Self {
        Self {
            selected_workspace_id: snapshot.selected_workspace_id.to_string(),
            workspaces: snapshot
                .workspaces
                .into_iter()
                .map(|workspace| WorkspaceSummaryDto {
                    id: workspace.id.to_string(),
                    name: workspace.name.to_string(),
                    is_selected: workspace.is_selected,
                })
                .collect(),
        }
    }
}

impl From<RequestWorkspaceSnapshot> for RequestWorkspaceSnapshotDto {
    fn from(snapshot: RequestWorkspaceSnapshot) -> Self {
        Self {
            workspace_id: snapshot.workspace_id.to_string(),
            saved_requests: snapshot
                .saved_requests
                .into_iter()
                .map(SavedRequestDto::from)
                .collect(),
            drafts: snapshot
                .drafts
                .into_iter()
                .map(RequestDraftDto::from)
                .collect(),
            tabs: snapshot.tabs.into_iter().map(RequestTabDto::from).collect(),
        }
    }
}

impl From<SavedRequest> for SavedRequestDto {
    fn from(request: SavedRequest) -> Self {
        Self {
            id: request.id.to_string(),
            workspace_id: request.workspace_id.to_string(),
            collection_id: request.collection_id.map(|id| id.to_string()),
            content: RequestContentDto::from(request.content),
        }
    }
}

impl From<RequestDraft> for RequestDraftDto {
    fn from(draft: RequestDraft) -> Self {
        Self {
            id: draft.id.to_string(),
            workspace_id: draft.workspace_id.to_string(),
            saved_request_id: draft.saved_request_id.map(|id| id.to_string()),
            content: RequestContentDto::from(draft.content),
            is_dirty: draft.is_dirty,
        }
    }
}

impl From<RequestTab> for RequestTabDto {
    fn from(tab: RequestTab) -> Self {
        Self {
            id: tab.id.to_string(),
            workspace_id: tab.workspace_id.to_string(),
            saved_request_id: tab.saved_request_id.map(|id| id.to_string()),
            draft_id: tab.draft_id.to_string(),
            position: tab.position,
            title: tab.title,
            is_active: tab.is_active,
        }
    }
}

impl From<RequestContent> for RequestContentDto {
    fn from(content: RequestContent) -> Self {
        Self {
            name: content.name,
            method: content.method,
            url: content.url,
            body: content.body,
            query: content
                .query
                .into_iter()
                .map(OrderedFieldDto::from)
                .collect(),
            headers: content
                .headers
                .into_iter()
                .map(OrderedFieldDto::from)
                .collect(),
        }
    }
}

impl From<RequestContentDto> for RequestContent {
    fn from(content: RequestContentDto) -> Self {
        Self {
            name: content.name,
            method: content.method,
            url: content.url,
            body: content.body,
            query: content.query.into_iter().map(OrderedField::from).collect(),
            headers: content
                .headers
                .into_iter()
                .map(OrderedField::from)
                .collect(),
        }
    }
}

impl From<OrderedField> for OrderedFieldDto {
    fn from(field: OrderedField) -> Self {
        Self {
            enabled: field.enabled,
            order: field.order,
            name: field.name,
            value: field.value,
        }
    }
}

impl From<OrderedFieldDto> for OrderedField {
    fn from(field: OrderedFieldDto) -> Self {
        Self {
            enabled: field.enabled,
            order: field.order,
            name: field.name,
            value: field.value,
        }
    }
}

impl From<CloseTabDecisionDto> for CloseTabDecision {
    fn from(decision: CloseTabDecisionDto) -> Self {
        match decision {
            CloseTabDecisionDto::Save => Self::Save,
            CloseTabDecisionDto::Discard => Self::Discard,
            CloseTabDecisionDto::Cancel => Self::Cancel,
        }
    }
}

impl From<StartExecutionResult> for StartRequestExecutionOutput {
    fn from(result: StartExecutionResult) -> Self {
        Self {
            execution_id: result.execution_id.to_string(),
        }
    }
}

impl From<CancelExecutionResult> for CancelRequestExecutionOutput {
    fn from(result: CancelExecutionResult) -> Self {
        Self {
            execution_id: result.execution_id.to_string(),
            cancelled: result.cancelled,
        }
    }
}

impl From<ExecutionEvent> for ExecutionEventDto {
    fn from(event: ExecutionEvent) -> Self {
        Self {
            execution_id: event.execution_id.to_string(),
            sequence: event.sequence,
            kind: ExecutionEventKindDto::from(event.kind),
        }
    }
}

impl From<ExecutionEventKind> for ExecutionEventKindDto {
    fn from(kind: ExecutionEventKind) -> Self {
        match kind {
            ExecutionEventKind::Started { method, url } => Self::Started { method, url },
            ExecutionEventKind::UploadProgress {
                sent_bytes,
                total_bytes,
            } => Self::UploadProgress {
                sent_bytes,
                total_bytes,
            },
            ExecutionEventKind::ResponseHeaders { status, headers } => Self::ResponseHeaders {
                status,
                headers: headers.into_iter().map(ExecutionHeaderDto::from).collect(),
            },
            ExecutionEventKind::DownloadProgress {
                received_bytes,
                total_bytes,
            } => Self::DownloadProgress {
                received_bytes,
                total_bytes,
            },
            ExecutionEventKind::Completed {
                status,
                body_preview,
                body_truncated,
            } => Self::Completed {
                status,
                body_preview,
                body_truncated,
            },
            ExecutionEventKind::Failed { message } => Self::Failed { message },
            ExecutionEventKind::Cancelled => Self::Cancelled,
        }
    }
}

impl From<ExecutionHeader> for ExecutionHeaderDto {
    fn from(header: ExecutionHeader) -> Self {
        Self {
            name: header.name,
            value: header.value,
        }
    }
}

impl From<BoundaryError> for IpcError {
    fn from(error: BoundaryError) -> Self {
        match error {
            BoundaryError::Workspace(error) => error.into(),
            BoundaryError::Request(error) => error.into(),
            BoundaryError::Execution(error) => error.into(),
            BoundaryError::InvalidWorkspaceId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Workspace id is invalid.".to_owned(),
                details: Some("workspaceId".to_owned()),
                retryable: false,
            },
            BoundaryError::InvalidSavedRequestId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Saved request id is invalid.".to_owned(),
                details: Some("savedRequestId".to_owned()),
                retryable: false,
            },
            BoundaryError::InvalidRequestDraftId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Request draft id is invalid.".to_owned(),
                details: Some("draftId".to_owned()),
                retryable: false,
            },
            BoundaryError::InvalidRequestTabId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Request tab id is invalid.".to_owned(),
                details: Some("tabId".to_owned()),
                retryable: false,
            },
            BoundaryError::InvalidExecutionId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Execution id is invalid.".to_owned(),
                details: Some("executionId".to_owned()),
                retryable: false,
            },
            BoundaryError::StateUnavailable => Self {
                code: IpcErrorCode::StateUnavailable,
                message: "Workspace state is temporarily unavailable.".to_owned(),
                details: None,
                retryable: true,
            },
        }
    }
}

impl From<ExecutionError> for IpcError {
    fn from(error: ExecutionError) -> Self {
        match error {
            ExecutionError::InvalidInput(detail) => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Execution input is invalid.".to_owned(),
                details: Some(detail),
                retryable: false,
            },
            ExecutionError::StateUnavailable => Self {
                code: IpcErrorCode::StateUnavailable,
                message: "Execution state is temporarily unavailable.".to_owned(),
                details: None,
                retryable: true,
            },
        }
    }
}

impl From<RequestError> for IpcError {
    fn from(error: RequestError) -> Self {
        match error {
            RequestError::WorkspaceNotFound => Self {
                code: IpcErrorCode::WorkspaceNotFound,
                message: "Workspace was not found.".to_owned(),
                details: None,
                retryable: false,
            },
            RequestError::NotFound => Self {
                code: IpcErrorCode::RequestNotFound,
                message: "Request item was not found.".to_owned(),
                details: None,
                retryable: false,
            },
            RequestError::SavedRequestAlreadyOpen => Self {
                code: IpcErrorCode::SavedRequestAlreadyOpen,
                message: "Saved request is already open.".to_owned(),
                details: None,
                retryable: false,
            },
            RequestError::InvalidInput(detail) => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Request input is invalid.".to_owned(),
                details: Some(detail),
                retryable: false,
            },
            RequestError::Persistence(_) => Self {
                code: IpcErrorCode::PersistenceUnavailable,
                message: "Request persistence is unavailable.".to_owned(),
                details: None,
                retryable: true,
            },
        }
    }
}

impl From<WorkspaceError> for IpcError {
    fn from(error: WorkspaceError) -> Self {
        match error {
            WorkspaceError::InvalidName(error) => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Workspace name is invalid.".to_owned(),
                details: Some(workspace_name_detail(error)),
                retryable: false,
            },
            WorkspaceError::NotFound => Self {
                code: IpcErrorCode::WorkspaceNotFound,
                message: "Workspace was not found.".to_owned(),
                details: None,
                retryable: false,
            },
            WorkspaceError::AlreadyExists => Self {
                code: IpcErrorCode::WorkspaceAlreadyExists,
                message: "Workspace name already exists.".to_owned(),
                details: Some("name".to_owned()),
                retryable: false,
            },
            WorkspaceError::CannotDeleteLastWorkspace => Self {
                code: IpcErrorCode::CannotDeleteLastWorkspace,
                message: "The last workspace cannot be deleted.".to_owned(),
                details: None,
                retryable: false,
            },
            WorkspaceError::Persistence(_) => Self {
                code: IpcErrorCode::PersistenceUnavailable,
                message: "Workspace persistence is unavailable.".to_owned(),
                details: None,
                retryable: true,
            },
        }
    }
}

fn workspace_name_detail(error: WorkspaceNameError) -> String {
    match error {
        WorkspaceNameError::Empty => "name.required".to_owned(),
        WorkspaceNameError::TooLong { .. } => "name.tooLong".to_owned(),
        WorkspaceNameError::ControlCharacter => "name.controlCharacter".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::{
        application::workspace::{WorkspaceRepository, WorkspaceSummary},
        domain::workspace::{Workspace, WorkspaceName},
    };

    struct FakeWorkspaceRepository {
        snapshot: WorkspaceSnapshot,
        next_error: Option<WorkspaceError>,
        calls: Vec<&'static str>,
    }

    impl FakeWorkspaceRepository {
        fn new() -> Self {
            let workspace = Workspace::new(WorkspaceName::new("Personal").expect("valid name"));
            Self {
                snapshot: WorkspaceSnapshot {
                    selected_workspace_id: workspace.id,
                    workspaces: vec![WorkspaceSummary {
                        id: workspace.id,
                        name: workspace.name,
                        is_selected: true,
                    }],
                },
                next_error: None,
                calls: Vec::new(),
            }
        }

        fn result(&mut self, call: &'static str) -> Result<WorkspaceSnapshot, WorkspaceError> {
            self.calls.push(call);
            match self.next_error.take() {
                Some(error) => Err(error),
                None => Ok(self.snapshot.clone()),
            }
        }
    }

    impl WorkspaceRepository for FakeWorkspaceRepository {
        fn initialize(&mut self) -> Result<WorkspaceSnapshot, WorkspaceError> {
            self.result("initialize")
        }

        fn create_workspace(
            &mut self,
            _name: WorkspaceName,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            self.result("create")
        }

        fn list_workspaces(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
            if let Some(error) = &self.next_error {
                return Err(match error {
                    WorkspaceError::InvalidName(_) => {
                        WorkspaceError::InvalidName(WorkspaceNameError::Empty)
                    }
                    WorkspaceError::NotFound => WorkspaceError::NotFound,
                    WorkspaceError::AlreadyExists => WorkspaceError::AlreadyExists,
                    WorkspaceError::CannotDeleteLastWorkspace => {
                        WorkspaceError::CannotDeleteLastWorkspace
                    }
                    WorkspaceError::Persistence(message) => {
                        WorkspaceError::Persistence(message.clone())
                    }
                });
            }

            Ok(self.snapshot.clone())
        }

        fn rename_workspace(
            &mut self,
            _id: WorkspaceId,
            _name: WorkspaceName,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            self.result("rename")
        }

        fn switch_workspace(
            &mut self,
            _id: WorkspaceId,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            self.result("switch")
        }

        fn delete_workspace(
            &mut self,
            _id: WorkspaceId,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            self.result("delete")
        }
    }

    #[test]
    fn snapshot_dto_serializes_with_camel_case_names() {
        let repository = FakeWorkspaceRepository::new();
        let snapshot = WorkspaceSnapshotDto::from(repository.snapshot);

        let value = serde_json::to_value(snapshot).expect("serialize snapshot");

        assert_eq!(
            value,
            json!({
                "selectedWorkspaceId": value["selectedWorkspaceId"],
                "workspaces": [{
                    "id": value["workspaces"][0]["id"],
                    "name": "Personal",
                    "isSelected": true
                }]
            })
        );
    }

    #[test]
    fn invalid_workspace_id_maps_to_safe_non_retryable_error() {
        let service = Mutex::new(WorkspaceService::new(FakeWorkspaceRepository::new()));

        let error = handle_switch_workspace(
            service.lock().expect("lock service"),
            WorkspaceIdInput {
                workspace_id: "not-a-uuid".to_owned(),
            },
        )
        .expect_err("invalid id");

        assert_eq!(error.code, IpcErrorCode::InvalidInput);
        assert_eq!(error.details.as_deref(), Some("workspaceId"));
        assert!(!error.retryable);
    }

    #[test]
    fn workspace_errors_map_to_stable_error_codes() {
        let cases = [
            (
                WorkspaceError::InvalidName(WorkspaceNameError::ControlCharacter),
                IpcErrorCode::InvalidInput,
                false,
            ),
            (
                WorkspaceError::NotFound,
                IpcErrorCode::WorkspaceNotFound,
                false,
            ),
            (
                WorkspaceError::AlreadyExists,
                IpcErrorCode::WorkspaceAlreadyExists,
                false,
            ),
            (
                WorkspaceError::CannotDeleteLastWorkspace,
                IpcErrorCode::CannotDeleteLastWorkspace,
                false,
            ),
            (
                WorkspaceError::Persistence("SQLITE_BUSY: sentinel database path".to_owned()),
                IpcErrorCode::PersistenceUnavailable,
                true,
            ),
        ];

        for (source, code, retryable) in cases {
            let error = IpcError::from(source);
            let serialized = serde_json::to_string(&error).expect("serialize error");

            assert_eq!(error.code, code);
            assert_eq!(error.retryable, retryable);
            assert!(!serialized.contains("SQLITE_BUSY"));
            assert!(!serialized.contains("sentinel database path"));
        }
    }

    #[test]
    fn poisoned_lock_maps_to_safe_retryable_error_without_lock_details() {
        let error = map_poison_error(PoisonError::new("sentinel poisoned lock detail"));
        let serialized = serde_json::to_string(&error).expect("serialize error");

        assert_eq!(error.code, IpcErrorCode::StateUnavailable);
        assert!(error.retryable);
        assert!(!serialized.contains("sentinel poisoned lock detail"));
    }

    #[test]
    fn commands_delegate_to_workspace_service() {
        let service = Mutex::new(WorkspaceService::new(FakeWorkspaceRepository::new()));
        let id = {
            let snapshot = service
                .lock()
                .expect("lock service")
                .list_workspaces()
                .expect("list");
            snapshot.selected_workspace_id.to_string()
        };

        let created = handle_create_workspace(
            service.lock().expect("lock service"),
            CreateWorkspaceInput {
                name: "Client".to_owned(),
            },
        )
        .expect("create");
        assert_eq!(created.workspaces.len(), 1);

        handle_rename_workspace(
            service.lock().expect("lock service"),
            RenameWorkspaceInput {
                workspace_id: id.clone(),
                name: "Renamed".to_owned(),
            },
        )
        .expect("rename");
        handle_switch_workspace(
            service.lock().expect("lock service"),
            WorkspaceIdInput {
                workspace_id: id.clone(),
            },
        )
        .expect("switch");
        handle_delete_workspace(
            service.lock().expect("lock service"),
            WorkspaceIdInput { workspace_id: id },
        )
        .expect("delete");
    }
}
