//! Typed Tauri IPC boundary.

use std::{
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
    str::FromStr,
    sync::Arc,
    sync::{MutexGuard, PoisonError},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager, State};
use ts_rs::{Config, TS};

use crate::{
    application::backup::{
        NativeBackupError, NativeBackupExclusion,
        NativeBackupExportInput as ApplicationNativeBackupExportInput, NativeBackupExportResult,
        NativeBackupManifest, NativeBackupManifestEntry, NativeBackupRepository,
        NativeBackupRestoreInput as ApplicationNativeBackupRestoreInput,
        NativeBackupRestorePreview,
        NativeBackupRestorePreviewInput as ApplicationNativeBackupRestorePreviewInput,
        NativeBackupRestoreResult, NativeBackupService,
    },
    application::curl::{
        CurlError, CurlGenerateInput as ApplicationCurlGenerateInput, CurlGenerateResult,
        CurlImportInput as ApplicationCurlImportInput, CurlImportPreview, CurlImportWarning,
        CurlService, CurlUnsupportedField,
    },
    application::execution::{
        CancelExecutionResult, ExecutionError, ExecutionEvent, ExecutionEventKind, ExecutionHeader,
        ExecutionId, ExecutionProxyMetadata, ExecutionRequest, ExecutionTimeoutMetadata,
        ExecutionTimingMetadata, StartExecutionResult,
    },
    application::oauth::{
        CancelOAuthAuthorizationResult, OAuthAuthorizationResult, OAuthError, OAuthFlowId,
        StartOAuthAuthorizationRequest,
    },
    application::postman_import::{
        PostmanEnvironmentExport, PostmanExportInput as ApplicationPostmanExportInput,
        PostmanExportResult, PostmanImportError,
        PostmanImportInput as ApplicationPostmanImportInput, PostmanImportPreview,
        PostmanImportRepository, PostmanImportResult, PostmanImportService, PostmanImportWarning,
        PostmanPriorImport, PostmanReimportChange, PostmanReimportDecision,
        PostmanReimportInput as ApplicationPostmanReimportInput, PostmanReimportPreview,
        PostmanReimportResult, PostmanUnsupportedField,
    },
    application::request::{
        CloseTabDecision, CollectionLocation, CookieJarSnapshot, ExecutionHistorySnapshot,
        RequestError, RequestRepository, RequestService, RequestWorkspaceSnapshot, ResolvedField,
        ResolvedMultipartPart, ResolvedRequestBody, ResolvedRequestContent, ResolvedValue,
        ResolvedVariableReference, VariableResolutionError, VariableResolutionErrorKind,
        VariableSource, REDACTED_VALUE,
    },
    application::workspace::{WorkspaceError, WorkspaceService, WorkspaceSnapshot},
    domain::{
        request::{
            ApiKeyPlacement, BodyFilePath, BodyFileReference, CollectionFolder, CollectionId,
            CollectionVariable, CookieDraft, CookieId, CookieSameSite, Environment, EnvironmentId,
            EnvironmentVariable, ExecutionRecord, ExecutionRecordId, ExecutionRecordResponse,
            MultipartPart, OrderedField, ProxyPolicy, ProxySource, RedirectPolicy, RequestAuth,
            RequestBody, RequestContent, RequestDraft, RequestDraftId, RequestTab, RequestTabId,
            SavedRequest, SavedRequestId, TimeoutPolicy, TlsPolicy, TransportPolicy, Variable,
            VariableValue, WorkspaceCookie,
        },
        workspace::{WorkspaceId, WorkspaceNameError},
    },
    AppState,
};

