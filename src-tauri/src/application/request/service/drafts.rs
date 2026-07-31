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
        RequestTabId, SavedRequest, SavedRequestId, Variable, VariableValue, WorkspaceCookie,
    },
    workspace::WorkspaceId,
};

use crate::application::secrets::{
    SecretClass, SecretOwner, SecretPersistence, SecretStore,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentVariableDraft {
    pub previous_name: Option<String>,
    pub name: String,
    pub value: EnvironmentVariableDraftValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentVariableDraftValue {
    Plain(String),
    Secret { value: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentMutationResult {
    pub snapshot: RequestWorkspaceSnapshot,
    pub secret_persistence: Option<SecretPersistence>,
}

fn cleanup_secret_references(secrets: &dyn SecretStore, references: &[String]) {
    for reference in references {
        let _ = secrets.delete(reference);
    }
}

fn execution_active_content(content: &RequestContent) -> RequestContent {
    let mut active = content.clone();
    active.query.retain(|field| field.enabled);
    active.headers.retain(|field| field.enabled);
    match &mut active.body {
        RequestBody::UrlEncoded { fields } => fields.retain(|field| field.enabled),
        RequestBody::Multipart { parts } => parts.retain(|part| match part {
            MultipartPart::Field { enabled, .. } | MultipartPart::File { enabled, .. } => *enabled,
        }),
        RequestBody::None | RequestBody::Raw { .. } | RequestBody::Binary { .. } => {}
    }
    active
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
    fn open_unsaved_tab_with_content(
        &mut self,
        workspace_id: WorkspaceId,
        content: RequestContent,
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
    fn create_environment(
        &mut self,
        workspace_id: WorkspaceId,
        name: String,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn update_environment(
        &mut self,
        workspace_id: WorkspaceId,
        environment_id: EnvironmentId,
        name: String,
        variables: Vec<Variable>,
    ) -> Result<RequestWorkspaceSnapshot, RequestError>;
    fn delete_environment(
        &mut self,
        workspace_id: WorkspaceId,
        environment_id: EnvironmentId,
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
            Arc::new(crate::application::secrets::SessionSecretStore::new()),
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

    pub fn open_unsaved_tab_with_content(
        &mut self,
        workspace_id: WorkspaceId,
        content: RequestContent,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository
            .open_unsaved_tab_with_content(workspace_id, content)
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

    pub fn create_environment(
        &mut self,
        workspace_id: WorkspaceId,
        name: impl Into<String>,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        self.repository.create_environment(workspace_id, name.into())
    }

    pub fn update_environment(
        &mut self,
        workspace_id: WorkspaceId,
        environment_id: EnvironmentId,
        name: impl Into<String>,
        drafts: Vec<EnvironmentVariableDraft>,
    ) -> Result<EnvironmentMutationResult, RequestError> {
        let current = self.repository.list_request_workspace(workspace_id)?;
        if !current
            .environments
            .iter()
            .any(|environment| environment.id == environment_id)
        {
            return Err(RequestError::NotFound);
        }

        let current_variables = current
            .environment_variables
            .iter()
            .filter(|variable| variable.environment_id == environment_id)
            .map(|variable| (variable.variable.name.clone(), variable.variable.value.clone()))
            .collect::<HashMap<_, _>>();
        let mut names = std::collections::HashSet::new();
        let mut created_references = Vec::new();
        let mut retained_references = std::collections::HashSet::new();
        let mut secret_persistence = None;
        let mut variables = Vec::with_capacity(drafts.len());

        for draft in drafts {
            let variable_name = draft.name.trim();
            if variable_name.is_empty()
                || variable_name.chars().count() > 256
                || variable_name.chars().any(char::is_control)
            {
                cleanup_secret_references(self.secrets.as_ref(), &created_references);
                return Err(RequestError::InvalidInput(
                    "environment.variable.name.invalid".to_owned(),
                ));
            }
            if !names.insert(variable_name.to_owned()) {
                cleanup_secret_references(self.secrets.as_ref(), &created_references);
                return Err(RequestError::InvalidInput(
                    "environment.variable.name.duplicate".to_owned(),
                ));
            }

            let value = match draft.value {
                EnvironmentVariableDraftValue::Plain(value) => VariableValue::Plain(value),
                EnvironmentVariableDraftValue::Secret { value: Some(value) } => {
                    let write = self
                        .secrets
                        .put(
                            &SecretOwner::new(
                                workspace_id,
                                SecretClass::ProtectedVariable,
                                format!("environment:{environment_id}:{variable_name}"),
                            ),
                            &value,
                        )
                        .map_err(|error| {
                            cleanup_secret_references(
                                self.secrets.as_ref(),
                                &created_references,
                            );
                            RequestError::Persistence(error.to_string())
                        })?;
                    secret_persistence = Some(match (secret_persistence, write.persistence) {
                        (Some(SecretPersistence::SessionOnly), _) => {
                            SecretPersistence::SessionOnly
                        }
                        (_, persistence) => persistence,
                    });
                    created_references.push(write.reference.clone());
                    retained_references.insert(write.reference.clone());
                    VariableValue::SecretReference(write.reference)
                }
                EnvironmentVariableDraftValue::Secret { value: None } => {
                    let previous_name = draft.previous_name.as_deref().ok_or_else(|| {
                        cleanup_secret_references(self.secrets.as_ref(), &created_references);
                        RequestError::InvalidInput(
                            "environment.variable.secret.required".to_owned(),
                        )
                    })?;
                    let reference = match current_variables.get(previous_name) {
                        Some(VariableValue::SecretReference(reference)) => reference.clone(),
                        _ => {
                            cleanup_secret_references(
                                self.secrets.as_ref(),
                                &created_references,
                            );
                            return Err(RequestError::InvalidInput(
                                "environment.variable.secret.required".to_owned(),
                            ));
                        }
                    };
                    retained_references.insert(reference.clone());
                    VariableValue::SecretReference(reference)
                }
            };
            variables.push(Variable {
                name: variable_name.to_owned(),
                value,
            });
        }

        let snapshot = match self.repository.update_environment(
            workspace_id,
            environment_id,
            name.into(),
            variables,
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                cleanup_secret_references(self.secrets.as_ref(), &created_references);
                return Err(error);
            }
        };

        for value in current_variables.values() {
            if let VariableValue::SecretReference(reference) = value {
                if !retained_references.contains(reference) {
                    let _ = self.secrets.delete(reference);
                }
            }
        }

        Ok(EnvironmentMutationResult {
            snapshot,
            secret_persistence,
        })
    }

    pub fn delete_environment(
        &mut self,
        workspace_id: WorkspaceId,
        environment_id: EnvironmentId,
    ) -> Result<RequestWorkspaceSnapshot, RequestError> {
        let current = self.repository.list_request_workspace(workspace_id)?;
        let references = current
            .environment_variables
            .iter()
            .filter(|variable| variable.environment_id == environment_id)
            .filter_map(|variable| match &variable.variable.value {
                VariableValue::SecretReference(reference) => Some(reference.clone()),
                VariableValue::Plain(_) => None,
            })
            .collect::<Vec<_>>();
        let snapshot = self
            .repository
            .delete_environment(workspace_id, environment_id)?;
        cleanup_secret_references(self.secrets.as_ref(), &references);
        Ok(snapshot)
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
        let active_references = std::cell::RefCell::new(std::collections::HashSet::new());
        let _ = materialize_request_auth_with_secret_resolver(
            &snapshot,
            execution_active_content(&content),
            &|reference| {
                active_references.borrow_mut().insert(reference.to_owned());
                Some(REDACTED_VALUE.to_owned())
            },
        );
        let missing_secret = std::cell::Cell::new(false);
        let tracked_secret_resolver = |reference: &str| {
            if !active_references.borrow().contains(reference) {
                return Some(REDACTED_VALUE.to_owned());
            }
            let value = self.secrets.get(reference).ok();
            if value.is_none() {
                missing_secret.set(true);
            }
            value
        };
        let materialized = materialize_request_auth_with_secret_resolver(
            &snapshot,
            content,
            &tracked_secret_resolver,
        );
        if missing_secret.get() {
            return Err(RequestError::InvalidInput(
                "request.secret.unavailable".to_owned(),
            ));
        }
        Ok(materialized)
    }

    pub fn materialize_request_content_for_curl(
        &self,
        workspace_id: WorkspaceId,
        expected_environment_id: Option<EnvironmentId>,
        content: RequestContent,
    ) -> Result<RequestContent, RequestError> {
        let snapshot = self.repository.list_request_workspace(workspace_id)?;
        materialize_request_content_for_curl(
            &snapshot,
            expected_environment_id,
            content,
            &|reference| self.secrets.get(reference).ok(),
        )
    }

    pub fn selected_environment_id(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Option<EnvironmentId>, RequestError> {
        let snapshot = self.repository.list_request_workspace(workspace_id)?;
        Ok(snapshot
            .environments
            .into_iter()
            .find(|environment| environment.is_selected)
            .map(|environment| environment.id))
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
