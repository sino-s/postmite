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

        fn open_unsaved_tab_with_content(
            &mut self,
            workspace_id: WorkspaceId,
            _content: RequestContent,
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
