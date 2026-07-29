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
