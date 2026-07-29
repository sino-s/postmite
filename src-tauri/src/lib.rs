pub mod application;
pub mod diagnostics;
pub mod domain;
pub mod infrastructure;
pub mod ipc;

use std::{
    fs,
    sync::{Arc, Mutex},
    time::Instant,
};

use application::{
    execution::ExecutionCoordinator,
    oauth::{OAuthCoordinator, SystemBrowserLauncher},
    request::RequestService,
    secrets::{FallbackSecretStore, SecretStore, SessionSecretStore},
    workspace::WorkspaceService,
};
use infrastructure::{secrets::LinuxSecretServiceStore, sqlite::SqliteWorkspaceRepository};
use tauri::{Manager, WindowEvent};

pub struct AppState {
    pub executions: Arc<ExecutionCoordinator>,
    pub oauth: Arc<OAuthCoordinator>,
    pub secrets: Arc<dyn SecretStore>,
    pub workspaces: Mutex<WorkspaceService<SqliteWorkspaceRepository>>,
    pub requests: Mutex<RequestService<SqliteWorkspaceRepository>>,
}

impl AppState {
    fn new(
        executions: Arc<ExecutionCoordinator>,
        oauth: Arc<OAuthCoordinator>,
        secrets: Arc<dyn SecretStore>,
        workspaces: WorkspaceService<SqliteWorkspaceRepository>,
        requests: RequestService<SqliteWorkspaceRepository>,
    ) -> Self {
        Self {
            executions,
            oauth,
            secrets,
            workspaces: Mutex::new(workspaces),
            requests: Mutex::new(requests),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let started_at = Instant::now();

    tauri::Builder::default()
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
            ipc::start_request_execution,
            ipc::cancel_request_execution,
            ipc::start_oauth_authorization,
            ipc::cancel_oauth_authorization,
        ])
        .setup(move |app| {
            let app_data_dir = diagnostics::app_data_dir(app)?;
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("postmite.sqlite3");

            let workspace_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let request_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let secrets: Arc<dyn SecretStore> = Arc::new(FallbackSecretStore::new(
                LinuxSecretServiceStore::new(),
                Arc::new(SessionSecretStore::new()),
            ));
            let mut workspaces = WorkspaceService::new(workspace_repository, Arc::clone(&secrets));
            let workspace_snapshot = workspaces.initialize()?;
            let mut requests = RequestService::new(request_repository, Arc::clone(&secrets));
            diagnostics::configure_perf_request_tabs(
                &mut requests,
                workspace_snapshot.selected_workspace_id,
            )?;
            let executions = Arc::new(ExecutionCoordinator::new());
            let oauth = Arc::new(OAuthCoordinator::new(Arc::new(SystemBrowserLauncher)));

            diagnostics::configure_e2e_request_smoke(Arc::clone(&executions))?;
            app.manage(AppState::new(
                executions, oauth, secrets, workspaces, requests,
            ));
            diagnostics::configure_perf(app, started_at.elapsed())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                state.executions.cancel_all();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Postmite");
}
