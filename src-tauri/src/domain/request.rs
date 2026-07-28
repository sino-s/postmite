use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::workspace::WorkspaceId;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct CollectionId(Uuid);

impl CollectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CollectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CollectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for CollectionId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct EnvironmentId(Uuid);

impl EnvironmentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EnvironmentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EnvironmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EnvironmentId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SavedRequestId(Uuid);

impl SavedRequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SavedRequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SavedRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for SavedRequestId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct RequestDraftId(Uuid);

impl RequestDraftId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RequestDraftId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RequestDraftId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RequestDraftId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct RequestTabId(Uuid);

impl RequestTabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RequestTabId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RequestTabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RequestTabId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CollectionFolder {
    pub id: CollectionId,
    pub workspace_id: WorkspaceId,
    pub parent_collection_id: Option<CollectionId>,
    pub name: String,
    pub position: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Environment {
    pub id: EnvironmentId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub position: u32,
    pub is_selected: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Variable {
    pub name: String,
    pub value: VariableValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum VariableValue {
    Plain(String),
    SecretReference(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CollectionVariable {
    pub workspace_id: WorkspaceId,
    pub variable: Variable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentVariable {
    pub environment_id: EnvironmentId,
    pub workspace_id: WorkspaceId,
    pub variable: Variable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OrderedField {
    pub enabled: bool,
    pub order: u32,
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestContent {
    pub name: String,
    pub method: String,
    pub url: String,
    pub body: String,
    pub query: Vec<OrderedField>,
    pub headers: Vec<OrderedField>,
}

impl RequestContent {
    pub fn blank() -> Self {
        Self {
            name: "Untitled Request".to_owned(),
            method: "GET".to_owned(),
            url: String::new(),
            body: String::new(),
            query: Vec::new(),
            headers: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SavedRequest {
    pub id: SavedRequestId,
    pub workspace_id: WorkspaceId,
    pub collection_id: Option<CollectionId>,
    pub position: u32,
    pub content: RequestContent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestDraft {
    pub id: RequestDraftId,
    pub workspace_id: WorkspaceId,
    pub saved_request_id: Option<SavedRequestId>,
    pub content: RequestContent,
    pub is_dirty: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestTab {
    pub id: RequestTabId,
    pub workspace_id: WorkspaceId,
    pub saved_request_id: Option<SavedRequestId>,
    pub draft_id: RequestDraftId,
    pub position: u32,
    pub title: String,
    pub is_active: bool,
}
