use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_WORKSPACE_NAME: &str = "Personal";
const MAX_WORKSPACE_NAME_LEN: usize = 120;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct WorkspaceId(Uuid);

impl WorkspaceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for WorkspaceId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceName(String);

impl WorkspaceName {
    pub fn new(value: impl AsRef<str>) -> Result<Self, WorkspaceNameError> {
        let trimmed = value.as_ref().trim();
        if trimmed.is_empty() {
            return Err(WorkspaceNameError::Empty);
        }
        if trimmed.chars().count() > MAX_WORKSPACE_NAME_LEN {
            return Err(WorkspaceNameError::TooLong {
                max: MAX_WORKSPACE_NAME_LEN,
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(WorkspaceNameError::ControlCharacter);
        }

        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum WorkspaceNameError {
    #[error("workspace name is required")]
    Empty,
    #[error("workspace name must be {max} characters or fewer")]
    TooLong { max: usize },
    #[error("workspace name cannot contain control characters")]
    ControlCharacter,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: WorkspaceName,
}

impl Workspace {
    pub fn new(name: WorkspaceName) -> Self {
        Self {
            id: WorkspaceId::new(),
            name,
        }
    }
}
