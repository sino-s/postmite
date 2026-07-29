use serde::Deserialize;
use thiserror::Error;

pub const DEFAULT_UPDATE_CHECK_URL: &str =
    "https://api.github.com/repos/sino-s/postmite/releases/latest";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateCheckResult {
    pub latest_version: String,
    pub update_available: bool,
}

#[derive(Debug, Error)]
pub enum UpdateCheckError {
    #[error("update request failed")]
    Request,
    #[error("update response was invalid")]
    InvalidResponse,
}

#[derive(Deserialize)]
struct ReleaseResponse {
    tag_name: String,
}

pub async fn check_for_update(
    endpoint: &str,
    current_version: &str,
) -> Result<UpdateCheckResult, UpdateCheckError> {
    let response = reqwest::Client::new()
        .get(endpoint)
        .header(reqwest::header::USER_AGENT, "Postmite update check")
        .send()
        .await
        .map_err(|_| UpdateCheckError::Request)?
        .error_for_status()
        .map_err(|_| UpdateCheckError::Request)?
        .text()
        .await
        .map_err(|_| UpdateCheckError::InvalidResponse)?;
    let response: ReleaseResponse =
        serde_json::from_str(&response).map_err(|_| UpdateCheckError::InvalidResponse)?;
    let latest_version = response.tag_name.trim_start_matches('v').to_owned();
    if latest_version.is_empty() {
        return Err(UpdateCheckError::InvalidResponse);
    }
    Ok(UpdateCheckResult {
        update_available: latest_version != current_version,
        latest_version,
    })
}

#[cfg(test)]
mod tests {
    use super::check_for_update;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::{timeout, Duration},
    };

    #[tokio::test]
    async fn sends_no_network_request_before_the_manual_check_then_requests_once() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let endpoint = format!("http://{}/latest", listener.local_addr().expect("address"));
        assert!(timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err());
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("manual connection");
            let mut request = [0; 1024];
            let read = stream.read(&mut request).await.expect("request read");
            assert!(std::str::from_utf8(&request[..read])
                .expect("request text")
                .starts_with("GET /latest HTTP/1.1"));
            stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 21\r\nconnection: close\r\n\r\n{\"tag_name\":\"v0.2.0\"}").await.expect("response");
        });
        let result = check_for_update(&endpoint, "0.1.0")
            .await
            .expect("manual result");
        assert_eq!(result.latest_version, "0.2.0");
        assert!(result.update_available);
        server.await.expect("server");
    }
}
