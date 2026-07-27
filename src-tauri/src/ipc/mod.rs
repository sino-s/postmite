//! Typed Tauri IPC boundary.

use std::{
    str::FromStr,
    sync::{MutexGuard, PoisonError},
};

use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::{Config, TS};

use crate::{
    application::workspace::{WorkspaceError, WorkspaceService, WorkspaceSnapshot},
    domain::workspace::{WorkspaceId, WorkspaceNameError},
    AppState,
};

pub const LIST_WORKSPACES_COMMAND: &str = "list_workspaces";
pub const CREATE_WORKSPACE_COMMAND: &str = "create_workspace";
pub const RENAME_WORKSPACE_COMMAND: &str = "rename_workspace";
pub const SWITCH_WORKSPACE_COMMAND: &str = "switch_workspace";
pub const DELETE_WORKSPACE_COMMAND: &str = "delete_workspace";

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpcErrorCode {
    InvalidInput,
    WorkspaceNotFound,
    WorkspaceAlreadyExists,
    CannotDeleteLastWorkspace,
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
    InvalidWorkspaceId,
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

    Ok(contract)
}

fn parse_workspace_id(value: &str) -> Result<WorkspaceId, IpcError> {
    WorkspaceId::from_str(value).map_err(|_| BoundaryError::InvalidWorkspaceId.into())
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

impl From<BoundaryError> for IpcError {
    fn from(error: BoundaryError) -> Self {
        match error {
            BoundaryError::Workspace(error) => error.into(),
            BoundaryError::InvalidWorkspaceId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Workspace id is invalid.".to_owned(),
                details: Some("workspaceId".to_owned()),
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
