//! Redacted local diagnostics boundary.

use std::{env, error::Error, fs, path::PathBuf, str::FromStr, time::Duration};

use tauri::{App, Manager, WebviewUrl, WebviewWindowBuilder};

const PERF_APP_DATA_DIR_ENV: &str = "POSTMITE_PERF_APP_DATA_DIR";
const PERF_READY_FILE_ENV: &str = "POSTMITE_PERF_READY_FILE";
const PERF_TAB_COUNT_ENV: &str = "POSTMITE_PERF_TAB_COUNT";
const DEFAULT_PERF_TAB_COUNT: u8 = 1;
const MAX_PERF_TAB_COUNT: u8 = 10;

#[derive(Debug, Eq, PartialEq)]
struct PerfSettings {
    ready_file: Option<PathBuf>,
    tab_count: u8,
}

pub fn app_data_dir(app: &App) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = env::var_os(PERF_APP_DATA_DIR_ENV) {
        return Ok(PathBuf::from(path));
    }

    Ok(app.path().app_data_dir()?)
}

pub fn configure_perf(app: &mut App, ready_after: Duration) -> Result<(), Box<dyn Error>> {
    let settings = PerfSettings::from_env()?;

    for index in 1..settings.tab_count {
        let label = format!("perf-tab-{index}");
        WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
            .title(format!("Postmite perf tab {index}"))
            .inner_size(1200.0, 800.0)
            .visible(false)
            .build()?;
    }

    if let Some(path) = settings.ready_file {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(
            path,
            format!(
                "{{\"readyMs\":{},\"tabCount\":{}}}\n",
                ready_after.as_millis(),
                settings.tab_count
            ),
        )?;
    }

    Ok(())
}

impl PerfSettings {
    fn from_env() -> Result<Self, Box<dyn Error>> {
        let ready_file = env::var_os(PERF_READY_FILE_ENV).map(PathBuf::from);
        let tab_count = match env::var(PERF_TAB_COUNT_ENV) {
            Ok(value) => parse_tab_count(&value)?,
            Err(env::VarError::NotPresent) => DEFAULT_PERF_TAB_COUNT,
            Err(error) => return Err(Box::new(error)),
        };

        Ok(Self {
            ready_file,
            tab_count,
        })
    }
}

fn parse_tab_count(value: &str) -> Result<u8, Box<dyn Error>> {
    let tab_count = u8::from_str(value)?;
    if !(1..=MAX_PERF_TAB_COUNT).contains(&tab_count) {
        return Err(
            format!("{PERF_TAB_COUNT_ENV} must be between 1 and {MAX_PERF_TAB_COUNT}").into(),
        );
    }

    Ok(tab_count)
}

#[cfg(test)]
mod tests {
    use super::parse_tab_count;

    #[test]
    fn parses_valid_perf_tab_count() {
        assert_eq!(parse_tab_count("1").expect("one tab"), 1);
        assert_eq!(parse_tab_count("10").expect("ten tabs"), 10);
    }

    #[test]
    fn rejects_out_of_range_perf_tab_count() {
        assert!(parse_tab_count("0").is_err());
        assert!(parse_tab_count("11").is_err());
    }
}
