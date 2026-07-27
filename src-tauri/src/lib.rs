pub mod application;
pub mod diagnostics;
pub mod domain;
pub mod infrastructure;
pub mod ipc;

use std::{fs, sync::Mutex};

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
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;

            let repository =
                SqliteWorkspaceRepository::open(app_data_dir.join("postmite.sqlite3"))?;
            let mut workspaces = WorkspaceService::new(repository);
            workspaces.initialize()?;

            app.manage(AppState::new(workspaces));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Postmite");
}
