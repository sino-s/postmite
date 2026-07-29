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
        ExecutionTimingMetadata, ResponseFileMetadata, StartExecutionResult,
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
    diagnostics::{
        DebugLoggingStatus, DiagnosticBundleExport, DiagnosticBundlePreview, DiagnosticsError,
    },
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
    infrastructure::sqlite::{
        DatabaseRecoveryMode, DatabaseRecoveryState,
        RecoverableDatabaseExport as ApplicationRecoverableDatabaseExport,
        SqliteWorkspaceRepository,
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
pub const GET_DATABASE_RECOVERY_STATE_COMMAND: &str = "get_database_recovery_state";
pub const EXPORT_RECOVERABLE_DATABASE_COMMAND: &str = "export_recoverable_database";
pub const GET_DIAGNOSTIC_BUNDLE_PREVIEW_COMMAND: &str = "get_diagnostic_bundle_preview";
pub const SET_DIAGNOSTIC_DEBUG_LOGGING_COMMAND: &str = "set_diagnostic_debug_logging";
pub const EXPORT_DIAGNOSTIC_BUNDLE_COMMAND: &str = "export_diagnostic_bundle";
pub const PREVIEW_CURL_IMPORT_COMMAND: &str = "preview_curl_import";
pub const IMPORT_CURL_AS_DRAFT_COMMAND: &str = "import_curl_as_draft";
pub const GENERATE_CURL_COMMAND: &str = "generate_curl";
pub const START_REQUEST_EXECUTION_COMMAND: &str = "start_request_execution";
pub const CANCEL_REQUEST_EXECUTION_COMMAND: &str = "cancel_request_execution";
pub const SAVE_RESPONSE_FILE_COMMAND: &str = "save_response_file";
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
pub struct DatabaseRecoveryStateDto {
    pub mode: DatabaseRecoveryModeDto,
    pub reason: Option<String>,
    pub snapshots: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DatabaseRecoveryModeDto {
    Normal,
    Safe,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecoverableDatabaseExportInput {
    pub source_path: String,
    pub export_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecoverableDatabaseExportDto {
    pub export_path: String,
    pub source_path: String,
    pub repaired_copy_path: String,
    pub table_count: u32,
    pub row_count: u32,
    pub redacted_value_count: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDebugLoggingInput {
    pub enabled: bool,
    pub duration_minutes: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBundleExportInput {
    pub bundle_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBundlePreviewDto {
    pub entries: Vec<String>,
    pub exclusions: Vec<String>,
    pub debug_logging_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDebugLoggingStatusDto {
    pub enabled: bool,
    pub expires_at_epoch_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticBundleExportDto {
    pub bundle_path: String,
    pub preview: DiagnosticBundlePreviewDto,
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
pub struct SaveResponseFileInput {
    pub source_path: String,
    pub destination_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveResponseFileOutput {
    pub destination_path: String,
    pub byte_count: u64,
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
        response_file: Option<ResponseFileMetadataDto>,
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
pub struct ResponseFileMetadataDto {
    pub path: String,
    pub byte_count: u64,
    pub expires_at_epoch_seconds: u64,
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
