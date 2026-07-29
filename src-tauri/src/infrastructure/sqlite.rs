use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, ErrorCode, OpenFlags, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{
    application::backup::{
        NativeBackupData, NativeBackupError, NativeBackupRepository, NativeBackupWorkspace,
    },
    application::postman_import::{
        ConvertedCollection, ConvertedPostmanImport, PostmanImportError, PostmanImportRepository,
        StoredPostmanImportRecord,
    },
    application::request::{
        CollectionLocation, ExecutionHistorySnapshot, ExecutionRecordDraft, RequestError,
        RequestRepository, RequestWorkspaceSnapshot, EXECUTION_HISTORY_RETENTION_DAYS,
        EXECUTION_HISTORY_RETENTION_LIMIT,
    },
    application::workspace::{
        WorkspaceError, WorkspaceRepository, WorkspaceSnapshot, WorkspaceSummary,
    },
    domain::{
        request::{
            BodyFilePath, BodyFileReference, CollectionFolder, CollectionId, CollectionVariable,
            CookieDraft, CookieId, CookieSameSite, Environment, EnvironmentId, EnvironmentVariable,
            ExecutionRecord, ExecutionRecordId, ExecutionRecordResponse, MultipartPart,
            OrderedField, RedirectPolicy, RequestAuth, RequestBody, RequestContent, RequestDraft,
            RequestDraftId, RequestTab, RequestTabId, SavedRequest, SavedRequestId, TlsPolicy,
            TransportPolicy, Variable, VariableValue, WorkspaceCookie,
        },
        workspace::{Workspace, WorkspaceId, WorkspaceName, DEFAULT_WORKSPACE_NAME},
    },
};

mod migrations;
mod repository;

const PRE_MIGRATION_SNAPSHOT_RETENTION: usize = 3;
const REDACTED_RECOVERY_VALUE: &str = "excluded";
const NEWER_SCHEMA_MESSAGE: &str = "database schema is newer than this Postmite build";

pub struct SqliteWorkspaceRepository {
    connection: Connection,
    recovery: DatabaseRecoveryState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRecoveryState {
    pub mode: DatabaseRecoveryMode,
    pub reason: Option<String>,
    pub snapshots: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DatabaseRecoveryMode {
    Normal,
    Safe,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverableDatabaseExport {
    pub export_path: String,
    pub source_path: String,
    pub repaired_copy_path: String,
    pub table_count: u32,
    pub row_count: u32,
    pub redacted_value_count: u32,
}
