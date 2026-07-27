pub mod application;
pub mod diagnostics;
pub mod domain;
pub mod infrastructure;
pub mod ipc;

use std::{fs, sync::Mutex, time::Instant};

use application::workspace::WorkspaceService;
use infrastructure::sqlite::SqliteWorkspaceRepository;
use tauri::Manager;

pub struct AppState {
    pub workspaces: Mutex<WorkspaceService<SqliteWorkspaceRepository>>,
}

impl AppState {
    fn new(workspaces: WorkspaceService<SqliteWorkspaceRepository>) -> Self {
        Self {
            workspaces: Mutex::new(workspaces),
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
        ])
        .setup(move |app| {
            let app_data_dir = diagnostics::app_data_dir(app)?;
            fs::create_dir_all(&app_data_dir)?;

            let repository =
                SqliteWorkspaceRepository::open(app_data_dir.join("postmite.sqlite3"))?;
            let mut workspaces = WorkspaceService::new(repository);
            workspaces.initialize()?;

            app.manage(AppState::new(workspaces));
            diagnostics::configure_perf(app, started_at.elapsed())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Postmite");
}
