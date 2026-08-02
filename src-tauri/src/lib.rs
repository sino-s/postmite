pub mod application;
pub mod diagnostics;
pub mod domain;
pub mod infrastructure;
pub mod ipc;

use std::{
    env, fs,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime},
};

#[cfg(target_os = "linux")]
use application::secrets::FallbackSecretStore;
use application::{
    backup::NativeBackupService,
    execution::ExecutionCoordinator,
    oauth::{OAuthCoordinator, SystemBrowserLauncher},
    postman_import::PostmanImportService,
    request::RequestService,
    secrets::{SecretStore, SessionSecretStore},
    workspace::WorkspaceService,
};
#[cfg(target_os = "linux")]
use infrastructure::secrets::LinuxSecretServiceStore;
use infrastructure::{
    http::{cleanup_all_response_temp_files, cleanup_expired_response_temp_files},
    sqlite::{DatabaseRecoveryMode, DatabaseRecoveryState, SqliteWorkspaceRepository},
};
use tauri::{Manager, WindowEvent};
use tauri_plugin_window_state::StateFlags;

const SESSION_ONLY_SECRETS_ENV: &str = "POSTMITE_SESSION_ONLY_SECRETS";

fn create_secret_store(force_session_only: bool) -> Arc<dyn SecretStore> {
    let session = Arc::new(SessionSecretStore::new());
    if force_session_only {
        return session;
    }

    #[cfg(target_os = "linux")]
    {
        Arc::new(FallbackSecretStore::new(
            LinuxSecretServiceStore::new(),
            session,
        ))
    }

    #[cfg(not(target_os = "linux"))]
    {
        session
    }
}

