use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{
    request::{
        CollectionFolder, CollectionId, CollectionVariable, Environment, EnvironmentId,
        EnvironmentVariable, RequestContent, RequestDraft, RequestDraftId, RequestTab,
        RequestTabId, SavedRequest, SavedRequestId, VariableValue,
    },
    workspace::WorkspaceId,
};

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
}

pub struct RequestService<R>
where
    R: RequestRepository,
{
    repository: R,
    pending_drafts: HashMap<RequestDraftId, PendingDraft>,
}

impl<R> RequestService<R>
where
    R: RequestRepository,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            pending_drafts: HashMap::new(),
        }
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
    pub query: Vec<ResolvedField>,
    pub headers: Vec<ResolvedField>,
    pub references: Vec<ResolvedVariableReference>,
    pub errors: Vec<VariableResolutionError>,
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
    let scope = VariableScope::from_snapshot(snapshot);
    let mut state = ResolutionState::default();
    let url = resolve_text(&content.url, &scope, &mut state);
    let body = resolve_text(&content.body, &scope, &mut state);
    let query = content
        .query
        .iter()
        .map(|field| ResolvedField {
            enabled: field.enabled,
            order: field.order,
            name: resolve_text(&field.name, &scope, &mut state),
            value: resolve_text(&field.value, &scope, &mut state),
        })
        .collect();
    let headers = content
        .headers
        .iter()
        .map(|field| ResolvedField {
            enabled: field.enabled,
            order: field.order,
            name: resolve_text(&field.name, &scope, &mut state),
            value: resolve_text(&field.value, &scope, &mut state),
        })
        .collect();

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
        query,
        headers,
        references,
        errors,
    }
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

fn resolve_text(input: &str, scope: &VariableScope, state: &mut ResolutionState) -> ResolvedValue {
    resolve_text_with_stack(input, scope, state, &mut Vec::new())
}

fn resolve_text_with_stack(
    input: &str,
    scope: &VariableScope,
    state: &mut ResolutionState,
    stack: &mut Vec<String>,
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
        let resolved = resolve_variable(name, scope, state, stack);
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
        VariableValue::Plain(value) => resolve_text_with_stack(value, scope, state, stack),
        VariableValue::SecretReference(_) => ResolvedValue {
            value: "********".to_owned(),
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
        CollectionVariable, Environment, EnvironmentVariable, OrderedField, Variable, VariableValue,
    };

    #[derive(Default)]
    struct FakeRequestRepository {
        persisted: Vec<(WorkspaceId, RequestDraftId, RequestContent)>,
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
    }

    #[test]
    fn queued_draft_updates_flush_only_latest_content() {
        let workspace_id = WorkspaceId::new();
        let draft_id = RequestDraftId::new();
        let mut service = RequestService::new(FakeRequestRepository::default());

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
        let mut service = RequestService::new(repository);

        service.queue_draft_update(workspace_id, draft_id, content("Queued"));
        service
            .close_tab(workspace_id, tab_id, CloseTabDecision::Cancel)
            .expect("cancel");

        assert!(service.repository.persisted.is_empty());
        assert_eq!(service.repository.save_calls, 0);
        assert_eq!(service.repository.close_calls, 0);
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
            body: String::new(),
            query: vec![OrderedField {
                enabled: true,
                order: 0,
                name: "duplicate".to_owned(),
                value: String::new(),
            }],
            headers: Vec::new(),
        }
    }
}
