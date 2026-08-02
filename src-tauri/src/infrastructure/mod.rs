//! Native adapters for persistence, filesystem, secrets, and HTTP execution.

pub mod http;
pub mod oauth;
#[cfg(target_os = "linux")]
pub mod secrets;
pub mod sqlite;