pub const LIST_WORKSPACES_COMMAND: &str = "list_workspaces";
pub const CREATE_WORKSPACE_COMMAND: &str = "create_workspace";
pub const RENAME_WORKSPACE_COMMAND: &str = "rename_workspace";
pub const SET_WORKSPACE_BASE_DIRECTORY_COMMAND: &str = "set_workspace_base_directory";
pub const SWITCH_WORKSPACE_COMMAND: &str = "switch_workspace";
pub const DELETE_WORKSPACE_COMMAND: &str = "delete_workspace";
pub const LIST_REQUEST_WORKSPACE_COMMAND: &str = "list_request_workspace";
pub const OPEN_UNSAVED_REQUEST_TAB_COMMAND: &str = "open_unsaved_request_tab";
pub const CREATE_SAVED_REQUEST_COMMAND: &str = "create_saved_request";
pub const CREATE_COLLECTION_FOLDER_COMMAND: &str = "create_collection_folder";
pub const SELECT_ENVIRONMENT_COMMAND: &str = "select_environment";
pub const RESOLVE_REQUEST_CONTENT_COMMAND: &str = "resolve_request_content";
pub const RENAME_COLLECTION_FOLDER_COMMAND: &str = "rename_collection_folder";
pub const MOVE_COLLECTION_FOLDER_COMMAND: &str = "move_collection_folder";
pub const DUPLICATE_COLLECTION_FOLDER_COMMAND: &str = "duplicate_collection_folder";
pub const DELETE_COLLECTION_FOLDER_COMMAND: &str = "delete_collection_folder";
pub const MOVE_SAVED_REQUEST_COMMAND: &str = "move_saved_request";
pub const DUPLICATE_SAVED_REQUEST_COMMAND: &str = "duplicate_saved_request";
pub const DELETE_SAVED_REQUEST_COMMAND: &str = "delete_saved_request";
pub const OPEN_SAVED_REQUEST_TAB_COMMAND: &str = "open_saved_request_tab";
pub const UPDATE_REQUEST_DRAFT_COMMAND: &str = "update_request_draft";
pub const FLUSH_REQUEST_DRAFTS_COMMAND: &str = "flush_request_drafts";
pub const SAVE_REQUEST_DRAFT_COMMAND: &str = "save_request_draft";
pub const CLOSE_REQUEST_TAB_COMMAND: &str = "close_request_tab";
pub const LIST_EXECUTION_HISTORY_COMMAND: &str = "list_execution_history";
pub const SET_EXECUTION_HISTORY_DISABLED_COMMAND: &str = "set_execution_history_disabled";
pub const SET_EXECUTION_RECORD_PINNED_COMMAND: &str = "set_execution_record_pinned";
pub const OPEN_EXECUTION_RECORD_AS_DRAFT_COMMAND: &str = "open_execution_record_as_draft";
pub const LIST_COOKIES_COMMAND: &str = "list_cookies";
pub const UPSERT_COOKIE_COMMAND: &str = "upsert_cookie";
pub const DELETE_COOKIE_COMMAND: &str = "delete_cookie";
pub const CLEAR_COOKIES_COMMAND: &str = "clear_cookies";
pub const REVEAL_COOKIE_VALUE_COMMAND: &str = "reveal_cookie_value";
pub const DESCRIBE_BODY_FILE_COMMAND: &str = "describe_body_file";
pub const RELINK_BODY_FILES_COMMAND: &str = "relink_body_files";
pub const PREVIEW_POSTMAN_IMPORT_COMMAND: &str = "preview_postman_import";
pub const IMPORT_POSTMAN_COMMAND: &str = "import_postman";
pub const EXPORT_POSTMAN_COMMAND: &str = "export_postman";
pub const PREVIEW_POSTMAN_REIMPORT_COMMAND: &str = "preview_postman_reimport";
pub const REIMPORT_POSTMAN_COMMAND: &str = "reimport_postman";
pub const EXPORT_NATIVE_BACKUP_COMMAND: &str = "export_native_backup";
pub const PREVIEW_NATIVE_BACKUP_RESTORE_COMMAND: &str = "preview_native_backup_restore";
pub const RESTORE_NATIVE_BACKUP_COMMAND: &str = "restore_native_backup";
pub const PREVIEW_CURL_IMPORT_COMMAND: &str = "preview_curl_import";
pub const IMPORT_CURL_AS_DRAFT_COMMAND: &str = "import_curl_as_draft";
pub const GENERATE_CURL_COMMAND: &str = "generate_curl";
pub const START_REQUEST_EXECUTION_COMMAND: &str = "start_request_execution";
pub const CANCEL_REQUEST_EXECUTION_COMMAND: &str = "cancel_request_execution";
pub const START_OAUTH_AUTHORIZATION_COMMAND: &str = "start_oauth_authorization";
pub const CANCEL_OAUTH_AUTHORIZATION_COMMAND: &str = "cancel_oauth_authorization";
pub const REQUEST_EXECUTION_EVENT: &str = "request_execution_event";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummaryDto {
    pub id: String,
    pub name: String,
    pub is_selected: bool,
    pub base_directory: Option<String>,
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
pub struct SetWorkspaceBaseDirectoryInput {
    pub workspace_id: String,
    pub base_directory: Option<String>,
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
    pub body: RequestBodyDto,
    pub query: Vec<OrderedFieldDto>,
    pub headers: Vec<OrderedFieldDto>,
    pub auth: RequestAuthDto,
    pub redirect: RedirectPolicyDto,
    pub tls: TlsPolicyDto,
    pub transport: TransportPolicyDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum RequestAuthDto {
    None,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    ApiKey {
        placement: ApiKeyPlacementDto,
        name: String,
        value: String,
    },
    ClientCredentials {
        token_endpoint: String,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiKeyPlacementDto {
    Header,
    Query,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RedirectPolicyDto {
    pub enabled: bool,
    pub max_redirects: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TlsPolicyDto {
    pub verify: bool,
    pub custom_ca_reference: Option<String>,
    pub client_certificate_reference: Option<String>,
    pub client_key_reference: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TransportPolicyDto {
    pub proxy: ProxyPolicyDto,
    pub timeouts: TimeoutPolicyDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPolicyDto {
    pub source: ProxySourceDto,
    pub url: Option<String>,
    pub no_proxy: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProxySourceDto {
    Disabled,
    ProcessEnvironment,
    Custom,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TimeoutPolicyDto {
    pub connect_ms: u64,
    pub overall_ms: u64,
    pub idle_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum RequestBodyDto {
    None,
    Raw { content: String },
    UrlEncoded { fields: Vec<OrderedFieldDto> },
    Multipart { parts: Vec<MultipartPartDto> },
    Binary { file: BodyFileReferenceDto },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum MultipartPartDto {
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
        file: BodyFileReferenceDto,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BodyFileReferenceDto {
    pub path: BodyFilePathDto,
    pub file_name: String,
    pub size: u64,
    pub modified_at_epoch_seconds: Option<i64>,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum BodyFilePathDto {
    Relative { path: String },
    Absolute { path: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DescribeBodyFileInput {
    pub workspace_id: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RelinkBodyFilesInput {
    pub workspace_id: String,
    pub from_path: String,
    pub replacement_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SavedRequestDto {
    pub id: String,
    pub workspace_id: String,
    pub collection_id: Option<String>,
    pub position: u32,
    pub content: RequestContentDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CollectionFolderDto {
    pub id: String,
    pub workspace_id: String,
    pub parent_collection_id: Option<String>,
    pub name: String,
    pub position: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentDto {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub position: u32,
    pub is_selected: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum VariableValueDto {
    Plain { value: String },
    SecretReference { reference: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VariableDto {
    pub name: String,
    pub value: VariableValueDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CollectionVariableDto {
    pub workspace_id: String,
    pub variable: VariableDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentVariableDto {
    pub environment_id: String,
    pub workspace_id: String,
    pub variable: VariableDto,
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
    pub collection_folders: Vec<CollectionFolderDto>,
    pub environments: Vec<EnvironmentDto>,
    pub collection_variables: Vec<CollectionVariableDto>,
    pub environment_variables: Vec<EnvironmentVariableDto>,
    pub saved_requests: Vec<SavedRequestDto>,
    pub drafts: Vec<RequestDraftDto>,
    pub tabs: Vec<RequestTabDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanImportInput {
    pub workspace_id: String,
    pub source_name: String,
    pub collection_json: String,
    pub environment_json: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanExportInput {
    pub workspace_id: String,
    pub source_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanExportResultDto {
    pub collection_json: String,
    pub environments: Vec<PostmanEnvironmentExportDto>,
    pub warning_count: u32,
    pub unsupported_count: u32,
    pub warnings: Vec<PostmanImportWarningDto>,
    pub unsupported: Vec<PostmanUnsupportedFieldDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanEnvironmentExportDto {
    pub name: String,
    pub environment_json: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanImportPreviewDto {
    pub source_id: String,
    pub source_name: String,
    pub source_hash: String,
    pub collection_count: u32,
    pub request_count: u32,
    pub environment_count: u32,
    pub warning_count: u32,
    pub unsupported_count: u32,
    pub warnings: Vec<PostmanImportWarningDto>,
    pub unsupported: Vec<PostmanUnsupportedFieldDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanImportWarningDto {
    pub location: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanUnsupportedFieldDto {
    pub location: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanImportResultDto {
    pub preview: PostmanImportPreviewDto,
    pub snapshot: RequestWorkspaceSnapshotDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanReimportPreviewDto {
    pub import_preview: PostmanImportPreviewDto,
    pub prior_import: Option<PostmanPriorImportDto>,
    pub changes: Vec<PostmanReimportChangeDto>,
    pub can_update: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanPriorImportDto {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub source_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanReimportChangeDto {
    pub location: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PostmanReimportDecisionDto {
    Update,
    Duplicate,
    Cancel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanReimportInput {
    pub import: PostmanImportInput,
    pub decision: PostmanReimportDecisionDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PostmanReimportResultDto {
    pub preview: PostmanReimportPreviewDto,
    pub snapshot: RequestWorkspaceSnapshotDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupExportInput {
    pub workspace_id: String,
    pub backup_path: String,
    pub include_body_files: bool,
    pub body_files_directory: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupRestorePreviewInput {
    pub backup_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupRestoreInput {
    pub backup_path: String,
    pub workspace_name: String,
    pub body_files_directory: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupExportResultDto {
    pub backup_path: String,
    pub manifest: NativeBackupManifestDto,
    pub preview: NativeBackupRestorePreviewDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupRestoreResultDto {
    pub preview: NativeBackupRestorePreviewDto,
    pub workspace_snapshot: WorkspaceSnapshotDto,
    pub request_snapshot: RequestWorkspaceSnapshotDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupRestorePreviewDto {
    pub source_workspace_name: String,
    pub collection_count: u32,
    pub request_count: u32,
    pub environment_count: u32,
    pub history_record_count: u32,
    pub cookie_count: u32,
    pub body_file_count: u32,
    pub expanded_bytes: u64,
    pub exclusions: Vec<NativeBackupExclusionDto>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupExclusionDto {
    pub location: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupManifestDto {
    pub format: String,
    pub version: u32,
    pub required_features: Vec<String>,
    pub entries: Vec<NativeBackupManifestEntryDto>,
    pub exclusions: Vec<NativeBackupExclusionDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupManifestEntryDto {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CurlImportInput {
    pub workspace_id: String,
    pub source_name: String,
    pub command: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CurlGenerateInput {
    pub content: RequestContentDto,
    pub resolved: Option<ResolvedRequestContentDto>,
    pub include_secrets: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CurlImportPreviewDto {
    pub source_name: String,
    pub content: RequestContentDto,
    pub warning_count: u32,
    pub unsupported_count: u32,
    pub warnings: Vec<CurlImportWarningDto>,
    pub unsupported: Vec<CurlUnsupportedFieldDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CurlImportWarningDto {
    pub location: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CurlUnsupportedFieldDto {
    pub location: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CurlImportResultDto {
    pub preview: CurlImportPreviewDto,
    pub snapshot: RequestWorkspaceSnapshotDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CurlGenerateResultDto {
    pub command: String,
    pub included_secret_count: u32,
    pub redacted_secret_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionHistorySnapshotDto {
    pub workspace_id: String,
    pub disabled: bool,
    pub records: Vec<ExecutionRecordDto>,
    pub warning: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRecordDto {
    pub id: String,
    pub workspace_id: String,
    pub created_at_epoch_seconds: i64,
    pub request: RequestContentDto,
    pub response: ExecutionRecordResponseDto,
    pub pinned: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRecordResponseDto {
    pub status: Option<u16>,
    pub headers: Vec<OrderedFieldDto>,
    pub body_preview: String,
    pub body_truncated: bool,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CookieSameSiteDto {
    Strict,
    Lax,
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCookieDto {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<CookieSameSiteDto>,
    pub expires_at_epoch_seconds: Option<i64>,
    pub session: bool,
    pub has_value: bool,
    pub value_preview: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CookieJarSnapshotDto {
    pub workspace_id: String,
    pub cookies: Vec<WorkspaceCookieDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCookieInput {
    pub workspace_id: String,
    pub cookie_id: Option<String>,
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<CookieSameSiteDto>,
    pub expires_at_epoch_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CookieIdInput {
    pub workspace_id: String,
    pub cookie_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RevealCookieValueOutput {
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateSavedRequestInput {
    pub workspace_id: String,
    pub content: RequestContentDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CollectionLocationDto {
    pub collection_id: Option<String>,
    pub position: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SelectEnvironmentInput {
    pub workspace_id: String,
    pub environment_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResolveRequestContentInput {
    pub workspace_id: String,
    pub content: RequestContentDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRequestContentDto {
    pub url: ResolvedValueDto,
    pub body: ResolvedValueDto,
    pub query: Vec<ResolvedFieldDto>,
    pub headers: Vec<ResolvedFieldDto>,
    pub unsafe_tls_visible: bool,
    pub references: Vec<ResolvedVariableReferenceDto>,
    pub errors: Vec<VariableResolutionErrorDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFieldDto {
    pub enabled: bool,
    pub order: u32,
    pub name: ResolvedValueDto,
    pub value: ResolvedValueDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedValueDto {
    pub value: String,
    pub contains_secret: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedVariableReferenceDto {
    pub name: String,
    pub source: VariableSourceDto,
    pub value: ResolvedValueDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VariableSourceDto {
    Collection,
    Environment,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VariableResolutionErrorDto {
    pub name: String,
    pub kind: VariableResolutionErrorKindDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VariableResolutionErrorKindDto {
    Missing,
    Cycle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionFolderInput {
    pub workspace_id: String,
    pub parent_collection_id: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RenameCollectionFolderInput {
    pub workspace_id: String,
    pub collection_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MoveCollectionFolderInput {
    pub workspace_id: String,
    pub collection_id: String,
    pub location: CollectionLocationDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CollectionFolderIdInput {
    pub workspace_id: String,
    pub collection_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MoveSavedRequestInput {
    pub workspace_id: String,
    pub saved_request_id: String,
    pub location: CollectionLocationDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SavedRequestIdInput {
    pub workspace_id: String,
    pub saved_request_id: String,
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
pub struct SetExecutionHistoryDisabledInput {
    pub workspace_id: String,
    pub disabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetExecutionRecordPinnedInput {
    pub workspace_id: String,
    pub record_id: String,
    pub pinned: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRecordIdInput {
    pub workspace_id: String,
    pub record_id: String,
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
pub struct StartOAuthAuthorizationInput {
    pub flow_id: String,
    pub authorization_endpoint: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub redirect_path: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OAuthAuthorizationResultDto {
    pub flow_id: String,
    pub redirect_uri: String,
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CancelOAuthAuthorizationInput {
    pub flow_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CancelOAuthAuthorizationOutput {
    pub flow_id: String,
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
        tls_verification: bool,
        proxy: ExecutionProxyMetadataDto,
        timeouts: ExecutionTimeoutMetadataDto,
        queued_ms: u64,
    },
    Redirected {
        from: String,
        to: String,
        status: u16,
    },
    UploadProgress {
        sent_bytes: u64,
        total_bytes: u64,
    },
    ResponseHeaders {
        status: u16,
        headers: Vec<ExecutionHeaderDto>,
        protocol: String,
        remote_addr: Option<String>,
    },
    DownloadProgress {
        received_bytes: u64,
        total_bytes: Option<u64>,
    },
    Completed {
        status: u16,
        body_preview: String,
        body_truncated: bool,
        decoded_bytes: u64,
        wire_bytes: Option<u64>,
        timing: ExecutionTimingMetadataDto,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionProxyMetadataDto {
    pub source: String,
    pub selected_proxy: Option<String>,
    pub bypass_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionTimeoutMetadataDto {
    pub connect_ms: Option<u64>,
    pub overall_ms: Option<u64>,
    pub idle_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionTimingMetadataDto {
    pub queued_ms: u64,
    pub dns_ms: Option<u64>,
    pub connect_ms: Option<u64>,
    pub tls_ms: Option<u64>,
    pub first_byte_ms: Option<u64>,
    pub download_ms: Option<u64>,
    pub total_ms: u64,
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
    InvalidCollectionId,
    InvalidEnvironmentId,
    InvalidSavedRequestId,
    InvalidRequestDraftId,
    InvalidRequestTabId,
    InvalidExecutionId,
    InvalidOAuthFlowId,
    InvalidExecutionRecordId,
    InvalidCookieId,
    Execution(ExecutionError),
    OAuth(OAuthError),
    NativeBackup(NativeBackupError),
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
pub fn generate_curl(input: CurlGenerateInput) -> Result<CurlGenerateResultDto, IpcError> {
    let input = ApplicationCurlGenerateInput::from(input);
    CurlService::generate(input)
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
    let app_for_sink = app.clone();
    let sink = Arc::new(move |event: ExecutionEvent| {
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
        let _ = app_for_sink.emit(REQUEST_EXECUTION_EVENT, ExecutionEventDto::from(event));
    });

    let oauth = Arc::clone(&state.oauth);
    let secrets = Arc::clone(&state.secrets);
    state
        .executions
        .start(
            request,
            sink,
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
        .map(StartRequestExecutionOutput::from)
        .map_err(|error| BoundaryError::Execution(error).into())
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
        SetWorkspaceBaseDirectoryInput::export_to_string(&cfg)?,
        WorkspaceIdInput::export_to_string(&cfg)?,
        OrderedFieldDto::export_to_string(&cfg)?,
        BodyFilePathDto::export_to_string(&cfg)?,
        BodyFileReferenceDto::export_to_string(&cfg)?,
        DescribeBodyFileInput::export_to_string(&cfg)?,
        RelinkBodyFilesInput::export_to_string(&cfg)?,
        MultipartPartDto::export_to_string(&cfg)?,
        RequestBodyDto::export_to_string(&cfg)?,
        ApiKeyPlacementDto::export_to_string(&cfg)?,
        RequestAuthDto::export_to_string(&cfg)?,
        RedirectPolicyDto::export_to_string(&cfg)?,
        TlsPolicyDto::export_to_string(&cfg)?,
        ProxySourceDto::export_to_string(&cfg)?,
        ProxyPolicyDto::export_to_string(&cfg)?,
        TimeoutPolicyDto::export_to_string(&cfg)?,
        TransportPolicyDto::export_to_string(&cfg)?,
        RequestContentDto::export_to_string(&cfg)?,
        SavedRequestDto::export_to_string(&cfg)?,
        CollectionFolderDto::export_to_string(&cfg)?,
        EnvironmentDto::export_to_string(&cfg)?,
        VariableValueDto::export_to_string(&cfg)?,
        VariableDto::export_to_string(&cfg)?,
        CollectionVariableDto::export_to_string(&cfg)?,
        EnvironmentVariableDto::export_to_string(&cfg)?,
        RequestDraftDto::export_to_string(&cfg)?,
        RequestTabDto::export_to_string(&cfg)?,
        RequestWorkspaceSnapshotDto::export_to_string(&cfg)?,
        PostmanImportInput::export_to_string(&cfg)?,
        PostmanExportInput::export_to_string(&cfg)?,
        PostmanExportResultDto::export_to_string(&cfg)?,
        PostmanEnvironmentExportDto::export_to_string(&cfg)?,
        PostmanImportPreviewDto::export_to_string(&cfg)?,
        PostmanImportWarningDto::export_to_string(&cfg)?,
        PostmanUnsupportedFieldDto::export_to_string(&cfg)?,
        PostmanImportResultDto::export_to_string(&cfg)?,
        PostmanReimportPreviewDto::export_to_string(&cfg)?,
        PostmanPriorImportDto::export_to_string(&cfg)?,
        PostmanReimportChangeDto::export_to_string(&cfg)?,
        PostmanReimportDecisionDto::export_to_string(&cfg)?,
        PostmanReimportInput::export_to_string(&cfg)?,
        PostmanReimportResultDto::export_to_string(&cfg)?,
        NativeBackupExportInput::export_to_string(&cfg)?,
        NativeBackupRestorePreviewInput::export_to_string(&cfg)?,
        NativeBackupRestoreInput::export_to_string(&cfg)?,
        NativeBackupExportResultDto::export_to_string(&cfg)?,
        NativeBackupRestoreResultDto::export_to_string(&cfg)?,
        NativeBackupRestorePreviewDto::export_to_string(&cfg)?,
        NativeBackupExclusionDto::export_to_string(&cfg)?,
        NativeBackupManifestDto::export_to_string(&cfg)?,
        NativeBackupManifestEntryDto::export_to_string(&cfg)?,
        CurlImportInput::export_to_string(&cfg)?,
        CurlGenerateInput::export_to_string(&cfg)?,
        CurlImportPreviewDto::export_to_string(&cfg)?,
        CurlImportWarningDto::export_to_string(&cfg)?,
        CurlUnsupportedFieldDto::export_to_string(&cfg)?,
        CurlImportResultDto::export_to_string(&cfg)?,
        CurlGenerateResultDto::export_to_string(&cfg)?,
        ExecutionHistorySnapshotDto::export_to_string(&cfg)?,
        ExecutionRecordDto::export_to_string(&cfg)?,
        ExecutionRecordResponseDto::export_to_string(&cfg)?,
        CookieSameSiteDto::export_to_string(&cfg)?,
        WorkspaceCookieDto::export_to_string(&cfg)?,
        CookieJarSnapshotDto::export_to_string(&cfg)?,
        UpsertCookieInput::export_to_string(&cfg)?,
        CookieIdInput::export_to_string(&cfg)?,
        RevealCookieValueOutput::export_to_string(&cfg)?,
        CreateSavedRequestInput::export_to_string(&cfg)?,
        CollectionLocationDto::export_to_string(&cfg)?,
        SelectEnvironmentInput::export_to_string(&cfg)?,
        ResolveRequestContentInput::export_to_string(&cfg)?,
        ResolvedRequestContentDto::export_to_string(&cfg)?,
        ResolvedFieldDto::export_to_string(&cfg)?,
        ResolvedValueDto::export_to_string(&cfg)?,
        ResolvedVariableReferenceDto::export_to_string(&cfg)?,
        VariableSourceDto::export_to_string(&cfg)?,
        VariableResolutionErrorDto::export_to_string(&cfg)?,
        VariableResolutionErrorKindDto::export_to_string(&cfg)?,
        CreateCollectionFolderInput::export_to_string(&cfg)?,
        RenameCollectionFolderInput::export_to_string(&cfg)?,
        MoveCollectionFolderInput::export_to_string(&cfg)?,
        CollectionFolderIdInput::export_to_string(&cfg)?,
        MoveSavedRequestInput::export_to_string(&cfg)?,
        SavedRequestIdInput::export_to_string(&cfg)?,
        OpenSavedRequestTabInput::export_to_string(&cfg)?,
        UpdateRequestDraftInput::export_to_string(&cfg)?,
        RequestDraftIdInput::export_to_string(&cfg)?,
        CloseTabDecisionDto::export_to_string(&cfg)?,
        CloseRequestTabInput::export_to_string(&cfg)?,
        SetExecutionHistoryDisabledInput::export_to_string(&cfg)?,
        SetExecutionRecordPinnedInput::export_to_string(&cfg)?,
        ExecutionRecordIdInput::export_to_string(&cfg)?,
        StartRequestExecutionInput::export_to_string(&cfg)?,
        StartRequestExecutionOutput::export_to_string(&cfg)?,
        CancelRequestExecutionInput::export_to_string(&cfg)?,
        CancelRequestExecutionOutput::export_to_string(&cfg)?,
        StartOAuthAuthorizationInput::export_to_string(&cfg)?,
        OAuthAuthorizationResultDto::export_to_string(&cfg)?,
        CancelOAuthAuthorizationInput::export_to_string(&cfg)?,
        CancelOAuthAuthorizationOutput::export_to_string(&cfg)?,
        ExecutionEventDto::export_to_string(&cfg)?,
        ExecutionEventKindDto::export_to_string(&cfg)?,
        ExecutionHeaderDto::export_to_string(&cfg)?,
        ExecutionProxyMetadataDto::export_to_string(&cfg)?,
        ExecutionTimeoutMetadataDto::export_to_string(&cfg)?,
        ExecutionTimingMetadataDto::export_to_string(&cfg)?,
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
         \tset_workspace_base_directory: {\n\
         \t\tinput: SetWorkspaceBaseDirectoryInput;\n\
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
         \tcreate_collection_folder: {\n\
         \t\tinput: CreateCollectionFolderInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tselect_environment: {\n\
         \t\tinput: SelectEnvironmentInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tresolve_request_content: {\n\
         \t\tinput: ResolveRequestContentInput;\n\
         \t\toutput: ResolvedRequestContentDto;\n\
         \t};\n\
         \trename_collection_folder: {\n\
         \t\tinput: RenameCollectionFolderInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tmove_collection_folder: {\n\
         \t\tinput: MoveCollectionFolderInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tduplicate_collection_folder: {\n\
         \t\tinput: CollectionFolderIdInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tdelete_collection_folder: {\n\
         \t\tinput: CollectionFolderIdInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tmove_saved_request: {\n\
         \t\tinput: MoveSavedRequestInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tduplicate_saved_request: {\n\
         \t\tinput: SavedRequestIdInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tdelete_saved_request: {\n\
         \t\tinput: SavedRequestIdInput;\n\
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
         \tlist_execution_history: {\n\
         \t\tinput: WorkspaceIdInput;\n\
         \t\toutput: ExecutionHistorySnapshotDto;\n\
         \t};\n\
         \tset_execution_history_disabled: {\n\
         \t\tinput: SetExecutionHistoryDisabledInput;\n\
         \t\toutput: ExecutionHistorySnapshotDto;\n\
         \t};\n\
         \tset_execution_record_pinned: {\n\
         \t\tinput: SetExecutionRecordPinnedInput;\n\
         \t\toutput: ExecutionHistorySnapshotDto;\n\
         \t};\n\
         \topen_execution_record_as_draft: {\n\
         \t\tinput: ExecutionRecordIdInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tlist_cookies: {\n\
         \t\tinput: WorkspaceIdInput;\n\
         \t\toutput: CookieJarSnapshotDto;\n\
         \t};\n\
         \tupsert_cookie: {\n\
         \t\tinput: UpsertCookieInput;\n\
         \t\toutput: CookieJarSnapshotDto;\n\
         \t};\n\
         \tdelete_cookie: {\n\
         \t\tinput: CookieIdInput;\n\
         \t\toutput: CookieJarSnapshotDto;\n\
         \t};\n\
         \tclear_cookies: {\n\
         \t\tinput: WorkspaceIdInput;\n\
         \t\toutput: CookieJarSnapshotDto;\n\
         \t};\n\
         \treveal_cookie_value: {\n\
         \t\tinput: CookieIdInput;\n\
         \t\toutput: RevealCookieValueOutput;\n\
         \t};\n\
         \tdescribe_body_file: {\n\
         \t\tinput: DescribeBodyFileInput;\n\
         \t\toutput: BodyFileReferenceDto;\n\
         \t};\n\
         \trelink_body_files: {\n\
         \t\tinput: RelinkBodyFilesInput;\n\
         \t\toutput: RequestWorkspaceSnapshotDto;\n\
         \t};\n\
         \tpreview_postman_import: {\n\
         \t\tinput: PostmanImportInput;\n\
         \t\toutput: PostmanImportPreviewDto;\n\
         \t};\n\
         \timport_postman: {\n\
         \t\tinput: PostmanImportInput;\n\
         \t\toutput: PostmanImportResultDto;\n\
         \t};\n\
         \texport_postman: {\n\
         \t\tinput: PostmanExportInput;\n\
         \t\toutput: PostmanExportResultDto;\n\
         \t};\n\
         \tpreview_postman_reimport: {\n\
         \t\tinput: PostmanImportInput;\n\
         \t\toutput: PostmanReimportPreviewDto;\n\
         \t};\n\
         \treimport_postman: {\n\
         \t\tinput: PostmanReimportInput;\n\
         \t\toutput: PostmanReimportResultDto;\n\
         \t};\n\
         \texport_native_backup: {\n\
         \t\tinput: NativeBackupExportInput;\n\
         \t\toutput: NativeBackupExportResultDto;\n\
         \t};\n\
         \tpreview_native_backup_restore: {\n\
         \t\tinput: NativeBackupRestorePreviewInput;\n\
         \t\toutput: NativeBackupRestorePreviewDto;\n\
         \t};\n\
         \trestore_native_backup: {\n\
         \t\tinput: NativeBackupRestoreInput;\n\
         \t\toutput: NativeBackupRestoreResultDto;\n\
         \t};\n\
         \tpreview_curl_import: {\n\
         \t\tinput: CurlImportInput;\n\
         \t\toutput: CurlImportPreviewDto;\n\
         \t};\n\
         \timport_curl_as_draft: {\n\
         \t\tinput: CurlImportInput;\n\
         \t\toutput: CurlImportResultDto;\n\
         \t};\n\
         \tgenerate_curl: {\n\
         \t\tinput: CurlGenerateInput;\n\
         \t\toutput: CurlGenerateResultDto;\n\
         \t};\n\
         \tstart_request_execution: {\n\
         \t\tinput: StartRequestExecutionInput;\n\
         \t\toutput: StartRequestExecutionOutput;\n\
         \t};\n\
         \tcancel_request_execution: {\n\
         \t\tinput: CancelRequestExecutionInput;\n\
         \t\toutput: CancelRequestExecutionOutput;\n\
         \t};\n\
         \tstart_oauth_authorization: {\n\
         \t\tinput: StartOAuthAuthorizationInput;\n\
         \t\toutput: OAuthAuthorizationResultDto;\n\
         \t};\n\
         \tcancel_oauth_authorization: {\n\
         \t\tinput: CancelOAuthAuthorizationInput;\n\
         \t\toutput: CancelOAuthAuthorizationOutput;\n\
         \t};\n\
         };\n\n\
         export type IpcCommandContracts = WorkspaceCommandContracts & RequestCommandContracts;\n",
    );

    Ok(contract)
}

fn parse_workspace_id(value: &str) -> Result<WorkspaceId, IpcError> {
    WorkspaceId::from_str(value).map_err(|_| BoundaryError::InvalidWorkspaceId.into())
}

fn parse_collection_id(value: &str) -> Result<CollectionId, IpcError> {
    CollectionId::from_str(value).map_err(|_| BoundaryError::InvalidCollectionId.into())
}

fn parse_optional_collection_id(value: Option<String>) -> Result<Option<CollectionId>, IpcError> {
    value.as_deref().map(parse_collection_id).transpose()
}

fn parse_environment_id(value: &str) -> Result<EnvironmentId, IpcError> {
    EnvironmentId::from_str(value).map_err(|_| BoundaryError::InvalidEnvironmentId.into())
}

fn parse_optional_environment_id(value: Option<String>) -> Result<Option<EnvironmentId>, IpcError> {
    value.as_deref().map(parse_environment_id).transpose()
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

fn parse_oauth_flow_id(value: &str) -> Result<OAuthFlowId, IpcError> {
    OAuthFlowId::from_str(value).map_err(|_| BoundaryError::InvalidOAuthFlowId.into())
}

fn parse_execution_record_id(value: &str) -> Result<ExecutionRecordId, IpcError> {
    ExecutionRecordId::from_str(value).map_err(|_| BoundaryError::InvalidExecutionRecordId.into())
}

fn parse_cookie_id(value: &str) -> Result<CookieId, IpcError> {
    CookieId::from_str(value).map_err(|_| BoundaryError::InvalidCookieId.into())
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
                    base_directory: workspace.base_directory,
                })
                .collect(),
        }
    }
}

impl From<RequestWorkspaceSnapshot> for RequestWorkspaceSnapshotDto {
    fn from(snapshot: RequestWorkspaceSnapshot) -> Self {
        Self {
            workspace_id: snapshot.workspace_id.to_string(),
            collection_folders: snapshot
                .collection_folders
                .into_iter()
                .map(CollectionFolderDto::from)
                .collect(),
            environments: snapshot
                .environments
                .into_iter()
                .map(EnvironmentDto::from)
                .collect(),
            collection_variables: snapshot
                .collection_variables
                .into_iter()
                .map(CollectionVariableDto::from)
                .collect(),
            environment_variables: snapshot
                .environment_variables
                .into_iter()
                .map(EnvironmentVariableDto::from)
                .collect(),
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

impl TryFrom<PostmanImportInput> for ApplicationPostmanImportInput {
    type Error = IpcError;

    fn try_from(input: PostmanImportInput) -> Result<Self, Self::Error> {
        Ok(Self {
            workspace_id: parse_workspace_id(&input.workspace_id)?,
            source_name: input.source_name,
            collection_json: input.collection_json,
            environment_json: input.environment_json,
        })
    }
}

impl TryFrom<PostmanExportInput> for ApplicationPostmanExportInput {
    type Error = IpcError;

    fn try_from(input: PostmanExportInput) -> Result<Self, Self::Error> {
        Ok(Self {
            workspace_id: parse_workspace_id(&input.workspace_id)?,
            source_name: input.source_name,
        })
    }
}

impl From<PostmanExportResult> for PostmanExportResultDto {
    fn from(result: PostmanExportResult) -> Self {
        Self {
            collection_json: result.collection_json,
            environments: result
                .environments
                .into_iter()
                .map(PostmanEnvironmentExportDto::from)
                .collect(),
            warning_count: result.warning_count,
            unsupported_count: result.unsupported_count,
            warnings: result
                .warnings
                .into_iter()
                .map(PostmanImportWarningDto::from)
                .collect(),
            unsupported: result
                .unsupported
                .into_iter()
                .map(PostmanUnsupportedFieldDto::from)
                .collect(),
        }
    }
}

impl From<PostmanEnvironmentExport> for PostmanEnvironmentExportDto {
    fn from(environment: PostmanEnvironmentExport) -> Self {
        Self {
            name: environment.name,
            environment_json: environment.environment_json,
        }
    }
}

impl From<PostmanImportPreview> for PostmanImportPreviewDto {
    fn from(preview: PostmanImportPreview) -> Self {
        Self {
            source_id: preview.source_id,
            source_name: preview.source_name,
            source_hash: preview.source_hash,
            collection_count: preview.collection_count,
            request_count: preview.request_count,
            environment_count: preview.environment_count,
            warning_count: preview.warning_count,
            unsupported_count: preview.unsupported_count,
            warnings: preview
                .warnings
                .into_iter()
                .map(PostmanImportWarningDto::from)
                .collect(),
            unsupported: preview
                .unsupported
                .into_iter()
                .map(PostmanUnsupportedFieldDto::from)
                .collect(),
        }
    }
}

impl From<PostmanImportWarning> for PostmanImportWarningDto {
    fn from(warning: PostmanImportWarning) -> Self {
        Self {
            location: warning.location,
            message: warning.message,
        }
    }
}

impl From<PostmanUnsupportedField> for PostmanUnsupportedFieldDto {
    fn from(field: PostmanUnsupportedField) -> Self {
        Self {
            location: field.location,
            reason: field.reason,
        }
    }
}

impl From<PostmanImportResult> for PostmanImportResultDto {
    fn from(result: PostmanImportResult) -> Self {
        Self {
            preview: PostmanImportPreviewDto::from(result.preview),
            snapshot: RequestWorkspaceSnapshotDto::from(result.snapshot),
        }
    }
}

impl From<PostmanReimportPreview> for PostmanReimportPreviewDto {
    fn from(preview: PostmanReimportPreview) -> Self {
        Self {
            import_preview: PostmanImportPreviewDto::from(preview.import_preview),
            prior_import: preview.prior_import.map(PostmanPriorImportDto::from),
            changes: preview
                .changes
                .into_iter()
                .map(PostmanReimportChangeDto::from)
                .collect(),
            can_update: preview.can_update,
        }
    }
}

impl From<PostmanPriorImport> for PostmanPriorImportDto {
    fn from(prior: PostmanPriorImport) -> Self {
        Self {
            id: prior.id,
            source_id: prior.source_id,
            source_name: prior.source_name,
            source_hash: prior.source_hash,
        }
    }
}

impl From<PostmanReimportChange> for PostmanReimportChangeDto {
    fn from(change: PostmanReimportChange) -> Self {
        Self {
            location: change.location,
            message: change.message,
        }
    }
}

impl From<PostmanReimportDecisionDto> for PostmanReimportDecision {
    fn from(decision: PostmanReimportDecisionDto) -> Self {
        match decision {
            PostmanReimportDecisionDto::Update => Self::Update,
            PostmanReimportDecisionDto::Duplicate => Self::Duplicate,
            PostmanReimportDecisionDto::Cancel => Self::Cancel,
        }
    }
}

impl TryFrom<PostmanReimportInput> for ApplicationPostmanReimportInput {
    type Error = IpcError;

    fn try_from(input: PostmanReimportInput) -> Result<Self, Self::Error> {
        Ok(Self {
            import: ApplicationPostmanImportInput::try_from(input.import)?,
            decision: PostmanReimportDecision::from(input.decision),
        })
    }
}

impl From<PostmanReimportResult> for PostmanReimportResultDto {
    fn from(result: PostmanReimportResult) -> Self {
        Self {
            preview: PostmanReimportPreviewDto::from(result.preview),
            snapshot: RequestWorkspaceSnapshotDto::from(result.snapshot),
        }
    }
}

impl TryFrom<NativeBackupExportInput> for ApplicationNativeBackupExportInput {
    type Error = IpcError;

    fn try_from(input: NativeBackupExportInput) -> Result<Self, Self::Error> {
        Ok(Self {
            workspace_id: parse_workspace_id(&input.workspace_id)?,
            backup_path: input.backup_path,
            include_body_files: input.include_body_files,
            body_files_directory: input.body_files_directory,
        })
    }
}

impl From<NativeBackupExportResult> for NativeBackupExportResultDto {
    fn from(result: NativeBackupExportResult) -> Self {
        Self {
            backup_path: result.backup_path,
            manifest: NativeBackupManifestDto::from(result.manifest),
            preview: NativeBackupRestorePreviewDto::from(result.preview),
        }
    }
}

impl From<NativeBackupRestoreResult> for NativeBackupRestoreResultDto {
    fn from(result: NativeBackupRestoreResult) -> Self {
        Self {
            preview: NativeBackupRestorePreviewDto::from(result.preview),
            workspace_snapshot: WorkspaceSnapshotDto::from(result.workspace_snapshot),
            request_snapshot: RequestWorkspaceSnapshotDto::from(result.request_snapshot),
        }
    }
}

impl From<NativeBackupRestorePreview> for NativeBackupRestorePreviewDto {
    fn from(preview: NativeBackupRestorePreview) -> Self {
        Self {
            source_workspace_name: preview.source_workspace_name,
            collection_count: preview.collection_count,
            request_count: preview.request_count,
            environment_count: preview.environment_count,
            history_record_count: preview.history_record_count,
            cookie_count: preview.cookie_count,
            body_file_count: preview.body_file_count,
            expanded_bytes: preview.expanded_bytes,
            exclusions: preview
                .exclusions
                .into_iter()
                .map(NativeBackupExclusionDto::from)
                .collect(),
            warnings: preview.warnings,
        }
    }
}

impl From<NativeBackupExclusion> for NativeBackupExclusionDto {
    fn from(exclusion: NativeBackupExclusion) -> Self {
        Self {
            location: exclusion.location,
            reason: exclusion.reason,
        }
    }
}

impl From<NativeBackupManifest> for NativeBackupManifestDto {
    fn from(manifest: NativeBackupManifest) -> Self {
        Self {
            format: manifest.format,
            version: manifest.version,
            required_features: manifest.required_features,
            entries: manifest
                .entries
                .into_iter()
                .map(NativeBackupManifestEntryDto::from)
                .collect(),
            exclusions: manifest
                .exclusions
                .into_iter()
                .map(NativeBackupExclusionDto::from)
                .collect(),
        }
    }
}

impl From<NativeBackupManifestEntry> for NativeBackupManifestEntryDto {
    fn from(entry: NativeBackupManifestEntry) -> Self {
        Self {
            path: entry.path,
            sha256: entry.sha256,
            bytes: entry.bytes,
        }
    }
}

impl TryFrom<CurlImportInput> for ApplicationCurlImportInput {
    type Error = IpcError;

    fn try_from(input: CurlImportInput) -> Result<Self, Self::Error> {
        Ok(Self {
            workspace_id: parse_workspace_id(&input.workspace_id)?,
            source_name: input.source_name,
            command: input.command,
        })
    }
}

impl From<CurlGenerateInput> for ApplicationCurlGenerateInput {
    fn from(input: CurlGenerateInput) -> Self {
        let content = RequestContent::from(input.content);
        let resolved = input
            .resolved
            .map(|resolved| resolved_request_content_from_dto(resolved, &content));
        Self {
            content,
            resolved,
            include_secrets: input.include_secrets,
        }
    }
}

impl From<CurlImportPreview> for CurlImportPreviewDto {
    fn from(preview: CurlImportPreview) -> Self {
        Self {
            source_name: preview.source_name,
            content: RequestContentDto::from(preview.content),
            warning_count: preview.warning_count,
            unsupported_count: preview.unsupported_count,
            warnings: preview
                .warnings
                .into_iter()
                .map(CurlImportWarningDto::from)
                .collect(),
            unsupported: preview
                .unsupported
                .into_iter()
                .map(CurlUnsupportedFieldDto::from)
                .collect(),
        }
    }
}

impl From<CurlImportWarning> for CurlImportWarningDto {
    fn from(warning: CurlImportWarning) -> Self {
        Self {
            location: warning.location,
            message: warning.message,
        }
    }
}

impl From<CurlUnsupportedField> for CurlUnsupportedFieldDto {
    fn from(field: CurlUnsupportedField) -> Self {
        Self {
            location: field.location,
            reason: field.reason,
        }
    }
}

impl From<CurlGenerateResult> for CurlGenerateResultDto {
    fn from(result: CurlGenerateResult) -> Self {
        Self {
            command: result.command,
            included_secret_count: result.included_secret_count,
            redacted_secret_count: result.redacted_secret_count,
        }
    }
}

fn resolved_request_content_from_dto(
    resolved: ResolvedRequestContentDto,
    content: &RequestContent,
) -> ResolvedRequestContent {
    let body_kind = match &content.body {
        RequestBody::None => ResolvedRequestBody::None,
        RequestBody::Raw { .. } => ResolvedRequestBody::Raw {
            content: ResolvedValue::from(resolved.body.clone()),
        },
        RequestBody::UrlEncoded { fields } => ResolvedRequestBody::UrlEncoded {
            fields: fields
                .iter()
                .map(|field| {
                    let contains_secret = resolved.body.contains_secret;
                    ResolvedField {
                        enabled: field.enabled,
                        order: field.order,
                        name: ResolvedValue {
                            value: field.name.clone(),
                            contains_secret: false,
                        },
                        value: ResolvedValue {
                            value: if contains_secret {
                                REDACTED_VALUE.to_owned()
                            } else {
                                field.value.clone()
                            },
                            contains_secret,
                        },
                    }
                })
                .collect(),
        },
        RequestBody::Multipart { parts } => ResolvedRequestBody::Multipart {
            parts: parts
                .iter()
                .map(|part| match part {
                    MultipartPart::Field {
                        enabled,
                        order,
                        name,
                        value,
                    } => ResolvedMultipartPart::Field {
                        enabled: *enabled,
                        order: *order,
                        name: ResolvedValue {
                            value: name.clone(),
                            contains_secret: false,
                        },
                        value: ResolvedValue {
                            value: if resolved.body.contains_secret {
                                REDACTED_VALUE.to_owned()
                            } else {
                                value.clone()
                            },
                            contains_secret: resolved.body.contains_secret,
                        },
                    },
                    MultipartPart::File {
                        enabled,
                        order,
                        name,
                        ..
                    } => ResolvedMultipartPart::File {
                        enabled: *enabled,
                        order: *order,
                        name: ResolvedValue {
                            value: name.clone(),
                            contains_secret: false,
                        },
                    },
                })
                .collect(),
        },
        RequestBody::Binary { .. } => ResolvedRequestBody::Binary,
    };
    ResolvedRequestContent {
        url: ResolvedValue::from(resolved.url),
        body: ResolvedValue::from(resolved.body),
        body_kind,
        query: resolved
            .query
            .into_iter()
            .map(ResolvedField::from)
            .collect(),
        headers: resolved
            .headers
            .into_iter()
            .map(ResolvedField::from)
            .collect(),
        unsafe_tls_visible: resolved.unsafe_tls_visible,
        references: resolved
            .references
            .into_iter()
            .map(ResolvedVariableReference::from)
            .collect(),
        errors: resolved
            .errors
            .into_iter()
            .map(VariableResolutionError::from)
            .collect(),
    }
}

impl From<ExecutionHistorySnapshot> for ExecutionHistorySnapshotDto {
    fn from(snapshot: ExecutionHistorySnapshot) -> Self {
        Self {
            workspace_id: snapshot.workspace_id.to_string(),
            disabled: snapshot.disabled,
            records: snapshot
                .records
                .into_iter()
                .map(ExecutionRecordDto::from)
                .collect(),
            warning: snapshot.warning,
        }
    }
}

impl From<CookieJarSnapshot> for CookieJarSnapshotDto {
    fn from(snapshot: CookieJarSnapshot) -> Self {
        Self {
            workspace_id: snapshot.workspace_id.to_string(),
            cookies: snapshot
                .cookies
                .into_iter()
                .map(WorkspaceCookieDto::from)
                .collect(),
        }
    }
}

impl From<WorkspaceCookie> for WorkspaceCookieDto {
    fn from(cookie: WorkspaceCookie) -> Self {
        Self {
            id: cookie.id.to_string(),
            workspace_id: cookie.workspace_id.to_string(),
            name: cookie.name,
            domain: cookie.domain,
            path: cookie.path,
            secure: cookie.secure,
            http_only: cookie.http_only,
            same_site: cookie.same_site.map(CookieSameSiteDto::from),
            expires_at_epoch_seconds: cookie.expires_at_epoch_seconds,
            session: cookie.session,
            has_value: cookie.has_value,
            value_preview: REDACTED_VALUE.to_owned(),
        }
    }
}

impl From<CookieSameSite> for CookieSameSiteDto {
    fn from(value: CookieSameSite) -> Self {
        match value {
            CookieSameSite::Strict => Self::Strict,
            CookieSameSite::Lax => Self::Lax,
            CookieSameSite::None => Self::None,
        }
    }
}

impl From<CookieSameSiteDto> for CookieSameSite {
    fn from(value: CookieSameSiteDto) -> Self {
        match value {
            CookieSameSiteDto::Strict => Self::Strict,
            CookieSameSiteDto::Lax => Self::Lax,
            CookieSameSiteDto::None => Self::None,
        }
    }
}

impl TryFrom<UpsertCookieInput> for CookieDraft {
    type Error = IpcError;

    fn try_from(input: UpsertCookieInput) -> Result<Self, Self::Error> {
        Ok(Self {
            id: input
                .cookie_id
                .as_deref()
                .map(parse_cookie_id)
                .transpose()?,
            workspace_id: parse_workspace_id(&input.workspace_id)?,
            name: input.name,
            value: input.value,
            domain: input.domain,
            path: input.path,
            secure: input.secure,
            http_only: input.http_only,
            same_site: input.same_site.map(CookieSameSite::from),
            expires_at_epoch_seconds: input.expires_at_epoch_seconds,
        })
    }
}

impl From<ExecutionRecord> for ExecutionRecordDto {
    fn from(record: ExecutionRecord) -> Self {
        Self {
            id: record.id.to_string(),
            workspace_id: record.workspace_id.to_string(),
            created_at_epoch_seconds: record.created_at_epoch_seconds,
            request: RequestContentDto::from(record.request),
            response: ExecutionRecordResponseDto::from(record.response),
            pinned: record.pinned,
        }
    }
}

impl From<ExecutionRecordResponse> for ExecutionRecordResponseDto {
    fn from(response: ExecutionRecordResponse) -> Self {
        Self {
            status: response.status,
            headers: response
                .headers
                .into_iter()
                .map(OrderedFieldDto::from)
                .collect(),
            body_preview: response.body_preview,
            body_truncated: response.body_truncated,
            error: response.error,
            duration_ms: response.duration_ms,
        }
    }
}

impl From<SavedRequest> for SavedRequestDto {
    fn from(request: SavedRequest) -> Self {
        Self {
            id: request.id.to_string(),
            workspace_id: request.workspace_id.to_string(),
            collection_id: request.collection_id.map(|id| id.to_string()),
            position: request.position,
            content: RequestContentDto::from(request.content),
        }
    }
}

impl From<CollectionFolder> for CollectionFolderDto {
    fn from(folder: CollectionFolder) -> Self {
        Self {
            id: folder.id.to_string(),
            workspace_id: folder.workspace_id.to_string(),
            parent_collection_id: folder.parent_collection_id.map(|id| id.to_string()),
            name: folder.name,
            position: folder.position,
        }
    }
}

impl From<Environment> for EnvironmentDto {
    fn from(environment: Environment) -> Self {
        Self {
            id: environment.id.to_string(),
            workspace_id: environment.workspace_id.to_string(),
            name: environment.name,
            position: environment.position,
            is_selected: environment.is_selected,
        }
    }
}

impl From<CollectionVariable> for CollectionVariableDto {
    fn from(variable: CollectionVariable) -> Self {
        Self {
            workspace_id: variable.workspace_id.to_string(),
            variable: VariableDto::from(variable.variable),
        }
    }
}

impl From<EnvironmentVariable> for EnvironmentVariableDto {
    fn from(variable: EnvironmentVariable) -> Self {
        Self {
            environment_id: variable.environment_id.to_string(),
            workspace_id: variable.workspace_id.to_string(),
            variable: VariableDto::from(variable.variable),
        }
    }
}

impl From<Variable> for VariableDto {
    fn from(variable: Variable) -> Self {
        Self {
            name: variable.name,
            value: VariableValueDto::from(variable.value),
        }
    }
}

impl From<VariableValue> for VariableValueDto {
    fn from(value: VariableValue) -> Self {
        match value {
            VariableValue::Plain(value) => Self::Plain { value },
            VariableValue::SecretReference(reference) => Self::SecretReference { reference },
        }
    }
}

impl TryFrom<CollectionLocationDto> for CollectionLocation {
    type Error = IpcError;

    fn try_from(location: CollectionLocationDto) -> Result<Self, Self::Error> {
        Ok(Self {
            collection_id: parse_optional_collection_id(location.collection_id)?,
            position: location.position,
        })
    }
}

impl From<ResolvedRequestContent> for ResolvedRequestContentDto {
    fn from(content: ResolvedRequestContent) -> Self {
        Self {
            url: ResolvedValueDto::from(content.url),
            body: ResolvedValueDto::from(content.body),
            query: content
                .query
                .into_iter()
                .map(ResolvedFieldDto::from)
                .collect(),
            headers: content
                .headers
                .into_iter()
                .map(ResolvedFieldDto::from)
                .collect(),
            unsafe_tls_visible: content.unsafe_tls_visible,
            references: content
                .references
                .into_iter()
                .map(ResolvedVariableReferenceDto::from)
                .collect(),
            errors: content
                .errors
                .into_iter()
                .map(VariableResolutionErrorDto::from)
                .collect(),
        }
    }
}

impl From<ResolvedField> for ResolvedFieldDto {
    fn from(field: ResolvedField) -> Self {
        Self {
            enabled: field.enabled,
            order: field.order,
            name: ResolvedValueDto::from(field.name),
            value: ResolvedValueDto::from(field.value),
        }
    }
}

impl From<ResolvedFieldDto> for ResolvedField {
    fn from(field: ResolvedFieldDto) -> Self {
        Self {
            enabled: field.enabled,
            order: field.order,
            name: ResolvedValue::from(field.name),
            value: ResolvedValue::from(field.value),
        }
    }
}

impl From<ResolvedValue> for ResolvedValueDto {
    fn from(value: ResolvedValue) -> Self {
        Self {
            value: value.value,
            contains_secret: value.contains_secret,
        }
    }
}

impl From<ResolvedValueDto> for ResolvedValue {
    fn from(value: ResolvedValueDto) -> Self {
        Self {
            value: value.value,
            contains_secret: value.contains_secret,
        }
    }
}

impl From<ResolvedVariableReference> for ResolvedVariableReferenceDto {
    fn from(reference: ResolvedVariableReference) -> Self {
        Self {
            name: reference.name,
            source: VariableSourceDto::from(reference.source),
            value: ResolvedValueDto::from(reference.value),
        }
    }
}

impl From<ResolvedVariableReferenceDto> for ResolvedVariableReference {
    fn from(reference: ResolvedVariableReferenceDto) -> Self {
        Self {
            name: reference.name,
            source: VariableSource::from(reference.source),
            value: ResolvedValue::from(reference.value),
        }
    }
}

impl From<VariableSource> for VariableSourceDto {
    fn from(source: VariableSource) -> Self {
        match source {
            VariableSource::Collection => Self::Collection,
            VariableSource::Environment => Self::Environment,
        }
    }
}

impl From<VariableSourceDto> for VariableSource {
    fn from(source: VariableSourceDto) -> Self {
        match source {
            VariableSourceDto::Collection => Self::Collection,
            VariableSourceDto::Environment => Self::Environment,
        }
    }
}

impl From<VariableResolutionError> for VariableResolutionErrorDto {
    fn from(error: VariableResolutionError) -> Self {
        Self {
            name: error.name,
            kind: VariableResolutionErrorKindDto::from(error.kind),
        }
    }
}

impl From<VariableResolutionErrorDto> for VariableResolutionError {
    fn from(error: VariableResolutionErrorDto) -> Self {
        Self {
            name: error.name,
            kind: VariableResolutionErrorKind::from(error.kind),
        }
    }
}

impl From<VariableResolutionErrorKind> for VariableResolutionErrorKindDto {
    fn from(kind: VariableResolutionErrorKind) -> Self {
        match kind {
            VariableResolutionErrorKind::Missing => Self::Missing,
            VariableResolutionErrorKind::Cycle => Self::Cycle,
        }
    }
}

impl From<VariableResolutionErrorKindDto> for VariableResolutionErrorKind {
    fn from(kind: VariableResolutionErrorKindDto) -> Self {
        match kind {
            VariableResolutionErrorKindDto::Missing => Self::Missing,
            VariableResolutionErrorKindDto::Cycle => Self::Cycle,
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
            body: RequestBodyDto::from(content.body),
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
            auth: RequestAuthDto::from(content.auth),
            redirect: RedirectPolicyDto::from(content.redirect),
            tls: TlsPolicyDto::from(content.tls),
            transport: TransportPolicyDto::from(content.transport),
        }
    }
}

impl From<RequestContentDto> for RequestContent {
    fn from(content: RequestContentDto) -> Self {
        Self {
            name: content.name,
            method: content.method,
            url: content.url,
            body: RequestBody::from(content.body),
            query: content.query.into_iter().map(OrderedField::from).collect(),
            headers: content
                .headers
                .into_iter()
                .map(OrderedField::from)
                .collect(),
            auth: RequestAuth::from(content.auth),
            redirect: RedirectPolicy::from(content.redirect),
            tls: TlsPolicy::from(content.tls),
            transport: TransportPolicy::from(content.transport),
        }
    }
}

impl From<RequestAuth> for RequestAuthDto {
    fn from(auth: RequestAuth) -> Self {
        match auth {
            RequestAuth::None => Self::None,
            RequestAuth::Basic { username, password } => Self::Basic { username, password },
            RequestAuth::Bearer { token } => Self::Bearer { token },
            RequestAuth::ApiKey {
                placement,
                name,
                value,
            } => Self::ApiKey {
                placement: ApiKeyPlacementDto::from(placement),
                name,
                value,
            },
            RequestAuth::ClientCredentials {
                token_endpoint,
                client_id,
                client_secret,
                scopes,
            } => Self::ClientCredentials {
                token_endpoint,
                client_id,
                client_secret,
                scopes,
            },
        }
    }
}

impl From<RequestAuthDto> for RequestAuth {
    fn from(auth: RequestAuthDto) -> Self {
        match auth {
            RequestAuthDto::None => Self::None,
            RequestAuthDto::Basic { username, password } => Self::Basic { username, password },
            RequestAuthDto::Bearer { token } => Self::Bearer { token },
            RequestAuthDto::ApiKey {
                placement,
                name,
                value,
            } => Self::ApiKey {
                placement: ApiKeyPlacement::from(placement),
                name,
                value,
            },
            RequestAuthDto::ClientCredentials {
                token_endpoint,
                client_id,
                client_secret,
                scopes,
            } => Self::ClientCredentials {
                token_endpoint,
                client_id,
                client_secret,
                scopes,
            },
        }
    }
}

impl From<ApiKeyPlacement> for ApiKeyPlacementDto {
    fn from(placement: ApiKeyPlacement) -> Self {
        match placement {
            ApiKeyPlacement::Header => Self::Header,
            ApiKeyPlacement::Query => Self::Query,
        }
    }
}

impl From<ApiKeyPlacementDto> for ApiKeyPlacement {
    fn from(placement: ApiKeyPlacementDto) -> Self {
        match placement {
            ApiKeyPlacementDto::Header => Self::Header,
            ApiKeyPlacementDto::Query => Self::Query,
        }
    }
}

impl From<RedirectPolicy> for RedirectPolicyDto {
    fn from(policy: RedirectPolicy) -> Self {
        Self {
            enabled: policy.enabled,
            max_redirects: policy.max_redirects,
        }
    }
}

impl From<RedirectPolicyDto> for RedirectPolicy {
    fn from(policy: RedirectPolicyDto) -> Self {
        Self {
            enabled: policy.enabled,
            max_redirects: policy.max_redirects,
        }
    }
}

impl From<TlsPolicy> for TlsPolicyDto {
    fn from(policy: TlsPolicy) -> Self {
        Self {
            verify: policy.verify,
            custom_ca_reference: policy.custom_ca_reference,
            client_certificate_reference: policy.client_certificate_reference,
            client_key_reference: policy.client_key_reference,
        }
    }
}

impl From<TlsPolicyDto> for TlsPolicy {
    fn from(policy: TlsPolicyDto) -> Self {
        Self {
            verify: policy.verify,
            custom_ca_reference: policy.custom_ca_reference,
            client_certificate_reference: policy.client_certificate_reference,
            client_key_reference: policy.client_key_reference,
        }
    }
}

impl From<TransportPolicy> for TransportPolicyDto {
    fn from(policy: TransportPolicy) -> Self {
        Self {
            proxy: ProxyPolicyDto::from(policy.proxy),
            timeouts: TimeoutPolicyDto::from(policy.timeouts),
        }
    }
}

impl From<TransportPolicyDto> for TransportPolicy {
    fn from(policy: TransportPolicyDto) -> Self {
        Self {
            proxy: ProxyPolicy::from(policy.proxy),
            timeouts: TimeoutPolicy::from(policy.timeouts),
        }
    }
}

impl From<ProxyPolicy> for ProxyPolicyDto {
    fn from(policy: ProxyPolicy) -> Self {
        Self {
            source: ProxySourceDto::from(policy.source),
            url: policy.url,
            no_proxy: policy.no_proxy,
        }
    }
}

impl From<ProxyPolicyDto> for ProxyPolicy {
    fn from(policy: ProxyPolicyDto) -> Self {
        Self {
            source: ProxySource::from(policy.source),
            url: policy.url,
            no_proxy: policy.no_proxy,
        }
    }
}

impl From<ProxySource> for ProxySourceDto {
    fn from(source: ProxySource) -> Self {
        match source {
            ProxySource::Disabled => Self::Disabled,
            ProxySource::ProcessEnvironment => Self::ProcessEnvironment,
            ProxySource::Custom => Self::Custom,
        }
    }
}

impl From<ProxySourceDto> for ProxySource {
    fn from(source: ProxySourceDto) -> Self {
        match source {
            ProxySourceDto::Disabled => Self::Disabled,
            ProxySourceDto::ProcessEnvironment => Self::ProcessEnvironment,
            ProxySourceDto::Custom => Self::Custom,
        }
    }
}

impl From<TimeoutPolicy> for TimeoutPolicyDto {
    fn from(policy: TimeoutPolicy) -> Self {
        Self {
            connect_ms: policy.connect_ms,
            overall_ms: policy.overall_ms,
            idle_ms: policy.idle_ms,
        }
    }
}

impl From<TimeoutPolicyDto> for TimeoutPolicy {
    fn from(policy: TimeoutPolicyDto) -> Self {
        Self {
            connect_ms: policy.connect_ms,
            overall_ms: policy.overall_ms,
            idle_ms: policy.idle_ms,
        }
    }
}

impl From<RequestBody> for RequestBodyDto {
    fn from(body: RequestBody) -> Self {
        match body {
            RequestBody::None => Self::None,
            RequestBody::Raw { content } => Self::Raw { content },
            RequestBody::UrlEncoded { fields } => Self::UrlEncoded {
                fields: fields.into_iter().map(OrderedFieldDto::from).collect(),
            },
            RequestBody::Multipart { parts } => Self::Multipart {
                parts: parts.into_iter().map(MultipartPartDto::from).collect(),
            },
            RequestBody::Binary { file } => Self::Binary {
                file: BodyFileReferenceDto::from(file),
            },
        }
    }
}

impl From<RequestBodyDto> for RequestBody {
    fn from(body: RequestBodyDto) -> Self {
        match body {
            RequestBodyDto::None => Self::None,
            RequestBodyDto::Raw { content } => Self::Raw { content },
            RequestBodyDto::UrlEncoded { fields } => Self::UrlEncoded {
                fields: fields.into_iter().map(OrderedField::from).collect(),
            },
            RequestBodyDto::Multipart { parts } => Self::Multipart {
                parts: parts.into_iter().map(MultipartPart::from).collect(),
            },
            RequestBodyDto::Binary { file } => Self::Binary {
                file: BodyFileReference::from(file),
            },
        }
    }
}

impl From<MultipartPart> for MultipartPartDto {
    fn from(part: MultipartPart) -> Self {
        match part {
            MultipartPart::Field {
                enabled,
                order,
                name,
                value,
            } => Self::Field {
                enabled,
                order,
                name,
                value,
            },
            MultipartPart::File {
                enabled,
                order,
                name,
                file,
            } => Self::File {
                enabled,
                order,
                name,
                file: BodyFileReferenceDto::from(file),
            },
        }
    }
}

impl From<MultipartPartDto> for MultipartPart {
    fn from(part: MultipartPartDto) -> Self {
        match part {
            MultipartPartDto::Field {
                enabled,
                order,
                name,
                value,
            } => Self::Field {
                enabled,
                order,
                name,
                value,
            },
            MultipartPartDto::File {
                enabled,
                order,
                name,
                file,
            } => Self::File {
                enabled,
                order,
                name,
                file: BodyFileReference::from(file),
            },
        }
    }
}

impl From<BodyFileReference> for BodyFileReferenceDto {
    fn from(file: BodyFileReference) -> Self {
        Self {
            path: BodyFilePathDto::from(file.path),
            file_name: file.file_name,
            size: file.size,
            modified_at_epoch_seconds: file.modified_at_epoch_seconds,
            sha256: file.sha256,
        }
    }
}

impl From<BodyFileReferenceDto> for BodyFileReference {
    fn from(file: BodyFileReferenceDto) -> Self {
        Self {
            path: BodyFilePath::from(file.path),
            file_name: file.file_name,
            size: file.size,
            modified_at_epoch_seconds: file.modified_at_epoch_seconds,
            sha256: file.sha256,
        }
    }
}

impl From<BodyFilePath> for BodyFilePathDto {
    fn from(path: BodyFilePath) -> Self {
        match path {
            BodyFilePath::Relative { path } => Self::Relative { path },
            BodyFilePath::Absolute { path } => Self::Absolute { path },
        }
    }
}

impl From<BodyFilePathDto> for BodyFilePath {
    fn from(path: BodyFilePathDto) -> Self {
        match path {
            BodyFilePathDto::Relative { path } => Self::Relative { path },
            BodyFilePathDto::Absolute { path } => Self::Absolute { path },
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

impl TryFrom<StartOAuthAuthorizationInput> for StartOAuthAuthorizationRequest {
    type Error = IpcError;

    fn try_from(input: StartOAuthAuthorizationInput) -> Result<Self, Self::Error> {
        Ok(Self {
            flow_id: parse_oauth_flow_id(&input.flow_id)?,
            authorization_endpoint: input.authorization_endpoint,
            client_id: input.client_id,
            scopes: input.scopes,
            redirect_path: input.redirect_path,
            timeout_ms: input.timeout_ms,
        })
    }
}

impl From<OAuthAuthorizationResult> for OAuthAuthorizationResultDto {
    fn from(result: OAuthAuthorizationResult) -> Self {
        Self {
            flow_id: result.flow_id.to_string(),
            redirect_uri: result.redirect_uri,
            code: result.code,
            state: result.state,
            error: result.error,
            error_description: result.error_description,
        }
    }
}

impl From<CancelOAuthAuthorizationResult> for CancelOAuthAuthorizationOutput {
    fn from(result: CancelOAuthAuthorizationResult) -> Self {
        Self {
            flow_id: result.flow_id.to_string(),
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
            ExecutionEventKind::Started {
                method,
                url,
                tls_verification,
                proxy,
                timeouts,
                queued_ms,
            } => Self::Started {
                method,
                url,
                tls_verification,
                proxy: ExecutionProxyMetadataDto::from(proxy),
                timeouts: ExecutionTimeoutMetadataDto::from(timeouts),
                queued_ms,
            },
            ExecutionEventKind::Redirected { from, to, status } => {
                Self::Redirected { from, to, status }
            }
            ExecutionEventKind::UploadProgress {
                sent_bytes,
                total_bytes,
            } => Self::UploadProgress {
                sent_bytes,
                total_bytes,
            },
            ExecutionEventKind::ResponseHeaders {
                status,
                headers,
                protocol,
                remote_addr,
            } => Self::ResponseHeaders {
                status,
                headers: headers.into_iter().map(ExecutionHeaderDto::from).collect(),
                protocol,
                remote_addr,
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
                decoded_bytes,
                wire_bytes,
                timing,
            } => Self::Completed {
                status,
                body_preview,
                body_truncated,
                decoded_bytes,
                wire_bytes,
                timing: ExecutionTimingMetadataDto::from(timing),
            },
            ExecutionEventKind::Failed { message } => Self::Failed { message },
            ExecutionEventKind::Cancelled => Self::Cancelled,
        }
    }
}

impl From<ExecutionProxyMetadata> for ExecutionProxyMetadataDto {
    fn from(metadata: ExecutionProxyMetadata) -> Self {
        Self {
            source: metadata.source,
            selected_proxy: metadata.selected_proxy,
            bypass_reason: metadata.bypass_reason,
        }
    }
}

impl From<ExecutionTimeoutMetadata> for ExecutionTimeoutMetadataDto {
    fn from(metadata: ExecutionTimeoutMetadata) -> Self {
        Self {
            connect_ms: metadata.connect_ms,
            overall_ms: metadata.overall_ms,
            idle_ms: metadata.idle_ms,
        }
    }
}

impl From<ExecutionTimingMetadata> for ExecutionTimingMetadataDto {
    fn from(metadata: ExecutionTimingMetadata) -> Self {
        Self {
            queued_ms: metadata.queued_ms,
            dns_ms: metadata.dns_ms,
            connect_ms: metadata.connect_ms,
            tls_ms: metadata.tls_ms,
            first_byte_ms: metadata.first_byte_ms,
            download_ms: metadata.download_ms,
            total_ms: metadata.total_ms,
        }
    }
}

impl From<ExecutionHeader> for ExecutionHeaderDto {
    fn from(header: ExecutionHeader) -> Self {
        let value = if header.name.eq_ignore_ascii_case("set-cookie") {
            REDACTED_VALUE.to_owned()
        } else {
            header.value
        };
        Self {
            name: header.name,
            value,
        }
    }
}

impl From<BoundaryError> for IpcError {
    fn from(error: BoundaryError) -> Self {
        match error {
            BoundaryError::Workspace(error) => error.into(),
            BoundaryError::Request(error) => error.into(),
            BoundaryError::Execution(error) => error.into(),
            BoundaryError::OAuth(error) => error.into(),
            BoundaryError::NativeBackup(error) => error.into(),
            BoundaryError::InvalidWorkspaceId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Workspace id is invalid.".to_owned(),
                details: Some("workspaceId".to_owned()),
                retryable: false,
            },
            BoundaryError::InvalidCollectionId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Collection id is invalid.".to_owned(),
                details: Some("collectionId".to_owned()),
                retryable: false,
            },
            BoundaryError::InvalidEnvironmentId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Environment id is invalid.".to_owned(),
                details: Some("environmentId".to_owned()),
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
            BoundaryError::InvalidOAuthFlowId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "OAuth flow id is invalid.".to_owned(),
                details: Some("flowId".to_owned()),
                retryable: false,
            },
            BoundaryError::InvalidExecutionRecordId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Execution record id is invalid.".to_owned(),
                details: Some("recordId".to_owned()),
                retryable: false,
            },
            BoundaryError::InvalidCookieId => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Cookie id is invalid.".to_owned(),
                details: Some("cookieId".to_owned()),
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

impl From<OAuthError> for IpcError {
    fn from(error: OAuthError) -> Self {
        match error {
            OAuthError::InvalidInput(detail) => Self {
                code: IpcErrorCode::InvalidInput,
                message: "OAuth input is invalid.".to_owned(),
                details: Some(detail.to_owned()),
                retryable: false,
            },
            OAuthError::ListenerFailed => Self {
                code: IpcErrorCode::StateUnavailable,
                message: "OAuth callback listener is unavailable.".to_owned(),
                details: None,
                retryable: true,
            },
            OAuthError::Timeout => Self {
                code: IpcErrorCode::InvalidInput,
                message: "OAuth authorization timed out.".to_owned(),
                details: Some("oauth.timeout".to_owned()),
                retryable: false,
            },
            OAuthError::Cancelled => Self {
                code: IpcErrorCode::InvalidInput,
                message: "OAuth authorization was cancelled.".to_owned(),
                details: Some("oauth.cancelled".to_owned()),
                retryable: false,
            },
            OAuthError::StateMismatch => Self {
                code: IpcErrorCode::InvalidInput,
                message: "OAuth state did not match.".to_owned(),
                details: Some("oauth.state.mismatch".to_owned()),
                retryable: false,
            },
            OAuthError::MissingCode => Self {
                code: IpcErrorCode::InvalidInput,
                message: "OAuth authorization code is missing.".to_owned(),
                details: Some("oauth.code.required".to_owned()),
                retryable: false,
            },
            OAuthError::BrowserOpenFailed => Self {
                code: IpcErrorCode::StateUnavailable,
                message: "System browser could not be opened.".to_owned(),
                details: None,
                retryable: true,
            },
            OAuthError::StateUnavailable => Self {
                code: IpcErrorCode::StateUnavailable,
                message: "OAuth state is temporarily unavailable.".to_owned(),
                details: None,
                retryable: true,
            },
            OAuthError::TokenRequestFailed => Self {
                code: IpcErrorCode::StateUnavailable,
                message: "OAuth token request failed.".to_owned(),
                details: Some("oauth.token.requestFailed".to_owned()),
                retryable: true,
            },
            OAuthError::InvalidTokenResponse => Self {
                code: IpcErrorCode::InvalidInput,
                message: "OAuth token response is invalid.".to_owned(),
                details: Some("oauth.token.response.invalid".to_owned()),
                retryable: false,
            },
            OAuthError::RefreshRequired => Self {
                code: IpcErrorCode::InvalidInput,
                message: "OAuth reauthorization is required.".to_owned(),
                details: Some("oauth.refresh.required".to_owned()),
                retryable: false,
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

impl From<PostmanImportError> for IpcError {
    fn from(error: PostmanImportError) -> Self {
        match error {
            PostmanImportError::InvalidInput(detail) => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Postman import input is invalid.".to_owned(),
                details: Some(detail),
                retryable: false,
            },
            PostmanImportError::WorkspaceNotFound => Self {
                code: IpcErrorCode::WorkspaceNotFound,
                message: "Workspace was not found.".to_owned(),
                details: None,
                retryable: false,
            },
            PostmanImportError::Persistence(_) => Self {
                code: IpcErrorCode::PersistenceUnavailable,
                message: "Postman import persistence is unavailable.".to_owned(),
                details: None,
                retryable: true,
            },
            PostmanImportError::Secret(detail) => Self {
                code: IpcErrorCode::InvalidInput,
                message: "Secret storage is unavailable for Postman import.".to_owned(),
                details: Some(detail),
                retryable: false,
            },
        }
    }
}

impl From<NativeBackupError> for IpcError {
    fn from(error: NativeBackupError) -> Self {
        match error {
            NativeBackupError::InvalidInput(detail) | NativeBackupError::InvalidArchive(detail) => {
                Self {
                    code: IpcErrorCode::InvalidInput,
                    message: "Native backup input is invalid.".to_owned(),
                    details: Some(detail),
                    retryable: false,
                }
            }
            NativeBackupError::WorkspaceNotFound => Self {
                code: IpcErrorCode::WorkspaceNotFound,
                message: "Workspace was not found.".to_owned(),
                details: None,
                retryable: false,
            },
            NativeBackupError::WorkspaceAlreadyExists => Self {
                code: IpcErrorCode::WorkspaceAlreadyExists,
                message: "Workspace name already exists.".to_owned(),
                details: None,
                retryable: false,
            },
            NativeBackupError::Persistence(_) => Self {
                code: IpcErrorCode::PersistenceUnavailable,
                message: "Native backup persistence is unavailable.".to_owned(),
                details: None,
                retryable: true,
            },
        }
    }
}

impl From<CurlError> for IpcError {
    fn from(error: CurlError) -> Self {
        match error {
            CurlError::InvalidInput(detail) => Self {
                code: IpcErrorCode::InvalidInput,
                message: "cURL input is invalid.".to_owned(),
                details: Some(detail),
                retryable: false,
            },
            CurlError::Request(error) => IpcError::from(error),
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
        application::{
            oauth::OAuthError,
            request::RequestError,
            workspace::{WorkspaceRepository, WorkspaceSummary},
        },
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
                        base_directory: None,
                    }],
                },
                next_error: None,
                calls: Vec::new(),
            }
        }

        fn with_two_workspaces() -> Self {
            let mut repository = Self::new();
            let workspace = Workspace::new(WorkspaceName::new("Client").expect("valid name"));
            repository.snapshot.workspaces.push(WorkspaceSummary {
                id: workspace.id,
                name: workspace.name,
                is_selected: false,
                base_directory: None,
            });
            repository
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

        fn set_workspace_base_directory(
            &mut self,
            _id: WorkspaceId,
            _base_directory: Option<String>,
        ) -> Result<WorkspaceSnapshot, WorkspaceError> {
            self.result("set_base_directory")
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
                    "isSelected": true,
                    "baseDirectory": null
                }]
            })
        );
    }

    #[test]
    fn invalid_workspace_id_maps_to_safe_non_retryable_error() {
        let service = Mutex::new(WorkspaceService::new_for_test(
            FakeWorkspaceRepository::new(),
        ));

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
    fn oauth_and_request_errors_do_not_serialize_fixture_secrets() {
        let fixture_secrets = [
            fixture_secret("OAUTH_CODE"),
            fixture_secret("OAUTH_ACCESS_TOKEN"),
            fixture_secret("OAUTH_REFRESH_TOKEN"),
            fixture_secret("OAUTH_CLIENT_SECRET"),
            fixture_secret("OAUTH_CALLBACK_STATE"),
            fixture_secret("BASIC_PASSWORD"),
        ];
        let errors = [
            IpcError::from(OAuthError::TokenRequestFailed),
            IpcError::from(OAuthError::InvalidTokenResponse),
            IpcError::from(OAuthError::RefreshRequired),
            IpcError::from(RequestError::Persistence(format!(
                "database failed near {}",
                fixture_secrets[5].as_str()
            ))),
        ];

        for error in errors {
            let serialized = serde_json::to_string(&error).expect("serialize error");
            for fixture_secret in &fixture_secrets {
                assert!(!serialized.contains(fixture_secret));
            }
        }
    }

    fn fixture_secret(name: &str) -> String {
        ["POSTMITE", "SECRET", name, "29"].join("_")
    }

    #[test]
    fn commands_delegate_to_workspace_service() {
        let service = Mutex::new(WorkspaceService::new_for_test(
            FakeWorkspaceRepository::with_two_workspaces(),
        ));
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
        assert_eq!(created.workspaces.len(), 2);

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
