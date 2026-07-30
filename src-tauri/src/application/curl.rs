use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{
    request::{
        BodyFilePath, BodyFileReference, MultipartPart, OrderedField, ProxyPolicy, ProxySource,
        RequestAuth, RequestBody, RequestContent, TimeoutPolicy, TransportPolicy,
    },
    workspace::WorkspaceId,
};

use super::request::{
    RequestError, RequestRepository, RequestService, ResolvedRequestBody, ResolvedRequestContent,
    ResolvedValue, REDACTED_VALUE,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CurlImportInput {
    pub workspace_id: WorkspaceId,
    pub source_name: String,
    pub command: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CurlGenerateInput {
    pub content: RequestContent,
    pub resolved: Option<ResolvedRequestContent>,
    pub include_secrets: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CurlImportPreview {
    pub source_name: String,
    pub content: RequestContent,
    pub warning_count: u32,
    pub unsupported_count: u32,
    pub warnings: Vec<CurlImportWarning>,
    pub unsupported: Vec<CurlUnsupportedField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CurlImportWarning {
    pub location: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CurlUnsupportedField {
    pub location: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CurlGenerateResult {
    pub command: String,
    pub included_secret_count: u32,
    pub redacted_secret_count: u32,
}

#[derive(Debug, Error)]
pub enum CurlError {
    #[error("curl input is invalid: {0}")]
    InvalidInput(String),
    #[error("request failed: {0}")]
    Request(#[from] RequestError),
}

pub struct CurlService;

impl CurlService {
    pub fn preview(input: &CurlImportInput) -> Result<CurlImportPreview, CurlError> {
        convert_curl(input)
    }

    pub fn import_as_draft<R>(
        requests: &mut RequestService<R>,
        input: CurlImportInput,
    ) -> Result<super::request::RequestWorkspaceSnapshot, CurlError>
    where
        R: RequestRepository,
    {
        let preview = convert_curl(&input)?;
        requests
            .open_unsaved_tab_with_content(input.workspace_id, preview.content)
            .map_err(CurlError::Request)
    }

    pub fn generate(input: CurlGenerateInput) -> Result<CurlGenerateResult, CurlError> {
        generate_curl(input)
    }
}

fn convert_curl(input: &CurlImportInput) -> Result<CurlImportPreview, CurlError> {
    if input.command.trim().is_empty() {
        return Err(CurlError::InvalidInput("curl.command.empty".to_owned()));
    }

    let tokens = lex_shell_words(&input.command)?;
    let mut parser = CurlParser::new(tokens);
    let converted = parser.parse()?;
    Ok(CurlImportPreview {
        source_name: input.source_name.clone(),
        content: converted.content,
        warning_count: converted.warnings.len() as u32,
        unsupported_count: converted.unsupported.len() as u32,
        warnings: converted.warnings,
        unsupported: converted.unsupported,
    })
}

struct ConvertedCurl {
    content: RequestContent,
    warnings: Vec<CurlImportWarning>,
    unsupported: Vec<CurlUnsupportedField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ShellToken {
    value: String,
    start: usize,
}

struct CurlParser {
    tokens: Vec<ShellToken>,
    index: usize,
    method: Option<String>,
    url: Option<String>,
    headers: Vec<OrderedField>,
    body_fields: Vec<OrderedField>,
    raw_bodies: Vec<String>,
    multipart: Vec<MultipartPart>,
    auth: RequestAuth,
    redirect_enabled: Option<bool>,
    max_redirects: Option<u8>,
    tls_verify: bool,
    proxy: ProxyPolicy,
    timeouts: TimeoutPolicy,
    warnings: Vec<CurlImportWarning>,
    unsupported: Vec<CurlUnsupportedField>,
}

impl CurlParser {
    fn new(tokens: Vec<ShellToken>) -> Self {
        Self {
            tokens,
            index: 0,
            method: None,
            url: None,
            headers: Vec::new(),
            body_fields: Vec::new(),
            raw_bodies: Vec::new(),
            multipart: Vec::new(),
            auth: RequestAuth::None,
            redirect_enabled: None,
            max_redirects: None,
            tls_verify: true,
            proxy: ProxyPolicy::default(),
            timeouts: TimeoutPolicy::default(),
            warnings: Vec::new(),
            unsupported: Vec::new(),
        }
    }

    fn parse(&mut self) -> Result<ConvertedCurl, CurlError> {
        if self.tokens.first().map(|token| token.value.as_str()) != Some("curl") {
            return Err(CurlError::InvalidInput("curl.command.expected".to_owned()));
        }
        self.index = 1;

        while self.index < self.tokens.len() {
            let token = self.tokens[self.index].clone();
            if !token.value.starts_with('-') || token.value == "-" {
                self.set_url(token.value, token.start);
                self.index += 1;
                continue;
            }
            self.parse_option(token)?;
        }

        let body = if !self.multipart.is_empty() {
            RequestBody::Multipart {
                parts: self.multipart.clone(),
            }
        } else if !self.body_fields.is_empty() {
            RequestBody::UrlEncoded {
                fields: self.body_fields.clone(),
            }
        } else if !self.raw_bodies.is_empty() {
            RequestBody::Raw {
                content: self.raw_bodies.join("&"),
            }
        } else {
            RequestBody::None
        };
        let method = self.method.clone().unwrap_or_else(|| {
            if matches!(body, RequestBody::None) {
                "GET".to_owned()
            } else {
                "POST".to_owned()
            }
        });
        let url = self.url.clone().unwrap_or_default();
        if url.is_empty() {
            self.unsupported.push(CurlUnsupportedField {
                location: "$.argv".to_owned(),
                reason: "URL argument is required".to_owned(),
            });
        }
        let mut content = RequestContent::blank();
        content.name = "Imported cURL".to_owned();
        content.method = method.to_uppercase();
        content.url = url;
        content.headers = self.headers.clone();
        content.body = body;
        content.auth = self.auth.clone();
        content.redirect.enabled = self.redirect_enabled.unwrap_or(content.redirect.enabled);
        if let Some(max_redirects) = self.max_redirects {
            content.redirect.max_redirects = max_redirects;
        }
        content.tls.verify = self.tls_verify;
        content.transport = TransportPolicy {
            proxy: self.proxy.clone(),
            timeouts: self.timeouts.clone(),
        };

        Ok(ConvertedCurl {
            content,
            warnings: std::mem::take(&mut self.warnings),
            unsupported: std::mem::take(&mut self.unsupported),
        })
    }

    fn parse_option(&mut self, token: ShellToken) -> Result<(), CurlError> {
        let (name, attached) = split_attached_value(&token.value);
        match name {
            "-X" | "--request" => {
                let value = self.option_value(attached, &token)?;
                self.method = Some(value);
            }
            "--url" => {
                let value = self.option_value(attached, &token)?;
                self.set_url(value, token.start);
            }
            "-H" | "--header" => {
                let value = self.option_value(attached, &token)?;
                self.add_header(value, token.start);
            }
            "-b" | "--cookie" | "--cookie-jar" => {
                let value = self.option_value(attached, &token)?;
                if name == "--cookie-jar" {
                    self.warning(token.start, "cookie jar output path is not imported");
                } else {
                    self.push_header("Cookie", &value);
                }
            }
            "-u" | "--user" => {
                let value = self.option_value(attached, &token)?;
                let (username, password) = value.split_once(':').unwrap_or((value.as_str(), ""));
                self.auth = RequestAuth::Basic {
                    username: username.to_owned(),
                    password: password.to_owned(),
                };
            }
            "--oauth2-bearer" | "--bearer" => {
                let token_value = self.option_value(attached, &token)?;
                self.auth = RequestAuth::Bearer { token: token_value };
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" => {
                let value = self.option_value(attached, &token)?;
                self.raw_bodies.push(value);
            }
            "--data-urlencode" => {
                let value = self.option_value(attached, &token)?;
                self.push_ordered_body_field(value);
            }
            "-G" | "--get" => {
                self.method = Some("GET".to_owned());
                self.index += 1;
            }
            "-F" | "--form" | "--form-string" => {
                let value = self.option_value(attached, &token)?;
                self.push_multipart(value, name == "--form-string");
            }
            "-L" | "--location" => {
                self.redirect_enabled = Some(true);
                self.index += 1;
            }
            "--max-redirs" => {
                let value = self.option_value(attached, &token)?;
                match value.parse::<u8>() {
                    Ok(value) => self.max_redirects = Some(value),
                    Err(_) => self.unsupported(token.start, "max redirects must be 0-255"),
                }
            }
            "-k" | "--insecure" => {
                self.tls_verify = false;
                self.index += 1;
            }
            "--cacert" => {
                let value = self.option_value(attached, &token)?;
                self.warning(
                    token.start,
                    &format!("custom CA path recorded as a literal reference: {value}"),
                );
            }
            "--cert" | "--key" => {
                let _ = self.option_value(attached, &token)?;
                self.warning(
                    token.start,
                    "client certificate material requires relinking",
                );
            }
            "-x" | "--proxy" => {
                let value = self.option_value(attached, &token)?;
                self.proxy.source = ProxySource::Custom;
                self.proxy.url = Some(value);
            }
            "--noproxy" => {
                let value = self.option_value(attached, &token)?;
                self.proxy.no_proxy = value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            "--connect-timeout" => {
                let value = self.option_value(attached, &token)?;
                self.timeouts.connect_ms = parse_seconds_to_ms(&value).unwrap_or_else(|| {
                    self.unsupported(token.start, "connect timeout must be numeric seconds");
                    self.timeouts.connect_ms
                });
            }
            "--max-time" | "-m" => {
                let value = self.option_value(attached, &token)?;
                self.timeouts.overall_ms = parse_seconds_to_ms(&value).unwrap_or_else(|| {
                    self.unsupported(token.start, "overall timeout must be numeric seconds");
                    self.timeouts.overall_ms
                });
            }
            "--speed-time" => {
                let value = self.option_value(attached, &token)?;
                self.timeouts.idle_ms = parse_seconds_to_ms(&value).unwrap_or_else(|| {
                    self.unsupported(token.start, "idle timeout must be numeric seconds");
                    self.timeouts.idle_ms
                });
            }
            "-A" | "--user-agent" => {
                let value = self.option_value(attached, &token)?;
                self.push_header("User-Agent", &value);
            }
            "-I" | "--head" => {
                self.method = Some("HEAD".to_owned());
                self.index += 1;
            }
            "--compressed" | "-s" | "--silent" | "-S" | "--show-error" | "-i" | "--include" => {
                self.index += 1;
            }
            _ => {
                self.unsupported(token.start, &format!("unsupported cURL option `{name}`"));
                self.index += 1;
            }
        }
        Ok(())
    }

    fn option_value(
        &mut self,
        attached: Option<String>,
        token: &ShellToken,
    ) -> Result<String, CurlError> {
        if let Some(value) = attached {
            self.index += 1;
            return Ok(value);
        }
        let Some(next) = self.tokens.get(self.index + 1) else {
            return Err(CurlError::InvalidInput(format!(
                "curl.option.valueMissing@{}",
                token.start
            )));
        };
        self.index += 2;
        Ok(next.value.clone())
    }

    fn set_url(&mut self, value: String, start: usize) {
        if self.url.is_some() {
            self.unsupported(start, "multiple URL arguments are not supported");
        } else {
            self.url = Some(value);
        }
    }

    fn add_header(&mut self, value: String, start: usize) {
        let Some((name, header_value)) = value.split_once(':') else {
            self.unsupported(start, "header must use `Name: value` syntax");
            return;
        };
        let name = name.trim();
        let header_value = header_value.trim_start();
        if name.eq_ignore_ascii_case("authorization") {
            if let Some(token) = header_value.strip_prefix("Bearer ") {
                self.auth = RequestAuth::Bearer {
                    token: token.to_owned(),
                };
                return;
            }
        }
        self.push_header(name, header_value);
    }

    fn push_header(&mut self, name: &str, value: &str) {
        self.headers.push(OrderedField {
            enabled: true,
            order: self.headers.len() as u32,
            name: name.to_owned(),
            value: value.to_owned(),
        });
    }

    fn push_ordered_body_field(&mut self, value: String) {
        let (name, field_value) = value.split_once('=').unwrap_or((value.as_str(), ""));
        self.body_fields.push(OrderedField {
            enabled: true,
            order: self.body_fields.len() as u32,
            name: name.to_owned(),
            value: field_value.to_owned(),
        });
    }

    fn push_multipart(&mut self, value: String, force_field: bool) {
        let (name, part_value) = value.split_once('=').unwrap_or((value.as_str(), ""));
        let order = self.multipart.len() as u32;
        if !force_field {
            if let Some(path) = part_value.strip_prefix('@') {
                self.multipart.push(MultipartPart::File {
                    enabled: true,
                    order,
                    name: name.to_owned(),
                    file: file_reference(path),
                });
                return;
            }
        }
        self.multipart.push(MultipartPart::Field {
            enabled: true,
            order,
            name: name.to_owned(),
            value: part_value.to_owned(),
        });
    }

    fn warning(&mut self, start: usize, message: &str) {
        self.warnings.push(CurlImportWarning {
            location: format!("$.argv@{start}"),
            message: message.to_owned(),
        });
    }

    fn unsupported(&mut self, start: usize, reason: &str) {
        self.unsupported.push(CurlUnsupportedField {
            location: format!("$.argv@{start}"),
            reason: reason.to_owned(),
        });
    }
}

fn lex_shell_words(command: &str) -> Result<Vec<ShellToken>, CurlError> {
    let chars: Vec<(usize, char)> = command.char_indices().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut start = 0usize;
    let mut quote: Option<char> = None;
    let mut token_open = false;
    let mut index = 0usize;

    while index < chars.len() {
        let (byte, ch) = chars[index];
        if quote.is_none() && matches!(ch, '|' | '>' | '<' | ';') {
            return Err(CurlError::InvalidInput(format!(
                "curl.shellSyntax.rejected@{byte}"
            )));
        }
        if quote.is_none() && ch == '&' && chars.get(index + 1).map(|(_, c)| *c) == Some('&') {
            return Err(CurlError::InvalidInput(format!(
                "curl.shellSyntax.rejected@{byte}"
            )));
        }
        if quote != Some('\'') && ch == '`' {
            return Err(CurlError::InvalidInput(format!(
                "curl.commandSubstitution.rejected@{byte}"
            )));
        }
        if quote != Some('\'') && ch == '$' && chars.get(index + 1).map(|(_, c)| *c) == Some('(') {
            return Err(CurlError::InvalidInput(format!(
                "curl.commandSubstitution.rejected@{byte}"
            )));
        }

        match (quote, ch) {
            (None, ' ' | '\t' | '\n' | '\r') => {
                if token_open {
                    tokens.push(ShellToken {
                        value: std::mem::take(&mut current),
                        start,
                    });
                    token_open = false;
                }
            }
            (None, '\'' | '"') => {
                quote = Some(ch);
                if !token_open {
                    token_open = true;
                    start = byte;
                }
            }
            (Some(active), ch) if ch == active => {
                quote = None;
            }
            (Some('"'), '\\') | (None, '\\') => {
                if !token_open {
                    token_open = true;
                    start = byte;
                }
                if let Some((_, next)) = chars.get(index + 1) {
                    current.push(*next);
                    index += 1;
                }
            }
            _ => {
                if !token_open {
                    token_open = true;
                    start = byte;
                }
                current.push(ch);
            }
        }
        index += 1;
    }

    if let Some(active) = quote {
        return Err(CurlError::InvalidInput(format!(
            "curl.quote.unclosed.{active}"
        )));
    }
    if token_open {
        tokens.push(ShellToken {
            value: current,
            start,
        });
    }
    Ok(tokens)
}

fn split_attached_value(option: &str) -> (&str, Option<String>) {
    if let Some((name, value)) = option.split_once('=') {
        return (name, Some(value.to_owned()));
    }
    for short in ["-X", "-H", "-b", "-u", "-d", "-F", "-A", "-m"] {
        if option.starts_with(short) && option.len() > short.len() {
            return (short, Some(option[short.len()..].to_owned()));
        }
    }
    (option, None)
}

fn parse_seconds_to_ms(value: &str) -> Option<u64> {
    let seconds = value.parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some((seconds * 1000.0).round() as u64)
}

fn file_reference(path: &str) -> BodyFileReference {
    BodyFileReference {
        path: if path.starts_with('/') {
            BodyFilePath::Absolute {
                path: path.to_owned(),
            }
        } else {
            BodyFilePath::Relative {
                path: path.to_owned(),
            }
        },
        file_name: path
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("upload.bin")
            .to_owned(),
        size: 0,
        modified_at_epoch_seconds: None,
        sha256: String::new(),
    }
}

fn generate_curl(input: CurlGenerateInput) -> Result<CurlGenerateResult, CurlError> {
    if input.content.url.trim().is_empty() {
        return Err(CurlError::InvalidInput("curl.url.required".to_owned()));
    }
    let mut builder = CurlCommandBuilder {
        args: vec!["curl".to_owned()],
        included_secret_count: 0,
        redacted_secret_count: 0,
        include_secrets: input.include_secrets,
    };
    let resolved = input.resolved.as_ref();
    builder.push_pair("-X", &input.content.method);
    let url = value_for(
        &input.content.url,
        resolved.map(|resolved| &resolved.url),
        &mut builder,
    );
    builder.push_value(url);

    for (index, header) in input.content.headers.iter().enumerate() {
        if !header.enabled {
            continue;
        }
        let value = resolved
            .and_then(|resolved| resolved.headers.get(index))
            .map(|resolved| {
                format!(
                    "{}: {}",
                    value_for(&header.name, Some(&resolved.name), &mut builder),
                    value_for(&header.value, Some(&resolved.value), &mut builder)
                )
            })
            .unwrap_or_else(|| format!("{}: {}", header.name, header.value));
        builder.push_pair("-H", &value);
    }

    match &input.content.auth {
        RequestAuth::None => {}
        RequestAuth::Basic { username, password } => {
            let password = builder.secret_value(password);
            builder.push_pair("-u", &format!("{username}:{password}"));
        }
        RequestAuth::Bearer { token } => {
            let token = builder.secret_value(token);
            builder.push_pair("-H", &format!("Authorization: Bearer {token}"));
        }
        RequestAuth::ApiKey {
            placement: _,
            name,
            value,
        } => {
            let value = builder.secret_value(value);
            builder.push_pair("-H", &format!("{name}: {value}"));
        }
        RequestAuth::ClientCredentials { .. } => {
            builder.push_pair("-H", "Authorization: Bearer ********");
            builder.redacted_secret_count += 1;
        }
    }

    if !input.content.redirect.enabled {
        builder.push_pair("--max-redirs", "0");
    } else if input.content.redirect.max_redirects != 10 {
        builder.push("--location");
        builder.push_pair(
            "--max-redirs",
            &input.content.redirect.max_redirects.to_string(),
        );
    } else {
        builder.push("--location");
    }
    if !input.content.tls.verify {
        builder.push("--insecure");
    }
    if input.content.transport.proxy.source == ProxySource::Custom {
        if let Some(proxy) = &input.content.transport.proxy.url {
            builder.push_pair("--proxy", proxy);
        }
    }
    if !input.content.transport.proxy.no_proxy.is_empty() {
        builder.push_pair(
            "--noproxy",
            &input.content.transport.proxy.no_proxy.join(","),
        );
    }
    if input.content.transport.timeouts.connect_ms != TimeoutPolicy::default().connect_ms {
        builder.push_pair(
            "--connect-timeout",
            &ms_to_seconds(input.content.transport.timeouts.connect_ms),
        );
    }
    if input.content.transport.timeouts.overall_ms != TimeoutPolicy::default().overall_ms {
        builder.push_pair(
            "--max-time",
            &ms_to_seconds(input.content.transport.timeouts.overall_ms),
        );
    }

    match &input.content.body {
        RequestBody::None => {}
        RequestBody::Raw { content } => {
            let body = value_for(
                content,
                resolved.and_then(|resolved| match &resolved.body_kind {
                    ResolvedRequestBody::Raw { content } => Some(content),
                    _ => None,
                }),
                &mut builder,
            );
            builder.push_pair("--data-raw", &body);
        }
        RequestBody::UrlEncoded { fields } => {
            for (index, field) in fields.iter().enumerate() {
                if !field.enabled {
                    continue;
                }
                let value = resolved
                    .and_then(|resolved| match &resolved.body_kind {
                        ResolvedRequestBody::UrlEncoded { fields } => fields.get(index),
                        _ => None,
                    })
                    .map(|resolved| {
                        format!(
                            "{}={}",
                            value_for(&field.name, Some(&resolved.name), &mut builder),
                            value_for(&field.value, Some(&resolved.value), &mut builder)
                        )
                    })
                    .unwrap_or_else(|| format!("{}={}", field.name, field.value));
                builder.push_pair("--data-urlencode", &value);
            }
        }
        RequestBody::Multipart { parts } => {
            for (index, part) in parts.iter().enumerate() {
                match part {
                    MultipartPart::Field {
                        enabled,
                        name,
                        value,
                        ..
                    } if *enabled => {
                        let resolved_part =
                            resolved.and_then(|resolved| match &resolved.body_kind {
                                ResolvedRequestBody::Multipart { parts } => parts.get(index),
                                _ => None,
                            });
                        let generated = match resolved_part {
                            Some(super::request::ResolvedMultipartPart::Field {
                                name: rn,
                                value: rv,
                                ..
                            }) => format!(
                                "{}={}",
                                value_for(name, Some(rn), &mut builder),
                                value_for(value, Some(rv), &mut builder)
                            ),
                            _ => format!("{name}={value}"),
                        };
                        builder.push_pair("-F", &generated);
                    }
                    MultipartPart::File {
                        enabled,
                        name,
                        file,
                        ..
                    } if *enabled => {
                        builder.push_pair("-F", &format!("{name}=@{}", body_file_path(file)));
                    }
                    _ => {}
                }
            }
        }
        RequestBody::Binary { file } => {
            builder.push_pair("--data-binary", &format!("@{}", body_file_path(file)));
        }
    }

    Ok(builder.finish())
}

struct CurlCommandBuilder {
    args: Vec<String>,
    included_secret_count: u32,
    redacted_secret_count: u32,
    include_secrets: bool,
}

impl CurlCommandBuilder {
    fn push(&mut self, value: &str) {
        self.args.push(value.to_owned());
    }

    fn push_value(&mut self, value: String) {
        self.args.push(value);
    }

    fn push_pair(&mut self, name: &str, value: &str) {
        self.args.push(name.to_owned());
        self.args.push(value.to_owned());
    }

    fn secret_value(&mut self, value: &str) -> String {
        if self.include_secrets {
            self.included_secret_count += 1;
            value.to_owned()
        } else {
            self.redacted_secret_count += 1;
            REDACTED_VALUE.to_owned()
        }
    }

    fn finish(self) -> CurlGenerateResult {
        CurlGenerateResult {
            command: self
                .args
                .iter()
                .map(|arg| shell_quote(arg))
                .collect::<Vec<_>>()
                .join(" "),
            included_secret_count: self.included_secret_count,
            redacted_secret_count: self.redacted_secret_count,
        }
    }
}

fn value_for(
    literal: &str,
    resolved: Option<&ResolvedValue>,
    builder: &mut CurlCommandBuilder,
) -> String {
    match resolved {
        Some(value) if value.contains_secret && builder.include_secrets => {
            builder.included_secret_count += 1;
            value.value.clone()
        }
        Some(value) if value.contains_secret => {
            builder.redacted_secret_count += 1;
            REDACTED_VALUE.to_owned()
        }
        Some(value) => value.value.clone(),
        None => literal.to_owned(),
    }
}

fn ms_to_seconds(ms: u64) -> String {
    if ms.is_multiple_of(1000) {
        (ms / 1000).to_string()
    } else {
        format!("{:.3}", ms as f64 / 1000.0)
    }
}

fn body_file_path(file: &BodyFileReference) -> String {
    match &file.path {
        BodyFilePath::Relative { path } | BodyFilePath::Absolute { path } => path.clone(),
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_owned();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-._/:{}?=&,%+".contains(ch))
    {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::request::{materialize_request_content_for_curl, RequestWorkspaceSnapshot},
        domain::request::{
            Environment, EnvironmentId, EnvironmentVariable, RequestAuth, RequestBody, Variable,
            VariableValue,
        },
    };

    fn input(command: &str) -> CurlImportInput {
        CurlImportInput {
            workspace_id: WorkspaceId::new(),
            source_name: "fixture".to_owned(),
            command: command.to_owned(),
        }
    }

    fn secret_resolution_fixture() -> (RequestWorkspaceSnapshot, EnvironmentId, RequestContent) {
        let workspace_id = WorkspaceId::new();
        let environment_id = EnvironmentId::new();
        (
            RequestWorkspaceSnapshot {
                workspace_id,
                collection_folders: Vec::new(),
                environments: vec![Environment {
                    id: environment_id,
                    workspace_id,
                    name: "Fixture".to_owned(),
                    position: 0,
                    is_selected: true,
                }],
                collection_variables: Vec::new(),
                environment_variables: vec![
                    EnvironmentVariable {
                        environment_id,
                        workspace_id,
                        variable: Variable {
                            name: "baseUrl".to_owned(),
                            value: VariableValue::Plain("https://resolved.example.test".to_owned()),
                        },
                    },
                    EnvironmentVariable {
                        environment_id,
                        workspace_id,
                        variable: Variable {
                            name: "credential".to_owned(),
                            value: VariableValue::SecretReference(
                                "secret://curl-fixture".to_owned(),
                            ),
                        },
                    },
                ],
                saved_requests: Vec::new(),
                drafts: Vec::new(),
                tabs: Vec::new(),
            },
            environment_id,
            RequestContent {
                url: "{{baseUrl}}/items".to_owned(),
                auth: RequestAuth::Bearer {
                    token: "{{credential}}".to_owned(),
                },
                ..RequestContent::blank()
            },
        )
    }

    #[test]
    fn supported_fixture_becomes_equivalent_request_draft_content() {
        let preview = CurlService::preview(&input(
            "curl -X PUT 'https://api.example.test/users?limit=10' \
             -H 'Accept: application/json' -H 'Authorization: Bearer {{token}}' \
             --data-raw '{\"ok\":true}' --location --max-redirs 3 \
             --proxy http://proxy.example:8080 --connect-timeout 2.5",
        ))
        .expect("preview");

        assert_eq!(preview.content.method, "PUT");
        assert_eq!(
            preview.content.url,
            "https://api.example.test/users?limit=10"
        );
        assert_eq!(preview.content.headers[0].name, "Accept");
        assert!(matches!(
            preview.content.auth,
            RequestAuth::Bearer { ref token } if token == "{{token}}"
        ));
        assert!(matches!(
            preview.content.body,
            RequestBody::Raw { ref content } if content == "{\"ok\":true}"
        ));
        assert_eq!(preview.content.redirect.max_redirects, 3);
        assert_eq!(preview.content.transport.timeouts.connect_ms, 2500);
        assert_eq!(preview.unsupported_count, 0);
    }

    #[test]
    fn form_and_file_fixture_preserves_ordered_parts() {
        let preview = CurlService::preview(&input(
            "curl https://upload.example.test -F title=demo -F file=@fixtures/payload.json",
        ))
        .expect("preview");

        match preview.content.body {
            RequestBody::Multipart { parts } => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(parts[0], MultipartPart::Field { order: 0, .. }));
                assert!(matches!(parts[1], MultipartPart::File { order: 1, .. }));
            }
            body => panic!("unexpected body: {body:?}"),
        }
    }

    #[test]
    fn unsupported_options_identify_location_and_reason() {
        let preview = CurlService::preview(&input("curl --ftp-method nocwd https://example.test"))
            .expect("preview");

        assert!(preview.unsupported_count >= 1);
        assert!(preview.unsupported.iter().any(|field| {
            field.location.starts_with("$.argv@") && field.reason.contains("--ftp-method")
        }));
    }

    #[test]
    fn malicious_shell_fixtures_never_parse_as_arguments() {
        for command in [
            "curl https://example.test | sh",
            "curl https://example.test > /tmp/out",
            "curl https://example.test $(touch /tmp/postmite)",
            "curl https://example.test `touch /tmp/postmite`",
        ] {
            assert!(matches!(
                CurlService::preview(&input(command)),
                Err(CurlError::InvalidInput(_))
            ));
        }
    }

    #[test]
    fn generated_default_output_redacts_resolved_secrets() {
        let mut content = RequestContent::blank();
        content.method = "POST".to_owned();
        content.url = "https://api.example.test/{{id}}".to_owned();
        content.body = RequestBody::Raw {
            content: "{{fixtureSecret}}".to_owned(),
        };
        let resolved = ResolvedRequestContent {
            url: ResolvedValue {
                value: "https://api.example.test/42".to_owned(),
                contains_secret: false,
            },
            body: ResolvedValue {
                value: "fixture-secret".to_owned(),
                contains_secret: true,
            },
            body_kind: ResolvedRequestBody::Raw {
                content: ResolvedValue {
                    value: "fixture-secret".to_owned(),
                    contains_secret: true,
                },
            },
            query: Vec::new(),
            headers: Vec::new(),
            unsafe_tls_visible: false,
            references: Vec::new(),
            errors: Vec::new(),
        };

        let generated = CurlService::generate(CurlGenerateInput {
            content: content.clone(),
            resolved: Some(resolved.clone()),
            include_secrets: false,
        })
        .expect("generate");
        assert!(!generated.command.contains("fixture-secret"));
        assert!(generated.command.contains(REDACTED_VALUE));
        assert_eq!(generated.redacted_secret_count, 1);

        let included = CurlService::generate(CurlGenerateInput {
            content,
            resolved: Some(resolved),
            include_secrets: true,
        })
        .expect("generate with secrets");
        assert!(included.command.contains("fixture-secret"));
        assert_eq!(included.included_secret_count, 1);
    }

    #[test]
    fn generated_output_excludes_disabled_fields_and_preserves_duplicate_order() {
        let enabled_field = |order, value: &str| OrderedField {
            enabled: true,
            order,
            name: "X-Mode".to_owned(),
            value: value.to_owned(),
        };
        let mut content = RequestContent::blank();
        content.url = "https://api.example.test/items?tag=first&tag=second".to_owned();
        content.query = vec![
            OrderedField {
                enabled: true,
                order: 0,
                name: "tag".to_owned(),
                value: "first".to_owned(),
            },
            OrderedField {
                enabled: false,
                order: 1,
                name: "tag".to_owned(),
                value: "off".to_owned(),
            },
            OrderedField {
                enabled: true,
                order: 2,
                name: "tag".to_owned(),
                value: "second".to_owned(),
            },
        ];
        content.headers = vec![
            enabled_field(0, "first"),
            OrderedField {
                enabled: false,
                order: 1,
                name: "X-Mode".to_owned(),
                value: "off".to_owned(),
            },
            enabled_field(2, "second"),
        ];

        let generated = CurlService::generate(CurlGenerateInput {
            content,
            resolved: None,
            include_secrets: false,
        })
        .expect("generate ordered cURL");

        assert!(generated
            .command
            .contains("https://api.example.test/items?tag=first&tag=second"));
        assert!(!generated.command.contains("tag=off"));
        assert!(!generated.command.contains("X-Mode: off"));
        let first = generated
            .command
            .find("X-Mode: first")
            .expect("first header");
        let second = generated
            .command
            .find("X-Mode: second")
            .expect("second header");
        assert!(first < second);
    }

    #[test]
    fn confirmed_generation_materializes_secrets_in_rust() {
        let (snapshot, environment_id, content) = secret_resolution_fixture();
        let runtime_secret = format!("runtime-{}", uuid::Uuid::new_v4());
        let materialized = materialize_request_content_for_curl(
            &snapshot,
            Some(environment_id),
            content,
            &|reference| (reference == "secret://curl-fixture").then(|| runtime_secret.clone()),
        )
        .expect("materialize confirmed cURL");

        let generated = CurlService::generate(CurlGenerateInput {
            content: materialized,
            resolved: None,
            include_secrets: true,
        })
        .expect("generate confirmed cURL");

        assert!(generated
            .command
            .contains("https://resolved.example.test/items"));
        assert!(generated.command.contains(&runtime_secret));
        assert!(!generated.command.contains("{{credential}}"));
    }

    #[test]
    fn confirmed_generation_rejects_missing_secret_without_exposing_reference() {
        let (snapshot, environment_id, content) = secret_resolution_fixture();
        let error =
            materialize_request_content_for_curl(&snapshot, Some(environment_id), content, &|_| {
                None
            })
            .expect_err("missing Secret must fail");
        let message = error.to_string();

        assert!(message.contains("curl.secret.unavailable"));
        assert!(!message.contains("secret://"));
    }

    #[test]
    fn confirmed_generation_rejects_environment_mismatch_before_secret_resolution() {
        let (snapshot, _environment_id, content) = secret_resolution_fixture();
        let resolver_called = std::cell::Cell::new(false);
        let error = materialize_request_content_for_curl(
            &snapshot,
            Some(EnvironmentId::new()),
            content,
            &|_| {
                resolver_called.set(true);
                None
            },
        )
        .expect_err("stale Environment must fail");

        assert!(error.to_string().contains("curl.resolution.stale"));
        assert!(!resolver_called.get());
    }
}
