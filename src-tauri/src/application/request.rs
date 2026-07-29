use std::{collections::HashMap, sync::Arc};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use cookie::{Cookie, Expiration, SameSite};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{
    request::{
        ApiKeyPlacement, BodyFileReference, CollectionFolder, CollectionId, CollectionVariable,
        CookieDraft, CookieId, CookieSameSite, Environment, EnvironmentId, EnvironmentVariable,
        ExecutionRecord, ExecutionRecordId, ExecutionRecordResponse, MultipartPart, OrderedField,
        RequestAuth, RequestBody, RequestContent, RequestDraft, RequestDraftId, RequestTab,
        RequestTabId, SavedRequest, SavedRequestId, VariableValue, WorkspaceCookie,
    },
    workspace::WorkspaceId,
};

use super::secrets::{SecretClass, SecretOwner, SecretStore};

pub const EXECUTION_HISTORY_RETENTION_DAYS: i64 = 30;
pub const EXECUTION_HISTORY_RETENTION_LIMIT: usize = 1_000;
pub const REDACTED_VALUE: &str = "********";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestWorkspaceSnapshot {
    pub workspace_id: WorkspaceId,
    pub collection_folders: Vec<CollectionFolder>,
    pub environments: Vec<Environment>,
    pub collection_variables: Vec<CollectionVariable>,
    pub environment_variables: Vec<EnvironmentVariable>,
    pub saved_requests: Vec<SavedRequest>,
    pub drafts: Vec<RequestDraft>,
    pub tabs: Vec<RequestTab>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionHistorySnapshot {
    pub workspace_id: WorkspaceId,
    pub disabled: bool,
    pub records: Vec<ExecutionRecord>,
    pub warning: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CookieJarSnapshot {
    pub workspace_id: WorkspaceId,
    pub cookies: Vec<WorkspaceCookie>,
}

impl ExecutionHistorySnapshot {
    pub fn warning_text() -> String {
        "Unknown sensitive values inside arbitrary response bodies may not always be detected."
            .to_owned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionRecordDraft {
    pub workspace_id: WorkspaceId,
    pub content: RequestContent,
    pub response: ExecutionRecordResponse,
    pub completed_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CollectionLocation {
    pub collection_id: Option<CollectionId>,
    pub position: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CloseTabDecision {
    Save,
    Discard,
    Cancel,
}

pub trait RequestRepository {
    fn list_request_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn open_unsaved_tab(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn create_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        content: RequestContent,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn create_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        parent_collection_id: Option<CollectionId>,
        name: String,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn select_environment(
        &mut self,
        workspace_id: WorkspaceId,
        environment_id: Option<EnvironmentId>,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn rename_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
        name: String,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn move_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
        location: CollectionLocation,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn duplicate_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn delete_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn move_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
        location: CollectionLocation,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn duplicate_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn delete_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn open_saved_request_tab(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn persist_draft(
        &mut self,
        workspace_id: WorkspaceId,
        draft_id: RequestDraftId,
        content: RequestContent,
    ) -> Result<(), RequestError>;
    fn save_draft(
        &mut self,
        workspace_id: WorkspaceId,
        draft_id: RequestDraftId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn close_tab(
        &mut self,
        workspace_id: WorkspaceId,
        tab_id: RequestTabId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn list_execution_history(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<ExecutionHistorySnapshot, RequestError>;
    fn set_execution_history_disabled(
        &mut self,
        workspace_id: WorkspaceId,
        disabled: bool,
    ) -> Result<ExecutionHistorySnapshot, RequestError>;
    fn set_execution_record_pinned(
        &mut self,
        workspace_id: WorkspaceId,
        record_id: ExecutionRecordId,
        pinned: bool,
    ) -> Result<ExecutionHistorySnapshot, RequestError>;
    fn insert_execution_record(&mut self, draft: ExecutionRecordDraft) -> Result<(), RequestError>;
    fn open_execution_record_as_draft(
        &mut self,
        workspace_id: WorkspaceId,
        record_id: ExecutionRecordId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn list_cookies(&self, workspace_id: WorkspaceId)
        -> Result<Vec<WorkspaceCookie>, RequestError>;
    fn upsert_cookie_metadata(
        &mut self,
        draft: CookieDraft,
        has_value: bool,
        secret_reference: Option<&str>,
        now_epoch_seconds: i64,
    ) -> Result<WorkspaceCookie, RequestError>;
    fn delete_cookie(
        &mut self,
        workspace_id: WorkspaceId,
        cookie_id: CookieId,
    ) -> Result<(), RequestError>;
    fn clear_cookies(&mut self, workspace_id: WorkspaceId) -> Result<(), RequestError>;
    fn cleanup_expired_cookies(
        &mut self,
        workspace_id: WorkspaceId,
        now_epoch_seconds: i64,
    ) -> Result<Vec<CookieId>, RequestError>;
    fn relink_body_files(
        &mut self,
        workspace_id: WorkspaceId,
        from_path: String,
        replacement: BodyFileReference,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
}

pub struct RequestService<R>
where
    R: RequestRepository,
{
    repository: R,
    secrets: Arc<dyn SecretStore>,
    pending_drafts: HashMap<RequestDraftId, PendingDraft>,
    cookie_values: HashMap<CookieId, String>,
}

impl<R> RequestService<R>
where
    R: RequestRepository,
{
    pub fn new(repository: R, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            repository,
            secrets,
            pending_drafts: HashMap::new(),
            cookie_values: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub fn new_for_test(repository: R) -> Self {
        Self::new(
            repository,
            Arc::new(super::secrets::SessionSecretStore::new()),
        )
    }

    pub fn list_request_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository.list_request_workspace(workspace_id)
    }

    pub fn open_unsaved_tab(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository.open_unsaved_tab(workspace_id)
    }

    pub fn create_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        content: RequestContent,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository.create_saved_request(workspace_id, content)
    }

    pub fn create_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        parent_collection_id: Option<CollectionId>,
        name: impl Into<String>,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .create_collection_folder(workspace_id, parent_collection_id, name.into())
    }

    pub fn select_environment(
        &mut self,
        workspace_id: WorkspaceId,
        environment_id: Option<EnvironmentId>,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .select_environment(workspace_id, environment_id)
    }

    pub fn resolve_request_content(
        &self,
        workspace_id: WorkspaceId,
        content: &RequestContent,
    ) -> Result<ResolvedRequestContent, RequestError> {
        let snapshot = self.repository.list_request_workspace(workspace_id)?;
        Ok(resolve_request_content(&snapshot, content))
    }

    pub fn materialize_request_content(
        &self,
        workspace_id: WorkspaceId,
        content: RequestContent,
    ) -> Result<RequestContent, RequestError> {
        let snapshot = self.repository.list_request_workspace(workspace_id)?;
        Ok(materialize_request_auth_with_secret_resolver(
            &snapshot,
            content,
            &|reference| self.secrets.get(reference).ok(),
        ))
    }

    pub fn rename_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
        name: impl Into<String>,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .rename_collection_folder(workspace_id, collection_id, name.into())
    }

    pub fn move_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
        location: CollectionLocation,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .move_collection_folder(workspace_id, collection_id, location)
    }

    pub fn duplicate_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .duplicate_collection_folder(workspace_id, collection_id)
    }

    pub fn delete_collection_folder(
        &mut self,
        workspace_id: WorkspaceId,
        collection_id: CollectionId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .delete_collection_folder(workspace_id, collection_id)
    }

    pub fn move_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
        location: CollectionLocation,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .move_saved_request(workspace_id, saved_request_id, location)
    }

    pub fn duplicate_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .duplicate_saved_request(workspace_id, saved_request_id)
    }

    pub fn delete_saved_request(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .delete_saved_request(workspace_id, saved_request_id)
    }

    pub fn open_saved_request_tab(
        &mut self,
        workspace_id: WorkspaceId,
        saved_request_id: SavedRequestId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .open_saved_request_tab(workspace_id, saved_request_id)
    }

    pub fn queue_draft_update(
        &mut self,
        workspace_id: WorkspaceId,
        draft_id: RequestDraftId,
        content: RequestContent,
    ) {
        self.pending_drafts.insert(
            draft_id,
            PendingDraft {
                workspace_id,
                content,
            },
        );
    }

    pub fn flush_pending_drafts(&mut self) -> Result<(), RequestError> {
        let pending = std::mem::take(&mut self.pending_drafts);
        for (draft_id, pending_draft) in pending {
            self.repository.persist_draft(
                pending_draft.workspace_id,
                draft_id,
                pending_draft.content,
            )?;
        }
        Ok(())
    }

    pub fn save_draft(
        &mut self,
        workspace_id: WorkspaceId,
        draft_id: RequestDraftId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.flush_pending_drafts()?;
        self.repository.save_draft(workspace_id, draft_id)
    }

    pub fn close_tab(
        &mut self,
        workspace_id: WorkspaceId,
        tab_id: RequestTabId,
        decision: CloseTabDecision,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        match decision {
            CloseTabDecision::Save => {
                self.flush_pending_drafts()?;
                let snapshot = self.repository.list_request_workspace(workspace_id)?;
                let tab = snapshot
                    .tabs
                    .iter()
                    .find(|tab| tab.id == tab_id)
                    .ok_or(RequestError::NotFound)?;
                self.repository.save_draft(workspace_id, tab.draft_id)?;
                self.repository.close_tab(workspace_id, tab_id)
            }
            CloseTabDecision::Discard => self.repository.close_tab(workspace_id, tab_id),
            CloseTabDecision::Cancel => self.repository.list_request_workspace(workspace_id),
        }
    }

    pub fn list_execution_history(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<ExecutionHistorySnapshot, RequestError> {
        self.repository.list_execution_history(workspace_id)
    }

    pub fn set_execution_history_disabled(
        &mut self,
        workspace_id: WorkspaceId,
        disabled: bool,
    ) -> Result<ExecutionHistorySnapshot, RequestError> {
        self.repository
            .set_execution_history_disabled(workspace_id, disabled)
    }

    pub fn set_execution_record_pinned(
        &mut self,
        workspace_id: WorkspaceId,
        record_id: ExecutionRecordId,
        pinned: bool,
    ) -> Result<ExecutionHistorySnapshot, RequestError> {
        self.repository
            .set_execution_record_pinned(workspace_id, record_id, pinned)
    }

    pub fn record_execution(
        &mut self,
        workspace_id: WorkspaceId,
        content: RequestContent,
        response: ExecutionRecordResponse,
        completed_at_epoch_seconds: i64,
    ) -> Result<(), RequestError> {
        let snapshot = self.repository.list_request_workspace(workspace_id)?;
        let resolved =
            resolve_request_content_with_secret_resolver(&snapshot, &content, &|reference| {
                self.secrets.get(reference).ok()
            });
        let redacted = redact_request_content(content, &resolved);
        self.repository
            .insert_execution_record(ExecutionRecordDraft {
                workspace_id,
                content: redacted,
                response: redact_response(response),
                completed_at_epoch_seconds,
            })
    }

    pub fn open_execution_record_as_draft(
        &mut self,
        workspace_id: WorkspaceId,
        record_id: ExecutionRecordId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .open_execution_record_as_draft(workspace_id, record_id)
    }

    pub fn list_cookies(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<CookieJarSnapshot, RequestError> {
        let cookies = self.cleanup_and_load_cookies(workspace_id, current_epoch_seconds())?;
        Ok(CookieJarSnapshot {
            workspace_id,
            cookies,
        })
    }

    pub fn reveal_cookie_value(
        &mut self,
        workspace_id: WorkspaceId,
        cookie_id: CookieId,
    ) -> Result<String, RequestError> {
        let cookies = self.cleanup_and_load_cookies(workspace_id, current_epoch_seconds())?;
        if !cookies.iter().any(|cookie| cookie.id == cookie_id) {
            return Err(RequestError::NotFound);
        }
        cookies
            .iter()
            .find(|cookie| cookie.id == cookie_id)
            .and_then(|cookie| self.cookie_value(cookie))
            .ok_or(RequestError::NotFound)
    }

    pub fn upsert_cookie(&mut self, draft: CookieDraft) -> Result<CookieJarSnapshot, RequestError> {
        validate_cookie_draft(&draft)?;
        let now = current_epoch_seconds();
        let existing = self
            .repository
            .list_cookies(draft.workspace_id)?
            .into_iter()
            .find(|cookie| cookie_scope_matches(cookie, &draft));
        let (has_value, secret_reference) = if draft.expires_at_epoch_seconds.is_some() {
            let write = self
                .secrets
                .put(
                    &SecretOwner::new(
                        draft.workspace_id,
                        SecretClass::CookieValue,
                        cookie_secret_owner_name(&draft),
                    ),
                    &draft.value,
                )
                .map_err(secret_request_error)?;
            (true, Some(write.reference))
        } else {
            (true, None)
        };
        let cookie = self.repository.upsert_cookie_metadata(
            draft.clone(),
            has_value,
            secret_reference.as_deref(),
            now,
        )?;
        if let Some(reference) = secret_reference.as_deref() {
            self.cookie_values.remove(&cookie.id);
            if let Some(old_reference) = existing
                .and_then(|cookie| cookie.secret_reference)
                .filter(|old_reference| old_reference != reference)
            {
                self.secrets
                    .delete(&old_reference)
                    .map_err(secret_request_error)?;
            }
        } else {
            self.cookie_values.insert(cookie.id, draft.value);
        }
        self.list_cookies(cookie.workspace_id)
    }

    pub fn delete_cookie(
        &mut self,
        workspace_id: WorkspaceId,
        cookie_id: CookieId,
    ) -> Result<CookieJarSnapshot, RequestError> {
        let secret_reference = self
            .repository
            .list_cookies(workspace_id)?
            .into_iter()
            .find(|cookie| cookie.id == cookie_id)
            .and_then(|cookie| cookie.secret_reference);
        self.repository.delete_cookie(workspace_id, cookie_id)?;
        self.cookie_values.remove(&cookie_id);
        if let Some(reference) = secret_reference {
            self.secrets
                .delete(&reference)
                .map_err(secret_request_error)?;
        }
        self.list_cookies(workspace_id)
    }

    pub fn clear_cookies(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<CookieJarSnapshot, RequestError> {
        let existing = self.repository.list_cookies(workspace_id)?;
        self.repository.clear_cookies(workspace_id)?;
        for cookie in existing {
            self.cookie_values.remove(&cookie.id);
            if let Some(reference) = cookie.secret_reference {
                self.secrets
                    .delete(&reference)
                    .map_err(secret_request_error)?;
            }
        }
        self.list_cookies(workspace_id)
    }

    pub fn relink_body_files(
        &mut self,
        workspace_id: WorkspaceId,
        from_path: String,
        replacement: BodyFileReference,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.flush_pending_drafts()?;
        self.repository
            .relink_body_files(workspace_id, from_path, replacement)
    }

    pub fn attach_matching_cookies(
        &mut self,
        workspace_id: WorkspaceId,
        mut content: RequestContent,
    ) -> Result<RequestContent, RequestError> {
        if has_enabled_cookie_header(&content.headers) {
            return Ok(content);
        }

        let url = Url::parse(&content.url)
            .map_err(|_| RequestError::InvalidInput("url.invalid".to_owned()))?;
        let now = current_epoch_seconds();
        let cookies = self.cleanup_and_load_cookies(workspace_id, now)?;
        let mut pairs = Vec::new();
        for cookie in cookies {
            if cookie_matches_url(&cookie, &url, now) {
                if let Some(value) = self.cookie_value(&cookie) {
                    pairs.push(format!("{}={}", cookie.name, value));
                }
            }
        }

        if !pairs.is_empty() {
            let order = content
                .headers
                .iter()
                .map(|field| field.order)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            content.headers.push(OrderedField {
                enabled: true,
                order,
                name: "Cookie".to_owned(),
                value: pairs.join("; "),
            });
        }

        Ok(content)
    }

    pub fn capture_set_cookie_headers(
        &mut self,
        workspace_id: WorkspaceId,
        request_url: &str,
        headers: &[OrderedField],
    ) -> Result<(), RequestError> {
        let url = Url::parse(request_url)
            .map_err(|_| RequestError::InvalidInput("url.invalid".to_owned()))?;
        for header in headers {
            if !header.name.eq_ignore_ascii_case("set-cookie") {
                continue;
            }
            if let Some(draft) = cookie_draft_from_set_cookie(workspace_id, &url, &header.value)? {
                let _ = self.upsert_cookie(draft)?;
            }
        }
        Ok(())
    }

    fn cleanup_and_load_cookies(
        &mut self,
        workspace_id: WorkspaceId,
        now_epoch_seconds: i64,
    ) -> Result<Vec<WorkspaceCookie>, RequestError> {
        let existing = self.repository.list_cookies(workspace_id)?;
        let removed = self
            .repository
            .cleanup_expired_cookies(workspace_id, now_epoch_seconds)?;
        for id in removed {
            self.cookie_values.remove(&id);
            if let Some(reference) = existing
                .iter()
                .find(|cookie| cookie.id == id)
                .and_then(|cookie| cookie.secret_reference.as_deref())
            {
                self.secrets
                    .delete(reference)
                    .map_err(secret_request_error)?;
            }
        }
        self.repository.list_cookies(workspace_id).map(|cookies| {
            cookies
                .into_iter()
                .map(|mut cookie| {
                    cookie.has_value = self.cookie_value(&cookie).is_some();
                    cookie
                })
                .collect()
        })
    }

    fn cookie_value(&self, cookie: &WorkspaceCookie) -> Option<String> {
        self.cookie_values.get(&cookie.id).cloned().or_else(|| {
            cookie
                .secret_reference
                .as_deref()
                .and_then(|reference| self.secrets.get(reference).ok())
        })
    }
}

impl<R> Drop for RequestService<R>
where
    R: RequestRepository,
{
    fn drop(&mut self) {
        let _ = self.flush_pending_drafts();
    }
}

#[derive(Clone)]
struct PendingDraft {
    workspace_id: WorkspaceId,
    content: RequestContent,
}

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("workspace was not found")]
    WorkspaceNotFound,
    #[error("request item was not found")]
    NotFound,
    #[error("the saved request is already open in this workspace")]
    SavedRequestAlreadyOpen,
    #[error("request input is invalid: {0}")]
    InvalidInput(String),
    #[error("request persistence failed: {0}")]
    Persistence(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedRequestContent {
    pub url: ResolvedValue,
    pub body: ResolvedValue,
    pub body_kind: ResolvedRequestBody,
    pub query: Vec<ResolvedField>,
    pub headers: Vec<ResolvedField>,
    pub unsafe_tls_visible: bool,
    pub references: Vec<ResolvedVariableReference>,
    pub errors: Vec<VariableResolutionError>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum ResolvedRequestBody {
    None,
    Raw { content: ResolvedValue },
    UrlEncoded { fields: Vec<ResolvedField> },
    Multipart { parts: Vec<ResolvedMultipartPart> },
    Binary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum ResolvedMultipartPart {
    Field {
        enabled: bool,
        order: u32,
        name: ResolvedValue,
        value: ResolvedValue,
    },
    File {
        enabled: bool,
        order: u32,
        name: ResolvedValue,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedField {
    pub enabled: bool,
    pub order: u32,
    pub name: ResolvedValue,
    pub value: ResolvedValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedValue {
    pub value: String,
    pub contains_secret: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedVariableReference {
    pub name: String,
    pub source: VariableSource,
    pub value: ResolvedValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum VariableSource {
    Collection,
    Environment,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VariableResolutionError {
    pub name: String,
    pub kind: VariableResolutionErrorKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum VariableResolutionErrorKind {
    Missing,
    Cycle,
}

pub fn resolve_request_content(
    snapshot: &RequestWorkspaceSnapshot,
    content: &RequestContent,
) -> ResolvedRequestContent {
    resolve_request_content_with_secret_resolver(snapshot, content, &|_| None)
}

fn resolve_request_content_with_secret_resolver(
    snapshot: &RequestWorkspaceSnapshot,
    content: &RequestContent,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedRequestContent {
    let scope = VariableScope::from_snapshot(snapshot);
    let mut state = ResolutionState::default();
    let url = resolve_text(&content.url, &scope, &mut state, secret_resolver);
    let body_kind = resolve_request_body(&content.body, &scope, &mut state, secret_resolver);
    let body = resolved_body_preview(&body_kind);
    let mut query = content
        .query
        .iter()
        .map(|field| ResolvedField {
            enabled: field.enabled,
            order: field.order,
            name: resolve_text(&field.name, &scope, &mut state, secret_resolver),
            value: resolve_text(&field.value, &scope, &mut state, secret_resolver),
        })
        .collect();
    let mut headers = content
        .headers
        .iter()
        .map(|field| ResolvedField {
            enabled: field.enabled,
            order: field.order,
            name: resolve_text(&field.name, &scope, &mut state, secret_resolver),
            value: resolve_text(&field.value, &scope, &mut state, secret_resolver),
        })
        .collect();
    apply_resolved_auth(
        &content.auth,
        &scope,
        &mut state,
        &mut query,
        &mut headers,
        secret_resolver,
    );

    let mut references = state.references.into_values().collect::<Vec<_>>();
    references.sort_by(|left, right| left.name.cmp(&right.name));
    let mut errors = state.errors.into_values().collect::<Vec<_>>();
    errors.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
    });

    ResolvedRequestContent {
        url,
        body,
        body_kind,
        query,
        headers,
        unsafe_tls_visible: !content.tls.verify,
        references,
        errors,
    }
}

pub fn redact_request_content(
    content: RequestContent,
    resolved: &ResolvedRequestContent,
) -> RequestContent {
    RequestContent {
        name: content.name,
        method: content.method,
        url: redact_value_pair(content.url, &resolved.url),
        body: redact_request_body(content.body, &resolved.body_kind),
        query: content
            .query
            .into_iter()
            .zip(resolved.query.iter())
            .map(|(field, resolved)| OrderedField {
                enabled: field.enabled,
                order: field.order,
                name: redact_value_pair(field.name, &resolved.name),
                value: redact_value_pair(field.value, &resolved.value),
            })
            .collect(),
        headers: content
            .headers
            .into_iter()
            .zip(resolved.headers.iter())
            .map(|(field, resolved)| {
                let sensitive_header = is_sensitive_header(&field.name);
                OrderedField {
                    enabled: field.enabled,
                    order: field.order,
                    name: field.name,
                    value: if sensitive_header || resolved.value.contains_secret {
                        REDACTED_VALUE.to_owned()
                    } else {
                        field.value
                    },
                }
            })
            .collect(),
        auth: redact_request_auth(content.auth),
        redirect: content.redirect,
        tls: content.tls,
        transport: redact_transport_policy(content.transport),
    }
}

fn redact_transport_policy(
    mut policy: crate::domain::request::TransportPolicy,
) -> crate::domain::request::TransportPolicy {
    if let Some(url) = policy.proxy.url.as_deref() {
        policy.proxy.url = Some(redact_url_credentials(url));
    }
    policy
}

fn redact_url_credentials(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return REDACTED_VALUE.to_owned();
    };
    if !url.username().is_empty() {
        let _ = url.set_username(REDACTED_VALUE);
    }
    if url.password().is_some() {
        let _ = url.set_password(Some(REDACTED_VALUE));
    }
    url.to_string()
}

pub fn materialize_request_auth(
    snapshot: &RequestWorkspaceSnapshot,
    content: RequestContent,
) -> RequestContent {
    materialize_request_auth_with_secret_resolver(snapshot, content, &|_| None)
}

fn materialize_request_auth_with_secret_resolver(
    snapshot: &RequestWorkspaceSnapshot,
    mut content: RequestContent,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> RequestContent {
    let resolved =
        resolve_request_content_with_secret_resolver(snapshot, &content, secret_resolver);
    content.url = resolved.url.value;
    content.body = materialize_request_body(content.body, &resolved.body_kind);
    content.query = resolved_fields_to_ordered(resolved.query);
    content.headers = resolved_fields_to_ordered(resolved.headers);
    content.auth = RequestAuth::None;
    content
}

fn materialize_request_body(body: RequestBody, resolved: &ResolvedRequestBody) -> RequestBody {
    match (body, resolved) {
        (RequestBody::Raw { .. }, ResolvedRequestBody::Raw { content }) => RequestBody::Raw {
            content: content.value.clone(),
        },
        (
            RequestBody::UrlEncoded { fields },
            ResolvedRequestBody::UrlEncoded {
                fields: resolved_fields,
            },
        ) => RequestBody::UrlEncoded {
            fields: fields
                .into_iter()
                .zip(resolved_fields.iter())
                .map(|(field, resolved)| OrderedField {
                    enabled: field.enabled,
                    order: field.order,
                    name: resolved.name.value.clone(),
                    value: resolved.value.value.clone(),
                })
                .collect(),
        },
        (RequestBody::Multipart { parts }, ResolvedRequestBody::Multipart { parts: resolved }) => {
            RequestBody::Multipart {
                parts: parts
                    .into_iter()
                    .zip(resolved.iter())
                    .map(|(part, resolved)| match (part, resolved) {
                        (
                            MultipartPart::Field { enabled, order, .. },
                            ResolvedMultipartPart::Field { name, value, .. },
                        ) => MultipartPart::Field {
                            enabled,
                            order,
                            name: name.value.clone(),
                            value: value.value.clone(),
                        },
                        (
                            MultipartPart::File {
                                enabled,
                                order,
                                file,
                                ..
                            },
                            ResolvedMultipartPart::File { name, .. },
                        ) => MultipartPart::File {
                            enabled,
                            order,
                            name: name.value.clone(),
                            file,
                        },
                        (part, _) => part,
                    })
                    .collect(),
            }
        }
        (body, _) => body,
    }
}

fn resolved_fields_to_ordered(fields: Vec<ResolvedField>) -> Vec<OrderedField> {
    fields
        .into_iter()
        .map(|field| OrderedField {
            enabled: field.enabled,
            order: field.order,
            name: field.name.value,
            value: field.value.value,
        })
        .collect()
}

fn apply_resolved_auth(
    auth: &RequestAuth,
    scope: &VariableScope,
    state: &mut ResolutionState,
    query: &mut Vec<ResolvedField>,
    headers: &mut Vec<ResolvedField>,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) {
    match auth {
        RequestAuth::None => {}
        RequestAuth::Basic { username, password } => {
            let username = resolve_text(username, scope, state, secret_resolver);
            let password = resolve_text(password, scope, state, secret_resolver);
            let contains_secret = username.contains_secret || password.contains_secret;
            let value = if contains_secret {
                REDACTED_VALUE.to_owned()
            } else {
                format!(
                    "Basic {}",
                    BASE64_STANDARD.encode(format!("{}:{}", username.value, password.value))
                )
            };
            headers.push(resolved_auth_field(
                headers,
                "Authorization",
                ResolvedValue {
                    value,
                    contains_secret,
                },
            ));
        }
        RequestAuth::Bearer { token } => {
            let token = resolve_text(token, scope, state, secret_resolver);
            headers.push(resolved_auth_field(
                headers,
                "Authorization",
                ResolvedValue {
                    value: if token.contains_secret {
                        REDACTED_VALUE.to_owned()
                    } else {
                        format!("Bearer {}", token.value)
                    },
                    contains_secret: token.contains_secret,
                },
            ));
        }
        RequestAuth::ApiKey {
            placement,
            name,
            value,
        } => {
            let name = resolve_text(name, scope, state, secret_resolver);
            let value = resolve_text(value, scope, state, secret_resolver);
            let field = resolved_auth_field(
                match placement {
                    ApiKeyPlacement::Header => headers,
                    ApiKeyPlacement::Query => query,
                },
                &name.value,
                ResolvedValue {
                    value: if value.contains_secret {
                        REDACTED_VALUE.to_owned()
                    } else {
                        value.value
                    },
                    contains_secret: value.contains_secret,
                },
            );
            match placement {
                ApiKeyPlacement::Header => headers.push(field),
                ApiKeyPlacement::Query => query.push(field),
            }
        }
    }
}

fn resolved_auth_field(
    existing: &[ResolvedField],
    name: &str,
    value: ResolvedValue,
) -> ResolvedField {
    ResolvedField {
        enabled: true,
        order: next_resolved_field_order(existing),
        name: ResolvedValue {
            value: name.to_owned(),
            contains_secret: false,
        },
        value,
    }
}

fn next_resolved_field_order(fields: &[ResolvedField]) -> u32 {
    fields
        .iter()
        .map(|field| field.order)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn redact_request_auth(auth: RequestAuth) -> RequestAuth {
    match auth {
        RequestAuth::None => RequestAuth::None,
        RequestAuth::Basic { username, .. } => RequestAuth::Basic {
            username,
            password: REDACTED_VALUE.to_owned(),
        },
        RequestAuth::Bearer { .. } => RequestAuth::Bearer {
            token: REDACTED_VALUE.to_owned(),
        },
        RequestAuth::ApiKey {
            placement, name, ..
        } => RequestAuth::ApiKey {
            placement,
            name,
            value: REDACTED_VALUE.to_owned(),
        },
    }
}

fn resolve_request_body(
    body: &RequestBody,
    scope: &VariableScope,
    state: &mut ResolutionState,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedRequestBody {
    match body {
        RequestBody::None => ResolvedRequestBody::None,
        RequestBody::Raw { content } => ResolvedRequestBody::Raw {
            content: resolve_text(content, scope, state, secret_resolver),
        },
        RequestBody::UrlEncoded { fields } => ResolvedRequestBody::UrlEncoded {
            fields: fields
                .iter()
                .map(|field| ResolvedField {
                    enabled: field.enabled,
                    order: field.order,
                    name: resolve_text(&field.name, scope, state, secret_resolver),
                    value: resolve_text(&field.value, scope, state, secret_resolver),
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
                        name: resolve_text(name, scope, state, secret_resolver),
                        value: resolve_text(value, scope, state, secret_resolver),
                    },
                    MultipartPart::File {
                        enabled,
                        order,
                        name,
                        ..
                    } => ResolvedMultipartPart::File {
                        enabled: *enabled,
                        order: *order,
                        name: resolve_text(name, scope, state, secret_resolver),
                    },
                })
                .collect(),
        },
        RequestBody::Binary { .. } => ResolvedRequestBody::Binary,
    }
}

fn resolved_body_preview(body: &ResolvedRequestBody) -> ResolvedValue {
    match body {
        ResolvedRequestBody::None => ResolvedValue {
            value: String::new(),
            contains_secret: false,
        },
        ResolvedRequestBody::Raw { content } => content.clone(),
        ResolvedRequestBody::UrlEncoded { fields } => ResolvedValue {
            value: fields
                .iter()
                .filter(|field| field.enabled)
                .map(|field| format!("{}={}", field.name.value, field.value.value))
                .collect::<Vec<_>>()
                .join("&"),
            contains_secret: fields
                .iter()
                .any(|field| field.name.contains_secret || field.value.contains_secret),
        },
        ResolvedRequestBody::Multipart { parts } => ResolvedValue {
            value: format!("{} multipart part(s)", parts.len()),
            contains_secret: parts.iter().any(|part| match part {
                ResolvedMultipartPart::Field { name, value, .. } => {
                    name.contains_secret || value.contains_secret
                }
                ResolvedMultipartPart::File { name, .. } => name.contains_secret,
            }),
        },
        ResolvedRequestBody::Binary => ResolvedValue {
            value: "binary file".to_owned(),
            contains_secret: false,
        },
    }
}

fn redact_request_body(body: RequestBody, resolved: &ResolvedRequestBody) -> RequestBody {
    match (body, resolved) {
        (RequestBody::Raw { content }, ResolvedRequestBody::Raw { content: resolved }) => {
            RequestBody::Raw {
                content: redact_value_pair(content, resolved),
            }
        }
        (
            RequestBody::UrlEncoded { fields },
            ResolvedRequestBody::UrlEncoded { fields: resolved },
        ) => RequestBody::UrlEncoded {
            fields: fields
                .into_iter()
                .zip(resolved.iter())
                .map(|(field, resolved)| OrderedField {
                    enabled: field.enabled,
                    order: field.order,
                    name: redact_value_pair(field.name, &resolved.name),
                    value: redact_value_pair(field.value, &resolved.value),
                })
                .collect(),
        },
        (RequestBody::Multipart { parts }, ResolvedRequestBody::Multipart { parts: resolved }) => {
            RequestBody::Multipart {
                parts: parts
                    .into_iter()
                    .zip(resolved.iter())
                    .map(|(part, resolved)| match (part, resolved) {
                        (
                            MultipartPart::Field {
                                enabled,
                                order,
                                name,
                                value,
                            },
                            ResolvedMultipartPart::Field {
                                name: resolved_name,
                                value: resolved_value,
                                ..
                            },
                        ) => MultipartPart::Field {
                            enabled,
                            order,
                            name: redact_value_pair(name, resolved_name),
                            value: redact_value_pair(value, resolved_value),
                        },
                        (part, _) => part,
                    })
                    .collect(),
            }
        }
        (body, _) => body,
    }
}

fn redact_response(response: ExecutionRecordResponse) -> ExecutionRecordResponse {
    ExecutionRecordResponse {
        headers: response
            .headers
            .into_iter()
            .map(|field| OrderedField {
                value: if is_sensitive_header(&field.name) {
                    REDACTED_VALUE.to_owned()
                } else {
                    field.value
                },
                ..field
            })
            .collect(),
        ..response
    }
}

fn redact_value_pair(original: String, resolved: &ResolvedValue) -> String {
    if resolved.contains_secret {
        REDACTED_VALUE.to_owned()
    } else {
        original
    }
}

fn is_sensitive_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("authorization")
        || name.eq_ignore_ascii_case("cookie")
        || name.eq_ignore_ascii_case("set-cookie")
}

fn has_enabled_cookie_header(headers: &[OrderedField]) -> bool {
    headers
        .iter()
        .any(|field| field.enabled && field.name.eq_ignore_ascii_case("cookie"))
}

fn validate_cookie_draft(draft: &CookieDraft) -> Result<(), RequestError> {
    if draft.name.trim().is_empty() {
        return Err(RequestError::InvalidInput(
            "cookie.name.required".to_owned(),
        ));
    }
    if draft.name.contains('=') || draft.name.contains(';') {
        return Err(RequestError::InvalidInput("cookie.name.invalid".to_owned()));
    }
    if draft.domain.trim().is_empty() {
        return Err(RequestError::InvalidInput(
            "cookie.domain.required".to_owned(),
        ));
    }
    if !draft.path.starts_with('/') {
        return Err(RequestError::InvalidInput("cookie.path.invalid".to_owned()));
    }
    Ok(())
}

fn cookie_matches_url(cookie: &WorkspaceCookie, url: &Url, now_epoch_seconds: i64) -> bool {
    if let Some(expires_at) = cookie.expires_at_epoch_seconds {
        if expires_at <= now_epoch_seconds {
            return false;
        }
    }
    if cookie.secure && url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let cookie_domain = cookie.domain.trim_start_matches('.').to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    if host != cookie_domain && !host.ends_with(&format!(".{cookie_domain}")) {
        return false;
    }
    url.path().starts_with(&cookie.path)
}

fn cookie_draft_from_set_cookie(
    workspace_id: WorkspaceId,
    url: &Url,
    value: &str,
) -> Result<Option<CookieDraft>, RequestError> {
    let parsed = Cookie::parse(value.to_owned())
        .map_err(|_| RequestError::InvalidInput("cookie.set_cookie.invalid".to_owned()))?;
    let Some(host) = url.host_str() else {
        return Ok(None);
    };
    let domain = parsed
        .domain()
        .map(str::to_owned)
        .unwrap_or_else(|| host.to_owned());
    let path = parsed
        .path()
        .map(str::to_owned)
        .unwrap_or_else(|| default_cookie_path(url.path()));
    let expires_at_epoch_seconds = match parsed.expires() {
        Some(Expiration::DateTime(datetime)) => Some(datetime.unix_timestamp()),
        _ => None,
    };
    let same_site = parsed.same_site().map(cookie_same_site_from_cookie);
    Ok(Some(CookieDraft {
        id: None,
        workspace_id,
        name: parsed.name().to_owned(),
        value: parsed.value().to_owned(),
        domain,
        path,
        secure: parsed.secure().unwrap_or(false),
        http_only: parsed.http_only().unwrap_or(false),
        same_site,
        expires_at_epoch_seconds,
    }))
}

fn default_cookie_path(url_path: &str) -> String {
    if !url_path.starts_with('/') || url_path == "/" {
        return "/".to_owned();
    }
    match url_path.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(index) => url_path[..index].to_owned(),
    }
}

fn cookie_same_site_from_cookie(value: SameSite) -> CookieSameSite {
    match value {
        SameSite::Strict => CookieSameSite::Strict,
        SameSite::Lax => CookieSameSite::Lax,
        SameSite::None => CookieSameSite::None,
    }
}

fn current_epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn secret_request_error(error: super::secrets::SecretError) -> RequestError {
    match error {
        super::secrets::SecretError::Locked => {
            RequestError::InvalidInput("secret.storage.locked".to_owned())
        }
        super::secrets::SecretError::Unavailable => {
            RequestError::InvalidInput("secret.storage.unavailable".to_owned())
        }
        super::secrets::SecretError::NotFound => {
            RequestError::InvalidInput("secret.reference.notFound".to_owned())
        }
        super::secrets::SecretError::Storage(_) => {
            RequestError::Persistence("secret storage failed".to_owned())
        }
    }
}

fn cookie_scope_matches(cookie: &WorkspaceCookie, draft: &CookieDraft) -> bool {
    cookie.workspace_id == draft.workspace_id
        && cookie.name == draft.name.trim()
        && cookie.domain == normalize_cookie_domain_for_request(&draft.domain)
        && cookie.path == draft.path
}

fn cookie_secret_owner_name(draft: &CookieDraft) -> String {
    format!(
        "{}:{}:{}",
        normalize_cookie_domain_for_request(&draft.domain),
        draft.path,
        draft.name.trim()
    )
}

fn normalize_cookie_domain_for_request(domain: &str) -> String {
    domain.trim().trim_start_matches('.').to_ascii_lowercase()
}

#[derive(Clone)]
struct ScopedVariable {
    source: VariableSource,
    value: VariableValue,
}

struct VariableScope {
    variables: HashMap<String, ScopedVariable>,
}

impl VariableScope {
    fn from_snapshot(snapshot: &RequestWorkspaceSnapshot) -> Self {
        let mut variables = snapshot
            .collection_variables
            .iter()
            .map(|item| {
                (
                    item.variable.name.clone(),
                    ScopedVariable {
                        source: VariableSource::Collection,
                        value: item.variable.value.clone(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let selected_environment_id = snapshot
            .environments
            .iter()
            .find(|environment| environment.is_selected)
            .map(|environment| environment.id);
        if let Some(environment_id) = selected_environment_id {
            for item in snapshot
                .environment_variables
                .iter()
                .filter(|item| item.environment_id == environment_id)
            {
                variables.insert(
                    item.variable.name.clone(),
                    ScopedVariable {
                        source: VariableSource::Environment,
                        value: item.variable.value.clone(),
                    },
                );
            }
        }

        Self { variables }
    }
}

#[derive(Default)]
struct ResolutionState {
    references: HashMap<String, ResolvedVariableReference>,
    errors: HashMap<String, VariableResolutionError>,
}

fn resolve_text(
    input: &str,
    scope: &VariableScope,
    state: &mut ResolutionState,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedValue {
    resolve_text_with_stack(input, scope, state, &mut Vec::new(), secret_resolver)
}

fn resolve_text_with_stack(
    input: &str,
    scope: &VariableScope,
    state: &mut ResolutionState,
    stack: &mut Vec<String>,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedValue {
    let mut output = String::new();
    let mut contains_secret = false;
    let mut cursor = 0;
    while let Some(start) = input[cursor..].find("{{") {
        let absolute_start = cursor + start;
        output.push_str(&input[cursor..absolute_start]);
        let after_start = absolute_start + 2;
        let Some(end) = input[after_start..].find("}}") else {
            output.push_str(&input[absolute_start..]);
            return ResolvedValue {
                value: output,
                contains_secret,
            };
        };
        let absolute_end = after_start + end;
        let name = input[after_start..absolute_end].trim();
        let resolved = resolve_variable(name, scope, state, stack, secret_resolver);
        if resolved.contains_secret {
            contains_secret = true;
        }
        output.push_str(&resolved.value);
        cursor = absolute_end + 2;
    }
    output.push_str(&input[cursor..]);

    ResolvedValue {
        value: output,
        contains_secret,
    }
}

fn resolve_variable(
    name: &str,
    scope: &VariableScope,
    state: &mut ResolutionState,
    stack: &mut Vec<String>,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedValue {
    if stack.iter().any(|item| item == name) {
        state.errors.insert(
            format!("{name}:cycle"),
            VariableResolutionError {
                name: name.to_owned(),
                kind: VariableResolutionErrorKind::Cycle,
            },
        );
        return ResolvedValue {
            value: format!("{{{{{name}}}}}"),
            contains_secret: false,
        };
    }

    let Some(variable) = scope.variables.get(name) else {
        state.errors.insert(
            format!("{name}:missing"),
            VariableResolutionError {
                name: name.to_owned(),
                kind: VariableResolutionErrorKind::Missing,
            },
        );
        return ResolvedValue {
            value: format!("{{{{{name}}}}}"),
            contains_secret: false,
        };
    };

    stack.push(name.to_owned());
    let value = match &variable.value {
        VariableValue::Plain(value) => {
            resolve_text_with_stack(value, scope, state, stack, secret_resolver)
        }
        VariableValue::SecretReference(reference) => ResolvedValue {
            value: secret_resolver(reference).unwrap_or_else(|| REDACTED_VALUE.to_owned()),
            contains_secret: true,
        },
    };
    stack.pop();

    state.references.insert(
        name.to_owned(),
        ResolvedVariableReference {
            name: name.to_owned(),
            source: variable.source,
            value: value.clone(),
        },
    );

    value
}

impl RequestError {
    pub fn persistence(error: impl std::error::Error) -> Self {
        Self::Persistence(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::request::{
        CollectionVariable, Environment, EnvironmentVariable, OrderedField, RequestAuth, Variable,
        VariableValue,
    };

    #[derive(Default)]
    struct FakeRequestRepository {
        persisted: Vec<(WorkspaceId, RequestDraftId, RequestContent)>,
        history_records: Vec<ExecutionRecordDraft>,
        cookies: Vec<WorkspaceCookie>,
        snapshot: Option<RequestWorkspaceSnapshot>,
        close_calls: usize,
        save_calls: usize,
    }

    impl RequestRepository for FakeRequestRepository {
        fn list_request_workspace(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            Ok(self.snapshot.clone().unwrap_or(RequestWorkspaceSnapshot {
                workspace_id,
                collection_folders: Vec::new(),
                environments: Vec::new(),
                collection_variables: Vec::new(),
                environment_variables: Vec::new(),
                saved_requests: Vec::new(),
                drafts: Vec::new(),
                tabs: Vec::new(),
            }))
        }

        fn open_unsaved_tab(
            &mut self,
            workspace_id: WorkspaceId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn create_saved_request(
            &mut self,
            workspace_id: WorkspaceId,
            _content: RequestContent,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn create_collection_folder(
            &mut self,
            workspace_id: WorkspaceId,
            _parent_collection_id: Option<CollectionId>,
            _name: String,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn select_environment(
            &mut self,
            workspace_id: WorkspaceId,
            _environment_id: Option<EnvironmentId>,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn rename_collection_folder(
            &mut self,
            workspace_id: WorkspaceId,
            _collection_id: CollectionId,
            _name: String,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn move_collection_folder(
            &mut self,
            workspace_id: WorkspaceId,
            _collection_id: CollectionId,
            _location: CollectionLocation,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn duplicate_collection_folder(
            &mut self,
            workspace_id: WorkspaceId,
            _collection_id: CollectionId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn delete_collection_folder(
            &mut self,
            workspace_id: WorkspaceId,
            _collection_id: CollectionId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn move_saved_request(
            &mut self,
            workspace_id: WorkspaceId,
            _saved_request_id: SavedRequestId,
            _location: CollectionLocation,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn duplicate_saved_request(
            &mut self,
            workspace_id: WorkspaceId,
            _saved_request_id: SavedRequestId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn delete_saved_request(
            &mut self,
            workspace_id: WorkspaceId,
            _saved_request_id: SavedRequestId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn open_saved_request_tab(
            &mut self,
            workspace_id: WorkspaceId,
            _saved_request_id: SavedRequestId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn persist_draft(
            &mut self,
            workspace_id: WorkspaceId,
            draft_id: RequestDraftId,
            content: RequestContent,
        ) -> Result<(), RequestError> {
            self.persisted.push((workspace_id, draft_id, content));
            Ok(())
        }

        fn save_draft(
            &mut self,
            workspace_id: WorkspaceId,
            _draft_id: RequestDraftId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.save_calls += 1;
            self.list_request_workspace(workspace_id)
        }

        fn close_tab(
            &mut self,
            workspace_id: WorkspaceId,
            _tab_id: RequestTabId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.close_calls += 1;
            self.list_request_workspace(workspace_id)
        }

        fn list_execution_history(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<ExecutionHistorySnapshot, RequestError> {
            Ok(ExecutionHistorySnapshot {
                workspace_id,
                disabled: false,
                records: Vec::new(),
                warning: ExecutionHistorySnapshot::warning_text(),
            })
        }

        fn set_execution_history_disabled(
            &mut self,
            workspace_id: WorkspaceId,
            _disabled: bool,
        ) -> Result<ExecutionHistorySnapshot, RequestError> {
            self.list_execution_history(workspace_id)
        }

        fn set_execution_record_pinned(
            &mut self,
            workspace_id: WorkspaceId,
            _record_id: ExecutionRecordId,
            _pinned: bool,
        ) -> Result<ExecutionHistorySnapshot, RequestError> {
            self.list_execution_history(workspace_id)
        }

        fn insert_execution_record(
            &mut self,
            draft: ExecutionRecordDraft,
        ) -> Result<(), RequestError> {
            self.history_records.push(draft);
            Ok(())
        }

        fn open_execution_record_as_draft(
            &mut self,
            workspace_id: WorkspaceId,
            _record_id: ExecutionRecordId,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }

        fn list_cookies(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<Vec<WorkspaceCookie>, RequestError> {
            Ok(self
                .cookies
                .iter()
                .filter(|cookie| cookie.workspace_id == workspace_id)
                .cloned()
                .collect())
        }

        fn upsert_cookie_metadata(
            &mut self,
            draft: CookieDraft,
            has_value: bool,
            secret_reference: Option<&str>,
            _now_epoch_seconds: i64,
        ) -> Result<WorkspaceCookie, RequestError> {
            let id = draft.id.unwrap_or_default();
            let cookie = WorkspaceCookie {
                id,
                workspace_id: draft.workspace_id,
                name: draft.name,
                domain: draft.domain,
                path: draft.path,
                secure: draft.secure,
                http_only: draft.http_only,
                same_site: draft.same_site,
                expires_at_epoch_seconds: draft.expires_at_epoch_seconds,
                session: draft.expires_at_epoch_seconds.is_none(),
                has_value,
                secret_reference: secret_reference.map(str::to_owned),
            };
            self.cookies.retain(|existing| {
                !(existing.workspace_id == cookie.workspace_id
                    && existing.name == cookie.name
                    && existing.domain == cookie.domain
                    && existing.path == cookie.path)
            });
            self.cookies.push(cookie.clone());
            Ok(cookie)
        }

        fn delete_cookie(
            &mut self,
            workspace_id: WorkspaceId,
            cookie_id: CookieId,
        ) -> Result<(), RequestError> {
            let before = self.cookies.len();
            self.cookies
                .retain(|cookie| !(cookie.workspace_id == workspace_id && cookie.id == cookie_id));
            if before == self.cookies.len() {
                return Err(RequestError::NotFound);
            }
            Ok(())
        }

        fn clear_cookies(&mut self, workspace_id: WorkspaceId) -> Result<(), RequestError> {
            self.cookies
                .retain(|cookie| cookie.workspace_id != workspace_id);
            Ok(())
        }

        fn cleanup_expired_cookies(
            &mut self,
            workspace_id: WorkspaceId,
            now_epoch_seconds: i64,
        ) -> Result<Vec<CookieId>, RequestError> {
            let removed = self
                .cookies
                .iter()
                .filter(|cookie| {
                    cookie.workspace_id == workspace_id
                        && cookie
                            .expires_at_epoch_seconds
                            .is_some_and(|expires_at| expires_at <= now_epoch_seconds)
                })
                .map(|cookie| cookie.id)
                .collect::<Vec<_>>();
            self.cookies.retain(|cookie| !removed.contains(&cookie.id));
            Ok(removed)
        }

        fn relink_body_files(
            &mut self,
            workspace_id: WorkspaceId,
            _from_path: String,
            _replacement: BodyFileReference,
        ) -> Result<RequestWorkspaceSnapshot, RequestError> {
            self.list_request_workspace(workspace_id)
        }
    }

    #[test]
    fn queued_draft_updates_flush_only_latest_content() {
        let workspace_id = WorkspaceId::new();
        let draft_id = RequestDraftId::new();
        let mut service = RequestService::new_for_test(FakeRequestRepository::default());

        service.queue_draft_update(workspace_id, draft_id, content("First"));
        service.queue_draft_update(workspace_id, draft_id, content("Second"));
        service.flush_pending_drafts().expect("flush");

        assert_eq!(service.repository.persisted.len(), 1);
        assert_eq!(service.repository.persisted[0].2.name, "Second");
    }

    #[test]
    fn close_cancel_does_not_flush_save_or_close() {
        let workspace_id = WorkspaceId::new();
        let draft_id = RequestDraftId::new();
        let tab_id = RequestTabId::new();
        let repository = FakeRequestRepository {
            snapshot: Some(RequestWorkspaceSnapshot {
                workspace_id,
                collection_folders: Vec::new(),
                environments: Vec::new(),
                collection_variables: Vec::new(),
                environment_variables: Vec::new(),
                saved_requests: Vec::new(),
                drafts: vec![RequestDraft {
                    id: draft_id,
                    workspace_id,
                    saved_request_id: None,
                    content: content("Draft"),
                    is_dirty: true,
                }],
                tabs: vec![RequestTab {
                    id: tab_id,
                    workspace_id,
                    saved_request_id: None,
                    draft_id,
                    position: 0,
                    title: "Draft".to_owned(),
                    is_active: true,
                }],
            }),
            ..Default::default()
        };
        let mut service = RequestService::new_for_test(repository);

        service.queue_draft_update(workspace_id, draft_id, content("Queued"));
        service
            .close_tab(workspace_id, tab_id, CloseTabDecision::Cancel)
            .expect("cancel");

        assert!(service.repository.persisted.is_empty());
        assert_eq!(service.repository.save_calls, 0);
        assert_eq!(service.repository.close_calls, 0);
    }

    #[test]
    fn matching_cookie_is_attached_without_cross_workspace_leakage() {
        let workspace_id = WorkspaceId::new();
        let other_workspace_id = WorkspaceId::new();
        let mut service = RequestService::new_for_test(FakeRequestRepository::default());
        service
            .upsert_cookie(cookie_draft(
                workspace_id,
                "sid",
                "jar-session",
                "example.test",
                "/api",
                false,
                None,
            ))
            .expect("store matching cookie");
        service
            .upsert_cookie(cookie_draft(
                other_workspace_id,
                "sid",
                "wrong-workspace",
                "example.test",
                "/api",
                false,
                None,
            ))
            .expect("store other workspace cookie");

        let content = service
            .attach_matching_cookies(
                workspace_id,
                RequestContent {
                    url: "https://example.test/api/users".to_owned(),
                    ..RequestContent::blank()
                },
            )
            .expect("attach cookies");

        assert_eq!(content.headers.len(), 1);
        assert_eq!(content.headers[0].name, "Cookie");
        assert_eq!(content.headers[0].value, "sid=jar-session");
    }

    #[test]
    fn explicit_cookie_header_wins_for_one_execution() {
        let workspace_id = WorkspaceId::new();
        let mut service = RequestService::new_for_test(FakeRequestRepository::default());
        service
            .upsert_cookie(cookie_draft(
                workspace_id,
                "sid",
                "jar-value",
                "example.test",
                "/",
                false,
                None,
            ))
            .expect("store cookie");

        let content = service
            .attach_matching_cookies(
                workspace_id,
                RequestContent {
                    url: "https://example.test/".to_owned(),
                    headers: vec![OrderedField {
                        enabled: true,
                        order: 0,
                        name: "Cookie".to_owned(),
                        value: "sid=explicit".to_owned(),
                    }],
                    ..RequestContent::blank()
                },
            )
            .expect("attach cookies");

        assert_eq!(content.headers.len(), 1);
        assert_eq!(content.headers[0].value, "sid=explicit");
    }

    #[test]
    fn secure_and_expired_cookies_are_not_attached() {
        let workspace_id = WorkspaceId::new();
        let mut service = RequestService::new_for_test(FakeRequestRepository::default());
        service
            .upsert_cookie(cookie_draft(
                workspace_id,
                "secure_sid",
                "secure",
                "example.test",
                "/",
                true,
                None,
            ))
            .expect("store secure cookie");
        service.repository.cookies.push(WorkspaceCookie {
            id: CookieId::new(),
            workspace_id,
            name: "old".to_owned(),
            domain: "example.test".to_owned(),
            path: "/".to_owned(),
            secure: false,
            http_only: false,
            same_site: None,
            expires_at_epoch_seconds: Some(1),
            session: false,
            has_value: true,
            secret_reference: None,
        });

        let content = service
            .attach_matching_cookies(
                workspace_id,
                RequestContent {
                    url: "http://example.test/".to_owned(),
                    ..RequestContent::blank()
                },
            )
            .expect("attach cookies");

        assert!(content.headers.is_empty());
    }

    #[test]
    fn set_cookie_headers_are_captured_with_default_path() {
        let workspace_id = WorkspaceId::new();
        let mut service = RequestService::new_for_test(FakeRequestRepository::default());

        service
            .capture_set_cookie_headers(
                workspace_id,
                "https://api.example.test/v1/users",
                &[OrderedField {
                    enabled: true,
                    order: 0,
                    name: "Set-Cookie".to_owned(),
                    value: "token=token-value; Secure; HttpOnly; SameSite=Lax".to_owned(),
                }],
            )
            .expect("capture set-cookie");

        let snapshot = service.list_cookies(workspace_id).expect("list cookies");
        assert_eq!(snapshot.cookies.len(), 1);
        assert_eq!(snapshot.cookies[0].name, "token");
        assert_eq!(snapshot.cookies[0].domain, "api.example.test");
        assert_eq!(snapshot.cookies[0].path, "/v1");
        assert!(snapshot.cookies[0].secure);
        assert!(snapshot.cookies[0].http_only);
        assert_eq!(snapshot.cookies[0].same_site, Some(CookieSameSite::Lax));
        assert_eq!(
            service
                .reveal_cookie_value(workspace_id, snapshot.cookies[0].id)
                .expect("reveal"),
            "token-value"
        );
    }

    #[test]
    fn resolver_applies_environment_precedence_and_masks_secret_references() {
        let workspace_id = WorkspaceId::new();
        let environment_id = EnvironmentId::new();
        let snapshot = RequestWorkspaceSnapshot {
            workspace_id,
            collection_folders: Vec::new(),
            environments: vec![Environment {
                id: environment_id,
                workspace_id,
                name: "Production".to_owned(),
                position: 0,
                is_selected: true,
            }],
            collection_variables: vec![CollectionVariable {
                workspace_id,
                variable: Variable {
                    name: "baseUrl".to_owned(),
                    value: VariableValue::Plain("https://collection.example.test".to_owned()),
                },
            }],
            environment_variables: vec![
                EnvironmentVariable {
                    environment_id,
                    workspace_id,
                    variable: Variable {
                        name: "baseUrl".to_owned(),
                        value: VariableValue::Plain("https://env.example.test".to_owned()),
                    },
                },
                EnvironmentVariable {
                    environment_id,
                    workspace_id,
                    variable: Variable {
                        name: "token".to_owned(),
                        value: VariableValue::SecretReference("secret://token".to_owned()),
                    },
                },
            ],
            saved_requests: Vec::new(),
            drafts: Vec::new(),
            tabs: Vec::new(),
        };
        let content = RequestContent {
            url: "{{baseUrl}}/users".to_owned(),
            headers: vec![OrderedField {
                enabled: true,
                order: 0,
                name: "Authorization".to_owned(),
                value: "Bearer {{token}}".to_owned(),
            }],
            ..RequestContent::blank()
        };

        let resolved = resolve_request_content(&snapshot, &content);

        assert_eq!(resolved.url.value, "https://env.example.test/users");
        assert_eq!(resolved.headers[0].value.value, "Bearer ********");
        assert!(resolved.headers[0].value.contains_secret);
        assert!(resolved.errors.is_empty());
        assert_eq!(
            resolved
                .references
                .iter()
                .find(|reference| reference.name == "baseUrl")
                .expect("baseUrl")
                .source,
            VariableSource::Environment
        );
    }

    #[test]
    fn resolver_applies_auth_after_variables_and_marks_unsafe_tls_visible() {
        let workspace_id = WorkspaceId::new();
        let environment_id = EnvironmentId::new();
        let snapshot = RequestWorkspaceSnapshot {
            workspace_id,
            collection_folders: Vec::new(),
            environments: vec![Environment {
                id: environment_id,
                workspace_id,
                name: "Prod".to_owned(),
                position: 0,
                is_selected: true,
            }],
            collection_variables: Vec::new(),
            environment_variables: vec![EnvironmentVariable {
                environment_id,
                workspace_id,
                variable: Variable {
                    name: "token".to_owned(),
                    value: VariableValue::SecretReference("secret://token".to_owned()),
                },
            }],
            saved_requests: Vec::new(),
            drafts: Vec::new(),
            tabs: Vec::new(),
        };
        let content = RequestContent {
            auth: RequestAuth::Bearer {
                token: "{{token}}".to_owned(),
            },
            tls: crate::domain::request::TlsPolicy {
                verify: false,
                ..Default::default()
            },
            ..RequestContent::blank()
        };

        let resolved = resolve_request_content(&snapshot, &content);

        assert!(resolved.unsafe_tls_visible);
        assert!(resolved.headers.iter().any(|field| {
            field.name.value == "Authorization"
                && field.value.value == REDACTED_VALUE
                && field.value.contains_secret
        }));
    }

    #[test]
    fn record_execution_redacts_authorization_cookie_and_secret_resolved_values() {
        let workspace_id = WorkspaceId::new();
        let environment_id = EnvironmentId::new();
        let repository = FakeRequestRepository {
            snapshot: Some(RequestWorkspaceSnapshot {
                workspace_id,
                collection_folders: Vec::new(),
                environments: vec![Environment {
                    id: environment_id,
                    workspace_id,
                    name: "Production".to_owned(),
                    position: 0,
                    is_selected: true,
                }],
                collection_variables: Vec::new(),
                environment_variables: vec![EnvironmentVariable {
                    environment_id,
                    workspace_id,
                    variable: Variable {
                        name: "token".to_owned(),
                        value: VariableValue::SecretReference("secret://token".to_owned()),
                    },
                }],
                saved_requests: Vec::new(),
                drafts: Vec::new(),
                tabs: Vec::new(),
            }),
            ..Default::default()
        };
        let mut service = RequestService::new_for_test(repository);
        let content = RequestContent {
            name: "Secret request".to_owned(),
            method: "POST".to_owned(),
            url: "https://example.test?token={{token}}".to_owned(),
            body: RequestBody::Raw {
                content: "{\"token\":\"{{token}}\"}".to_owned(),
            },
            query: vec![OrderedField {
                enabled: true,
                order: 0,
                name: "api_key".to_owned(),
                value: "{{token}}".to_owned(),
            }],
            headers: vec![
                OrderedField {
                    enabled: true,
                    order: 0,
                    name: "Authorization".to_owned(),
                    value: "Bearer plain-token".to_owned(),
                },
                OrderedField {
                    enabled: true,
                    order: 1,
                    name: "Cookie".to_owned(),
                    value: "sid=plain-cookie".to_owned(),
                },
                OrderedField {
                    enabled: true,
                    order: 2,
                    name: "X-Token".to_owned(),
                    value: "{{token}}".to_owned(),
                },
            ],
            ..RequestContent::blank()
        };

        service
            .record_execution(
                workspace_id,
                content,
                ExecutionRecordResponse {
                    status: Some(200),
                    headers: vec![OrderedField {
                        enabled: true,
                        order: 0,
                        name: "Set-Cookie".to_owned(),
                        value: "safe-for-now".to_owned(),
                    }],
                    body_preview: "unknown-body-secret-is-not-detectable".to_owned(),
                    body_truncated: false,
                    error: None,
                    duration_ms: Some(25),
                },
                1_800_000_000,
            )
            .expect("record execution");

        let record = service
            .repository
            .history_records
            .first()
            .expect("history record");
        assert_eq!(record.content.url, REDACTED_VALUE);
        assert_eq!(
            record.content.body,
            RequestBody::Raw {
                content: REDACTED_VALUE.to_owned()
            }
        );
        assert_eq!(record.content.query[0].value, REDACTED_VALUE);
        assert_eq!(record.content.headers[0].value, REDACTED_VALUE);
        assert_eq!(record.content.headers[1].value, REDACTED_VALUE);
        assert_eq!(record.content.headers[2].value, REDACTED_VALUE);
        assert!(record.response.body_preview.contains("unknown-body-secret"));
    }

    #[test]
    fn resolver_reports_stable_missing_and_cycle_errors() {
        let workspace_id = WorkspaceId::new();
        let snapshot = RequestWorkspaceSnapshot {
            workspace_id,
            collection_folders: Vec::new(),
            environments: Vec::new(),
            collection_variables: vec![
                CollectionVariable {
                    workspace_id,
                    variable: Variable {
                        name: "a".to_owned(),
                        value: VariableValue::Plain("{{b}}".to_owned()),
                    },
                },
                CollectionVariable {
                    workspace_id,
                    variable: Variable {
                        name: "b".to_owned(),
                        value: VariableValue::Plain("{{a}}".to_owned()),
                    },
                },
            ],
            environment_variables: Vec::new(),
            saved_requests: Vec::new(),
            drafts: Vec::new(),
            tabs: Vec::new(),
        };
        let content = RequestContent {
            url: "{{missing}}/{{a}}".to_owned(),
            ..RequestContent::blank()
        };

        let resolved = resolve_request_content(&snapshot, &content);

        assert_eq!(resolved.url.value, "{{missing}}/{{a}}");
        assert_eq!(
            resolved
                .errors
                .iter()
                .map(|error| (error.name.as_str(), error.kind))
                .collect::<Vec<_>>(),
            vec![
                ("a", VariableResolutionErrorKind::Cycle),
                ("missing", VariableResolutionErrorKind::Missing)
            ]
        );
    }

    fn content(name: &str) -> RequestContent {
        RequestContent {
            name: name.to_owned(),
            method: "GET".to_owned(),
            url: "https://example.test".to_owned(),
            body: RequestBody::None,
            query: vec![OrderedField {
                enabled: true,
                order: 0,
                name: "duplicate".to_owned(),
                value: String::new(),
            }],
            headers: Vec::new(),
            ..RequestContent::blank()
        }
    }

    fn cookie_draft(
        workspace_id: WorkspaceId,
        name: &str,
        value: &str,
        domain: &str,
        path: &str,
        secure: bool,
        expires_at_epoch_seconds: Option<i64>,
    ) -> CookieDraft {
        CookieDraft {
            id: None,
            workspace_id,
            name: name.to_owned(),
            value: value.to_owned(),
            domain: domain.to_owned(),
            path: path.to_owned(),
            secure,
            http_only: false,
            same_site: None,
            expires_at_epoch_seconds,
        }
    }
}
