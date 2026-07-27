use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{
    request::{
        RequestContent, RequestDraft, RequestDraftId, RequestTab, RequestTabId, SavedRequest,
        SavedRequestId,
    },
    workspace::WorkspaceId,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestWorkspaceSnapshot {
    pub workspace_id: WorkspaceId,
    pub saved_requests: Vec<SavedRequest>,
    pub drafts: Vec<RequestDraft>,
    pub tabs: Vec<RequestTab>,
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

impl RequestError {
    pub fn persistence(error: impl std::error::Error) -> Self {
        Self::Persistence(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::request::OrderedField;

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
