pub mod application;
pub mod diagnostics;
pub mod domain;
pub mod infrastructure;
pub mod ipc;

use std::{fs, sync::Mutex, time::Instant};

use application::{request::RequestService, workspace::WorkspaceService};
use infrastructure::sqlite::SqliteWorkspaceRepository;
use tauri::Manager;

pub struct AppState {
    pub workspaces: Mutex<WorkspaceService<SqliteWorkspaceRepository>>,
    pub requests: Mutex<RequestService<SqliteWorkspaceRepository>>,
}

impl AppState {
    fn new(
        workspaces: WorkspaceService<SqliteWorkspaceRepository>,
        requests: RequestService<SqliteWorkspaceRepository>,
    ) -> Self {
        Self {
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
            ipc::open_saved_request_tab,
            ipc::update_request_draft,
            ipc::flush_request_drafts,
            ipc::save_request_draft,
            ipc::close_request_tab,
        ])
        .setup(move |app| {
            let app_data_dir = diagnostics::app_data_dir(app)?;
            fs::create_dir_all(&app_data_dir)?;
            let database_path = app_data_dir.join("postmite.sqlite3");

            let workspace_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let request_repository = SqliteWorkspaceRepository::open(&database_path)?;
            let mut workspaces = WorkspaceService::new(workspace_repository);
            workspaces.initialize()?;
            let requests = RequestService::new(request_repository);

            app.manage(AppState::new(workspaces, requests));
            diagnostics::configure_perf(app, started_at.elapsed())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Postmite");
}
