use std::{collections::HashMap, time::Duration};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;
use url::{form_urlencoded, Url};

use crate::application::oauth::OAuthError;

const MAX_CALLBACK_REQUEST_BYTES: usize = 8 * 1024;
const CALLBACK_RESPONSE_BODY: &str = "Authorization received. Return to Postmite.";

pub struct LoopbackCallbackListener {
    listener: TcpListener,
    redirect_uri: String,
    path: String,
}

impl LoopbackCallbackListener {
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub async fn wait(
        self,
        cancellation: CancellationToken,
        timeout: Duration,
    ) -> Result<LoopbackCallback, OAuthError> {
        let callback = async {
            let (mut stream, _) = self
                .listener
                .accept()
                .await
                .map_err(|_| OAuthError::ListenerFailed)?;
            let mut buffer = vec![0_u8; MAX_CALLBACK_REQUEST_BYTES];
            let read = stream
                .read(&mut buffer)
                .await
                .map_err(|_| OAuthError::ListenerFailed)?;
            let request = String::from_utf8_lossy(&buffer[..read]);
            let callback = parse_callback_request(&request, &self.path)?;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                CALLBACK_RESPONSE_BODY.len(),
                CALLBACK_RESPONSE_BODY
            );
            stream
                .write_all(response.as_bytes())
                .await
                .map_err(|_| OAuthError::ListenerFailed)?;
            Ok(callback)
        };

        tokio::select! {
            _ = cancellation.cancelled() => Err(OAuthError::Cancelled),
            result = tokio::time::timeout(timeout, callback) => match result {
                Ok(result) => result,
                Err(_) => Err(OAuthError::Timeout),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopbackCallback {
    query: HashMap<String, String>,
}

impl LoopbackCallback {
    pub fn query_value(&self, name: &str) -> Option<&str> {
        self.query.get(name).map(String::as_str)
    }
}

pub async fn listen_for_oauth_callback(path: &str) -> Result<LoopbackCallbackListener, OAuthError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| OAuthError::ListenerFailed)?;
    let address = listener
        .local_addr()
        .map_err(|_| OAuthError::ListenerFailed)?;
    if !address.ip().is_loopback() || address.ip().to_string() != "127.0.0.1" {
        return Err(OAuthError::ListenerFailed);
    }
    Ok(LoopbackCallbackListener {
        listener,
        redirect_uri: format!("http://127.0.0.1:{}{}", address.port(), path),
        path: path.to_owned(),
    })
}

fn parse_callback_request(
    request: &str,
    expected_path: &str,
) -> Result<LoopbackCallback, OAuthError> {
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or(OAuthError::ListenerFailed)?;
    let url =
        Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| OAuthError::ListenerFailed)?;
    if url.path() != expected_path {
        return Err(OAuthError::ListenerFailed);
    }
    let query = form_urlencoded::parse(url.query().unwrap_or_default().as_bytes())
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect();
    Ok(LoopbackCallback { query })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn oauth_loopback_listener_binds_only_127_0_0_1() {
        let listener = listen_for_oauth_callback("/oauth/callback")
            .await
            .expect("listener");
        let url = Url::parse(listener.redirect_uri()).expect("redirect uri");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert!(url.port().is_some());
    }

    #[tokio::test]
    async fn oauth_loopback_listener_is_removed_after_callback() {
        let listener = listen_for_oauth_callback("/oauth/callback")
            .await
            .expect("listener");
        let url = Url::parse(listener.redirect_uri()).expect("redirect uri");
        let port = url.port().expect("port");
        let redirect_uri = listener.redirect_uri().to_owned();
        let task = tokio::spawn(async move {
            listener
                .wait(CancellationToken::new(), Duration::from_secs(1))
                .await
                .expect("callback")
        });

        reqwest::get(format!(
            "{redirect_uri}?code=fixture-code&state=fixture-state"
        ))
        .await
        .expect("send callback");
        let callback = task.await.expect("join");
        assert_eq!(callback.query_value("code"), Some("fixture-code"));
        assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
    }
}
