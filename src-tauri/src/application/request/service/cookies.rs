
fn validate_cookie_draft(draft: &CookieDraft) -> Result<(), RequestError> {
    if draft.name.trim().is_empty() {
        return Err(RequestError::InvalidInput(
            "cookie.name.required".to_owned(),
        ));
    }
    if draft.name.contains('=') || draft.name.contains(';') {
        return Err(RequestError::InvalidInput("cookie.name.invalid".to_owned()));
    }
    if draft.domain.trim().is_empty() {
        return Err(RequestError::InvalidInput(
            "cookie.domain.required".to_owned(),
        ));
    }
    if !draft.path.starts_with('/') {
        return Err(RequestError::InvalidInput("cookie.path.invalid".to_owned()));
    }
    Ok(())
}

fn cookie_matches_url(cookie: &WorkspaceCookie, url: &Url, now_epoch_seconds: i64) -> bool {
    if let Some(expires_at) = cookie.expires_at_epoch_seconds {
        if expires_at <= now_epoch_seconds {
            return false;
        }
    }
    if cookie.secure && url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let cookie_domain = cookie.domain.trim_start_matches('.').to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    if host != cookie_domain && !host.ends_with(&format!(".{cookie_domain}")) {
        return false;
    }
    url.path().starts_with(&cookie.path)
}

fn cookie_draft_from_set_cookie(
    workspace_id: WorkspaceId,
    url: &Url,
    value: &str,
) -> Result<Option<CookieDraft>, RequestError> {
    let parsed = Cookie::parse(value.to_owned())
        .map_err(|_| RequestError::InvalidInput("cookie.set_cookie.invalid".to_owned()))?;
    let Some(host) = url.host_str() else {
        return Ok(None);
    };
    let domain = parsed
        .domain()
        .map(str::to_owned)
        .unwrap_or_else(|| host.to_owned());
    let path = parsed
        .path()
        .map(str::to_owned)
        .unwrap_or_else(|| default_cookie_path(url.path()));
    let expires_at_epoch_seconds = match parsed.expires() {
        Some(Expiration::DateTime(datetime)) => Some(datetime.unix_timestamp()),
        _ => None,
    };
    let same_site = parsed.same_site().map(cookie_same_site_from_cookie);
    Ok(Some(CookieDraft {
        id: None,
        workspace_id,
        name: parsed.name().to_owned(),
        value: parsed.value().to_owned(),
        domain,
        path,
        secure: parsed.secure().unwrap_or(false),
        http_only: parsed.http_only().unwrap_or(false),
        same_site,
        expires_at_epoch_seconds,
    }))
}

fn default_cookie_path(url_path: &str) -> String {
    if !url_path.starts_with('/') || url_path == "/" {
        return "/".to_owned();
    }
    match url_path.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(index) => url_path[..index].to_owned(),
    }
}

fn cookie_same_site_from_cookie(value: SameSite) -> CookieSameSite {
    match value {
        SameSite::Strict => CookieSameSite::Strict,
        SameSite::Lax => CookieSameSite::Lax,
        SameSite::None => CookieSameSite::None,
    }
}

fn current_epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn secret_request_error(error: crate::application::secrets::SecretError) -> RequestError {
    match error {
        crate::application::secrets::SecretError::Locked => {
            RequestError::InvalidInput("secret.storage.locked".to_owned())
        }
        crate::application::secrets::SecretError::Unavailable => {
            RequestError::InvalidInput("secret.storage.unavailable".to_owned())
        }
        crate::application::secrets::SecretError::NotFound => {
            RequestError::InvalidInput("secret.reference.notFound".to_owned())
        }
        crate::application::secrets::SecretError::Storage(_) => {
            RequestError::Persistence("secret storage failed".to_owned())
        }
    }
}

fn cookie_scope_matches(cookie: &WorkspaceCookie, draft: &CookieDraft) -> bool {
    cookie.workspace_id == draft.workspace_id
        && cookie.name == draft.name.trim()
        && cookie.domain == normalize_cookie_domain_for_request(&draft.domain)
        && cookie.path == draft.path
}

fn cookie_secret_owner_name(draft: &CookieDraft) -> String {
    format!(
        "{}:{}:{}",
        normalize_cookie_domain_for_request(&draft.domain),
        draft.path,
        draft.name.trim()
    )
}

fn normalize_cookie_domain_for_request(domain: &str) -> String {
    domain.trim().trim_start_matches('.').to_ascii_lowercase()
}
