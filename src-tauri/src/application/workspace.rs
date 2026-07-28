use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::workspace::{Workspace, WorkspaceId, WorkspaceName, WorkspaceNameError};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceSummary {
    pub id: WorkspaceId,
    pub name: WorkspaceName,
    pub is_selected: bool,
    pub base_directory: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceSnapshot {
    pub selected_workspace_id: WorkspaceId,
    pub workspaces: Vec<WorkspaceSummary>,
}

pub trait WorkspaceRepository {
    fn initialize(&mut self) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn create_workspace(
        &mut self,
        name: WorkspaceName,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn list_workspaces(&self) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn rename_workspace(
        &mut self,
        id: WorkspaceId,
        name: WorkspaceName,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn set_workspace_base_directory(
        &mut self,
        id: WorkspaceId,
        base_directory: Option<String>,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn switch_workspace(&mut self, id: WorkspaceId) -> Result<WorkspaceSnapshot, WorkspaceError>;
    fn delete_workspace(&mut self, id: WorkspaceId) -> Result<WorkspaceSnapshot, WorkspaceError>;
}

pub struct WorkspaceService<R> {
    repository: R,
}

impl<R> WorkspaceService<R>
where
    R: WorkspaceRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn initialize(&mut self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        self.repository.initialize()
    }

    pub fn create_workspace(
        &mut self,
        name: impl AsRef<str>,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        self.repository
            .create_workspace(WorkspaceName::new(name).map_err(WorkspaceError::InvalidName)?)
    }

    pub fn list_workspaces(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        self.repository.list_workspaces()
    }

    pub fn rename_workspace(
        &mut self,
        id: WorkspaceId,
        name: impl AsRef<str>,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        self.repository.rename_workspace(
            id,
            WorkspaceName::new(name).map_err(WorkspaceError::InvalidName)?,
        )
    }

    pub fn set_workspace_base_directory(
        &mut self,
        id: WorkspaceId,
        base_directory: Option<String>,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        self.repository
            .set_workspace_base_directory(id, base_directory)
    }

    pub fn switch_workspace(
        &mut self,
        id: WorkspaceId,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        self.repository.switch_workspace(id)
    }

    pub fn delete_workspace(
        &mut self,
        id: WorkspaceId,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        self.repository.delete_workspace(id)
    }
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    InvalidName(WorkspaceNameError),
    #[error("workspace was not found")]
    NotFound,
    #[error("workspace name already exists")]
    AlreadyExists,
    #[error("the last workspace cannot be deleted")]
    CannotDeleteLastWorkspace,
    #[error("workspace persistence failed: {0}")]
    Persistence(String),
}

impl WorkspaceError {
    pub fn persistence(error: impl std::error::Error) -> Self {
        Self::Persistence(error.to_string())
    }
}

impl From<Workspace> for WorkspaceSummary {
    fn from(workspace: Workspace) -> Self {
        Self {
            id: workspace.id,
            name: workspace.name,
            is_selected: false,
            base_directory: None,
        }
    }
}
