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
    execution::ExecutionCoordinator, request::RequestService, workspace::WorkspaceService,
};
use infrastructure::sqlite::SqliteWorkspaceRepository;
use tauri::Manager;

pub struct AppState {
    pub executions: Arc<ExecutionCoordinator>,
    pub workspaces: Mutex<WorkspaceService<SqliteWorkspaceRepository>>,
    pub requests: Mutex<RequestService<SqliteWorkspaceRepository>>,
}

impl AppState {
    fn new(
        executions: Arc<ExecutionCoordinator>,
        workspaces: WorkspaceService<SqliteWorkspaceRepository>,
        requests: RequestService<SqliteWorkspaceRepository>,
    ) -> Self {
        Self {
            executions,
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
            ipc::start_request_execution,
            ipc::cancel_request_execution,
        ])
        .setup(move |app| {
            let app_data_dir = diagnostics::app_data_dir(app)?;
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("postmite.sqlite3");

            let workspace_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let request_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let mut workspaces = WorkspaceService::new(workspace_repository);
            let workspace_snapshot = workspaces.initialize()?;
            let mut requests = RequestService::new(request_repository);
            diagnostics::configure_perf_request_tabs(
                &mut requests,
                workspace_snapshot.selected_workspace_id,
            )?;
            let executions = Arc::new(ExecutionCoordinator::new());

            diagnostics::configure_e2e_request_smoke(Arc::clone(&executions))?;
            app.manage(AppState::new(executions, workspaces, requests));
            diagnostics::configure_perf(app, started_at.elapsed())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Postmite");
}
