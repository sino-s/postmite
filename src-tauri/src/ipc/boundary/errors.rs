
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
