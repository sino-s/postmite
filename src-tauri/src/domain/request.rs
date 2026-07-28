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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ExecutionRecordId(Uuid);

impl ExecutionRecordId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ExecutionRecordId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExecutionRecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ExecutionRecordId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(value)?))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct CookieId(Uuid);

impl CookieId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CookieId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CookieId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for CookieId {
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
    pub body: RequestBody,
    pub query: Vec<OrderedField>,
    pub headers: Vec<OrderedField>,
}

impl RequestContent {
    pub fn blank() -> Self {
        Self {
            name: "Untitled Request".to_owned(),
            method: "GET".to_owned(),
            url: String::new(),
            body: RequestBody::None,
            query: Vec::new(),
            headers: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum RequestBody {
    None,
    Raw { content: String },
    UrlEncoded { fields: Vec<OrderedField> },
    Multipart { parts: Vec<MultipartPart> },
    Binary { file: BodyFileReference },
}

impl RequestBody {
    pub fn legacy_raw_text(&self) -> String {
        match self {
            Self::None => String::new(),
            Self::Raw { content } => content.clone(),
            Self::UrlEncoded { fields } => fields
                .iter()
                .filter(|field| field.enabled)
                .map(|field| format!("{}={}", field.name, field.value))
                .collect::<Vec<_>>()
                .join("&"),
            Self::Multipart { .. } | Self::Binary { .. } => String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum MultipartPart {
    Field {
        enabled: bool,
        order: u32,
        name: String,
        value: String,
    },
    File {
        enabled: bool,
        order: u32,
        name: String,
        file: BodyFileReference,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyFileReference {
    pub path: BodyFilePath,
    pub file_name: String,
    pub size: u64,
    pub modified_at_epoch_seconds: Option<i64>,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum BodyFilePath {
    Relative { path: String },
    Absolute { path: String },
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionRecord {
    pub id: ExecutionRecordId,
    pub workspace_id: WorkspaceId,
    pub created_at_epoch_seconds: i64,
    pub request: RequestContent,
    pub response: ExecutionRecordResponse,
    pub pinned: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CookieSameSite {
    Strict,
    Lax,
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceCookie {
    pub id: CookieId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<CookieSameSite>,
    pub expires_at_epoch_seconds: Option<i64>,
    pub session: bool,
    pub has_value: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CookieDraft {
    pub id: Option<CookieId>,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<CookieSameSite>,
    pub expires_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionRecordResponse {
    pub status: Option<u16>,
    pub headers: Vec<OrderedField>,
    pub body_preview: String,
    pub body_truncated: bool,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}