pub struct AppState {
    pub database_recovery: DatabaseRecoveryState,
    pub diagnostics: diagnostics::DiagnosticsService,
    pub executions: Arc<ExecutionCoordinator>,
    pub oauth: Arc<OAuthCoordinator>,
    pub secrets: Arc<dyn SecretStore>,
    pub workspaces: Mutex<WorkspaceService<SqliteWorkspaceRepository>>,
    pub requests: Mutex<RequestService<SqliteWorkspaceRepository>>,
    pub postman_imports: Mutex<PostmanImportService<SqliteWorkspaceRepository>>,
    pub native_backups: Mutex<NativeBackupService<SqliteWorkspaceRepository>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let started_at = Instant::now();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE)
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            ipc::list_workspaces,
            ipc::create_workspace,
            ipc::rename_workspace,
            ipc::set_workspace_base_directory,
            ipc::switch_workspace,
            ipc::delete_workspace,
            ipc::list_request_workspace,
            ipc::open_unsaved_request_tab,
            ipc::create_saved_request,
            ipc::create_collection_folder,
            ipc::select_environment,
            ipc::create_environment,
            ipc::update_environment,
            ipc::delete_environment,
            ipc::resolve_request_content,
            ipc::rename_collection_folder,
            ipc::move_collection_folder,
            ipc::duplicate_collection_folder,
            ipc::delete_collection_folder,
            ipc::move_saved_request,
            ipc::duplicate_saved_request,
            ipc::delete_saved_request,
            ipc::open_saved_request_tab,
            ipc::update_request_draft,
            ipc::flush_request_drafts,
            ipc::save_request_draft,
            ipc::close_request_tab,
            ipc::list_execution_history,
            ipc::set_execution_history_disabled,
            ipc::set_execution_record_pinned,
            ipc::open_execution_record_as_draft,
            ipc::list_cookies,
            ipc::upsert_cookie,
            ipc::delete_cookie,
            ipc::clear_cookies,
            ipc::reveal_cookie_value,
            ipc::describe_body_file,
            ipc::relink_body_files,
            ipc::preview_postman_import,
            ipc::import_postman,
            ipc::export_postman,
            ipc::preview_postman_reimport,
            ipc::reimport_postman,
            ipc::export_native_backup,
            ipc::preview_native_backup_restore,
            ipc::restore_native_backup,
            ipc::get_database_recovery_state,
            ipc::export_recoverable_database,
            ipc::get_diagnostic_bundle_preview,
            ipc::set_diagnostic_debug_logging,
            ipc::record_frontend_execution_trace,
            ipc::export_diagnostic_bundle,
            ipc::check_for_update,
            ipc::preview_curl_import,
            ipc::import_curl_as_draft,
            ipc::generate_curl,
            ipc::start_request_execution,
            ipc::cancel_request_execution,
            ipc::save_response_file,
            ipc::start_oauth_authorization,
            ipc::cancel_oauth_authorization,
        ])
        .setup(move |app| {
            let app_data_dir = diagnostics::app_data_dir(app)?;
            fs::create_dir_all(&app_data_dir)?;
            cleanup_expired_response_temp_files(SystemTime::now());
            let database_path = app_data_dir.join("postmite.sqlite3");
            let diagnostics = diagnostics::DiagnosticsService::new(&app_data_dir)?;

            let workspace_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let database_recovery = workspace_repository.recovery_state();
            let request_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let postman_import_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let native_backup_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let secrets = create_secret_store(env::var_os(SESSION_ONLY_SECRETS_ENV).is_some());
            let mut workspaces = WorkspaceService::new(workspace_repository, Arc::clone(&secrets));
            let workspace_snapshot = if database_recovery.mode == DatabaseRecoveryMode::Normal {
                Some(workspaces.initialize()?)
            } else {
                workspaces.initialize().ok()
            };
            let mut requests = RequestService::new(request_repository, Arc::clone(&secrets));
            let postman_imports =
                PostmanImportService::new(postman_import_repository, Arc::clone(&secrets));
            let native_backups = NativeBackupService::new(native_backup_repository);
            if database_recovery.mode == DatabaseRecoveryMode::Normal {
                let workspace_snapshot = workspace_snapshot
                    .as_ref()
                    .expect("normal database initializes workspace state");
                diagnostics::configure_perf_request_tabs(
                    &mut requests,
                    workspace_snapshot.selected_workspace_id,
                )?;
            }
            let executions = Arc::new(ExecutionCoordinator::new());
            let oauth = Arc::new(OAuthCoordinator::new(Arc::new(SystemBrowserLauncher)));

            diagnostics::configure_e2e_request_smoke(Arc::clone(&executions))?;
            if database_recovery.mode == DatabaseRecoveryMode::Normal {
                let workspace_snapshot = workspace_snapshot
                    .as_ref()
                    .expect("normal database initializes workspace state");
                diagnostics::configure_e2e_security(
                    &mut requests,
                    workspace_snapshot.selected_workspace_id,
                    Arc::clone(&secrets),
                )?;
            }
            app.manage(AppState {
                database_recovery,
                diagnostics,
                executions,
                oauth,
                secrets,
                workspaces: Mutex::new(workspaces),
                requests: Mutex::new(requests),
                postman_imports: Mutex::new(postman_imports),
                native_backups: Mutex::new(native_backups),
            });
            let state = app.state::<AppState>();
            state.diagnostics.record_startup(
                match state.database_recovery.mode {
                    DatabaseRecoveryMode::Normal => "normal",
                    DatabaseRecoveryMode::Safe => "safe",
                },
                started_at.elapsed(),
            );
            diagnostics::configure_perf(app, started_at.elapsed())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                state.executions.cancel_all();
                cleanup_all_response_temp_files();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Postmite");
}

#[cfg(all(test, not(target_os = "linux")))]
mod tests {
    use super::*;
    use crate::{
        application::secrets::{SecretClass, SecretPersistence},
        domain::workspace::WorkspaceId,
    };

    #[test]
    fn non_linux_default_secret_store_is_session_only() {
        let owner = application::secrets::SecretOwner::new(
            WorkspaceId::new(),
            SecretClass::ProtectedVariable,
            "cross-platform",
        );
        let write = create_secret_store(false)
            .put(&owner, "")
            .expect("non-Linux secret store is available for the session");

        assert_eq!(write.persistence, SecretPersistence::SessionOnly);
    }
}
