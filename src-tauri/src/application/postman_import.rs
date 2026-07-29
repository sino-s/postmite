use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{
    request::{
        ApiKeyPlacement, BodyFilePath, CollectionId, EnvironmentId, MultipartPart, OrderedField,
        RequestAuth, RequestBody, RequestContent, SavedRequest, TimeoutPolicy, TransportPolicy,
        VariableValue,
    },
    workspace::WorkspaceId,
};

use super::{
    request::RequestWorkspaceSnapshot,
    secrets::{SecretClass, SecretOwner, SecretStore},
};

pub const POSTMAN_IMPORT_MAX_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanImportInput {
    pub workspace_id: WorkspaceId,
    pub source_name: String,
    pub collection_json: String,
    pub environment_json: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanExportInput {
    pub workspace_id: WorkspaceId,
    pub source_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanExportResult {
    pub collection_json: String,
    pub environments: Vec<PostmanEnvironmentExport>,
    pub warning_count: u32,
    pub unsupported_count: u32,
    pub warnings: Vec<PostmanImportWarning>,
    pub unsupported: Vec<PostmanUnsupportedField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanEnvironmentExport {
    pub name: String,
    pub environment_json: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanImportPreview {
    pub source_id: String,
    pub source_name: String,
    pub source_hash: String,
    pub collection_count: u32,
    pub request_count: u32,
    pub environment_count: u32,
    pub warning_count: u32,
    pub unsupported_count: u32,
    pub warnings: Vec<PostmanImportWarning>,
    pub unsupported: Vec<PostmanUnsupportedField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanImportWarning {
    pub location: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanUnsupportedField {
    pub location: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanImportResult {
    pub preview: PostmanImportPreview,
    pub snapshot: RequestWorkspaceSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanReimportPreview {
    pub import_preview: PostmanImportPreview,
    pub prior_import: Option<PostmanPriorImport>,
    pub changes: Vec<PostmanReimportChange>,
    pub can_update: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanPriorImport {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub source_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanReimportChange {
    pub location: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PostmanReimportDecision {
    Update,
    Duplicate,
    Cancel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanReimportInput {
    pub import: PostmanImportInput,
    pub decision: PostmanReimportDecision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostmanReimportResult {
    pub preview: PostmanReimportPreview,
    pub snapshot: RequestWorkspaceSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvertedPostmanImport {
    pub workspace_id: WorkspaceId,
    pub source_id: String,
    pub source_name: String,
    pub source_hash: String,
    pub collection_json_sha256: String,
    pub environment_json_sha256: Option<String>,
    pub warnings: Vec<PostmanImportWarning>,
    pub unsupported: Vec<PostmanUnsupportedField>,
    pub collections: Vec<ConvertedCollection>,
    pub requests: Vec<ConvertedSavedRequest>,
    pub environments: Vec<ConvertedEnvironment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvertedCollection {
    pub import_index: usize,
    pub parent_import_index: Option<usize>,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvertedSavedRequest {
    pub collection_import_index: Option<usize>,
    pub content: RequestContent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvertedEnvironment {
    pub name: String,
    pub variables: Vec<ConvertedVariable>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvertedVariable {
    pub name: String,
    pub value: VariableValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredPostmanImportRecord {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub source_hash: String,
    pub collection_ids: Vec<CollectionId>,
    pub environment_ids: Vec<EnvironmentId>,
}

pub trait PostmanImportRepository {
    fn list_postman_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RequestWorkspaceSnapshot, PostmanImportError>;
    fn find_latest_postman_import(
        &self,
        workspace_id: WorkspaceId,
        source_id: &str,
    ) -> Result<Option<StoredPostmanImportRecord>, PostmanImportError>;
    fn import_postman(
        &mut self,
        import: ConvertedPostmanImport,
    ) -> Result<RequestWorkspaceSnapshot, PostmanImportError>;
    fn update_postman_import(
        &mut self,
        prior: &StoredPostmanImportRecord,
        import: ConvertedPostmanImport,
    ) -> Result<RequestWorkspaceSnapshot, PostmanImportError>;
}

pub struct PostmanImportService<R>
where
    R: PostmanImportRepository,
{
    repository: R,
    secrets: Arc<dyn SecretStore>,
}

impl<R> PostmanImportService<R>
where
    R: PostmanImportRepository,
{
    pub fn new(repository: R, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            repository,
            secrets,
        }
    }

    pub fn preview(
        &self,
        input: &PostmanImportInput,
    ) -> Result<PostmanImportPreview, PostmanImportError> {
        let converted = convert_postman(input, None)?;
        Ok(converted.preview())
    }

    pub fn export(
        &self,
        input: &PostmanExportInput,
    ) -> Result<PostmanExportResult, PostmanImportError> {
        let snapshot = self.repository.list_postman_workspace(input.workspace_id)?;
        export_postman(input, &snapshot)
    }

    pub fn import(
        &mut self,
        input: PostmanImportInput,
    ) -> Result<PostmanImportResult, PostmanImportError> {
        let converted = convert_postman(&input, Some(&self.secrets))?;
        let preview = converted.preview();
        let secret_references = converted.secret_references();
        let snapshot = match self.repository.import_postman(converted) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                for reference in secret_references {
                    let _ = self.secrets.delete(&reference);
                }
                return Err(error);
            }
        };
        Ok(PostmanImportResult { preview, snapshot })
    }

    pub fn preview_reimport(
        &self,
        input: &PostmanImportInput,
    ) -> Result<PostmanReimportPreview, PostmanImportError> {
        let converted = convert_postman(input, None)?;
        let import_preview = converted.preview();
        let prior = self
            .repository
            .find_latest_postman_import(input.workspace_id, &converted.source_id)?;
        Ok(reimport_preview(import_preview, prior))
    }

    pub fn reimport(
        &mut self,
        input: PostmanReimportInput,
    ) -> Result<PostmanReimportResult, PostmanImportError> {
        let preview = self.preview_reimport(&input.import)?;
        if input.decision == PostmanReimportDecision::Cancel {
            let snapshot = self
                .repository
                .list_postman_workspace(input.import.workspace_id)?;
            return Ok(PostmanReimportResult { preview, snapshot });
        }

        let converted = convert_postman(&input.import, Some(&self.secrets))?;
        let secret_references = converted.secret_references();
        let snapshot = match input.decision {
            PostmanReimportDecision::Duplicate => self.repository.import_postman(converted),
            PostmanReimportDecision::Update => {
                let Some(prior) = preview.prior_import.as_ref() else {
                    return Err(PostmanImportError::InvalidInput(
                        "postman.reimport.prior.required".to_owned(),
                    ));
                };
                let stored = self
                    .repository
                    .find_latest_postman_import(input.import.workspace_id, &prior.source_id)?
                    .ok_or_else(|| {
                        PostmanImportError::InvalidInput(
                            "postman.reimport.prior.required".to_owned(),
                        )
                    })?;
                if stored.collection_ids.is_empty() && stored.environment_ids.is_empty() {
                    return Err(PostmanImportError::InvalidInput(
                        "postman.reimport.prior.entities.required".to_owned(),
                    ));
                }
                self.repository.update_postman_import(&stored, converted)
            }
            PostmanReimportDecision::Cancel => unreachable!(),
        };
        match snapshot {
            Ok(snapshot) => Ok(PostmanReimportResult { preview, snapshot }),
            Err(error) => {
                for reference in secret_references {
                    let _ = self.secrets.delete(&reference);
                }
                Err(error)
            }
        }
    }
}

fn reimport_preview(
    import_preview: PostmanImportPreview,
    prior: Option<StoredPostmanImportRecord>,
) -> PostmanReimportPreview {
    let prior_import = prior.as_ref().map(|record| PostmanPriorImport {
        id: record.id.clone(),
        source_id: record.source_id.clone(),
        source_name: record.source_name.clone(),
        source_hash: record.source_hash.clone(),
    });
    let mut changes = Vec::new();
    if let Some(record) = &prior {
        if record.source_hash == import_preview.source_hash {
            changes.push(PostmanReimportChange {
                location: "$.source".to_owned(),
                message: "incoming source matches the previous import".to_owned(),
            });
        } else {
            changes.push(PostmanReimportChange {
                location: "$.source".to_owned(),
                message: "incoming source differs from the previous import".to_owned(),
            });
        }
        changes.push(PostmanReimportChange {
            location: "$.workspace".to_owned(),
            message: format!(
                "update would replace {} imported collection root(s) and {} environment(s)",
                record.collection_ids.len(),
                record.environment_ids.len()
            ),
        });
    } else {
        changes.push(PostmanReimportChange {
            location: "$.source".to_owned(),
            message: "no earlier Import Record was found; duplicate import is available".to_owned(),
        });
    }
    let can_update = prior
        .as_ref()
        .map(|record| !record.collection_ids.is_empty() || !record.environment_ids.is_empty())
        .unwrap_or(false);
    PostmanReimportPreview {
        import_preview,
        prior_import,
        changes,
        can_update,
    }
}

impl ConvertedPostmanImport {
    fn preview(&self) -> PostmanImportPreview {
        PostmanImportPreview {
            source_id: self.source_id.clone(),
            source_name: self.source_name.clone(),
            source_hash: self.source_hash.clone(),
            collection_count: self.collections.len() as u32,
            request_count: self.requests.len() as u32,
            environment_count: self.environments.len() as u32,
            warning_count: self.warnings.len() as u32,
            unsupported_count: self.unsupported.len() as u32,
            warnings: self.warnings.clone(),
            unsupported: self.unsupported.clone(),
        }
    }

    fn secret_references(&self) -> Vec<String> {
        self.environments
            .iter()
            .flat_map(|environment| environment.variables.iter())
            .filter_map(|variable| match &variable.value {
                VariableValue::SecretReference(reference) => Some(reference.clone()),
                VariableValue::Plain(_) => None,
            })
            .collect()
    }
}

#[derive(Debug, Error)]
pub enum PostmanImportError {
    #[error("postman import input is invalid: {0}")]
    InvalidInput(String),
    #[error("workspace was not found")]
    WorkspaceNotFound,
    #[error("postman import persistence failed: {0}")]
    Persistence(String),
    #[error("secret storage failed: {0}")]
    Secret(String),
}

impl PostmanImportError {
    pub fn persistence(error: impl std::error::Error) -> Self {
        Self::Persistence(error.to_string())
    }
}

fn convert_postman(
    input: &PostmanImportInput,
    secrets: Option<&Arc<dyn SecretStore>>,
) -> Result<ConvertedPostmanImport, PostmanImportError> {
    validate_size("collection", &input.collection_json)?;
    if let Some(environment_json) = &input.environment_json {
        validate_size("environment", environment_json)?;
    }

    let mut context = ConvertContext::default();
    let collection_root: Value = serde_json::from_str(&input.collection_json)
        .map_err(|_| PostmanImportError::InvalidInput("postman.collection.malformed".to_owned()))?;
    let collection_json_sha256 = sha256_hex(input.collection_json.as_bytes());
    let environment_json_sha256 = input
        .environment_json
        .as_ref()
        .map(|json| sha256_hex(json.as_bytes()));
    let source_hash = sha256_hex(
        format!(
            "{}\n{}",
            collection_json_sha256,
            environment_json_sha256.as_deref().unwrap_or("")
        )
        .as_bytes(),
    );

    let info = collection_root
        .get("info")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            PostmanImportError::InvalidInput("postman.collection.info.required".to_owned())
        })?;
    let schema = info.get("schema").and_then(Value::as_str).unwrap_or("");
    if !schema.contains("v2.1") {
        return Err(PostmanImportError::InvalidInput(
            "postman.collection.schema.v2_1.required".to_owned(),
        ));
    }
    let collection_name = info
        .get("name")
        .and_then(Value::as_str)
        .map(clean_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| clean_name(&input.source_name));
    let source_id = info
        .get("_postman_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|| source_hash.clone());

    note_unsupported_keys(
        &mut context,
        "$.collection",
        &collection_root,
        &["info", "item", "auth", "event", "variable"],
    );
    note_unsupported_keys(
        &mut context,
        "$.collection.info",
        collection_root.get("info").unwrap_or(&Value::Null),
        &["name", "_postman_id", "schema", "description"],
    );
    note_if_present(
        &mut context,
        "$.collection.event",
        &collection_root,
        "event",
        "collection scripts are out of scope",
    );
    note_if_present(
        &mut context,
        "$.collection.variable",
        &collection_root,
        "variable",
        "collection variables are out of scope for Postman import",
    );

    let mut collections = Vec::new();
    let mut requests = Vec::new();
    let root_index = collections.len();
    collections.push(ConvertedCollection {
        import_index: root_index,
        parent_import_index: None,
        name: collection_name,
    });
    let items = collection_root
        .get("item")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            PostmanImportError::InvalidInput("postman.collection.item.required".to_owned())
        })?;
    convert_items(
        items,
        Some(root_index),
        "$.collection.item",
        &mut collections,
        &mut requests,
        &mut context,
    )?;

    let mut environments = Vec::new();
    if let Some(environment_json) = &input.environment_json {
        let environment: Value = serde_json::from_str(environment_json).map_err(|_| {
            PostmanImportError::InvalidInput("postman.environment.malformed".to_owned())
        })?;
        environments.push(convert_environment(
            input.workspace_id,
            &input.source_name,
            &environment,
            secrets,
            &mut context,
        )?);
    }

    Ok(ConvertedPostmanImport {
        workspace_id: input.workspace_id,
        source_id,
        source_name: clean_name(&input.source_name),
        source_hash,
        collection_json_sha256,
        environment_json_sha256,
        warnings: context.warnings,
        unsupported: context.unsupported,
        collections,
        requests,
        environments,
    })
}

fn export_postman(
    input: &PostmanExportInput,
    snapshot: &RequestWorkspaceSnapshot,
) -> Result<PostmanExportResult, PostmanImportError> {
    if input.workspace_id != snapshot.workspace_id {
        return Err(PostmanImportError::WorkspaceNotFound);
    }
    let mut context = ConvertContext::default();
    let collection_items = export_child_items(snapshot, None, &mut context);
    let collection = serde_json::json!({
        "info": {
            "name": clean_name(&input.source_name),
            "_postman_id": sha256_hex(format!("postmite:{}:{}", snapshot.workspace_id, input.source_name).as_bytes()),
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": collection_items
    });
    let collection_json = serde_json::to_string_pretty(&collection)
        .map_err(|error| PostmanImportError::Persistence(error.to_string()))?;
    let environments = snapshot
        .environments
        .iter()
        .map(|environment| {
            let values = snapshot
                .environment_variables
                .iter()
                .filter(|variable| variable.environment_id == environment.id)
                .map(|variable| match &variable.variable.value {
                    VariableValue::Plain(value) => serde_json::json!({
                        "key": variable.variable.name,
                        "value": value,
                        "enabled": true,
                        "type": "text"
                    }),
                    VariableValue::SecretReference(_) => {
                        context.warnings.push(PostmanImportWarning {
                            location: format!("$.environment.{}.{}", environment.name, variable.variable.name),
                            message: "Secret value was omitted from Postman export".to_owned(),
                        });
                        serde_json::json!({
                            "key": variable.variable.name,
                            "value": "",
                            "enabled": true,
                            "type": "secret"
                        })
                    }
                })
                .collect::<Vec<_>>();
            let document = serde_json::json!({
                "id": sha256_hex(format!("postmite:{}:{}", snapshot.workspace_id, environment.id).as_bytes()),
                "name": environment.name,
                "values": values,
                "_postman_variable_scope": "environment"
            });
            let environment_json = serde_json::to_string_pretty(&document)
                .map_err(|error| PostmanImportError::Persistence(error.to_string()))?;
            Ok(PostmanEnvironmentExport {
                name: environment.name.clone(),
                environment_json,
            })
        })
        .collect::<Result<Vec<_>, PostmanImportError>>()?;
    Ok(PostmanExportResult {
        collection_json,
        environments,
        warning_count: context.warnings.len() as u32,
        unsupported_count: context.unsupported.len() as u32,
        warnings: context.warnings,
        unsupported: context.unsupported,
    })
}

fn export_child_items(
    snapshot: &RequestWorkspaceSnapshot,
    parent_id: Option<CollectionId>,
    context: &mut ConvertContext,
) -> Vec<Value> {
    let mut items = Vec::new();
    let mut folders = snapshot
        .collection_folders
        .iter()
        .filter(|folder| folder.parent_collection_id == parent_id)
        .collect::<Vec<_>>();
    folders.sort_by_key(|folder| (folder.position, folder.name.clone(), folder.id.to_string()));
    for folder in folders {
        items.push(serde_json::json!({
            "name": folder.name,
            "item": export_child_items(snapshot, Some(folder.id), context)
        }));
    }
    let mut requests = snapshot
        .saved_requests
        .iter()
        .filter(|request| request.collection_id == parent_id)
        .collect::<Vec<_>>();
    requests.sort_by_key(|request| {
        (
            request.position,
            request.content.name.clone(),
            request.id.to_string(),
        )
    });
    for request in requests {
        items.push(export_request_item(request, context));
    }
    items
}

fn export_request_item(request: &SavedRequest, context: &mut ConvertContext) -> Value {
    serde_json::json!({
        "name": request.content.name,
        "request": {
            "method": request.content.method,
            "url": export_url(&request.content),
            "header": export_fields(&request.content.headers),
            "body": export_body(&request.content.body, &request.content.name, context),
            "auth": export_auth(&request.content.auth, &request.content.name, context)
        }
    })
}

fn export_url(content: &RequestContent) -> Value {
    let query = export_fields(&content.query);
    if query.as_array().map(Vec::is_empty).unwrap_or(true) {
        Value::String(content.url.clone())
    } else {
        serde_json::json!({
            "raw": content.url,
            "query": query
        })
    }
}

fn export_fields(fields: &[OrderedField]) -> Value {
    let mut rows = fields.iter().collect::<Vec<_>>();
    rows.sort_by_key(|field| field.order);
    Value::Array(
        rows.into_iter()
            .map(|field| {
                serde_json::json!({
                    "key": field.name,
                    "value": field.value,
                    "disabled": !field.enabled
                })
            })
            .collect(),
    )
}

fn export_body(body: &RequestBody, request_name: &str, context: &mut ConvertContext) -> Value {
    match body {
        RequestBody::None => Value::Null,
        RequestBody::Raw { content } => serde_json::json!({
            "mode": "raw",
            "raw": content
        }),
        RequestBody::UrlEncoded { fields } => serde_json::json!({
            "mode": "urlencoded",
            "urlencoded": export_fields(fields)
        }),
        RequestBody::Multipart { parts } => {
            let rows = parts
                .iter()
                .filter_map(|part| match part {
                    MultipartPart::Field {
                        enabled,
                        order: _,
                        name,
                        value,
                    } => Some(serde_json::json!({
                        "key": name,
                        "value": value,
                        "type": "text",
                        "disabled": !enabled
                    })),
                    MultipartPart::File { file, .. } => {
                        match file.path {
                            BodyFilePath::Relative { .. } => context.unsupported.push(
                                PostmanUnsupportedField {
                                    location: format!("$.collection.item[{request_name}].request.body.formdata"),
                                    reason: "multipart file references are diagnostic-only in Postman export".to_owned(),
                                },
                            ),
                            BodyFilePath::Absolute { .. } => context.unsupported.push(
                                PostmanUnsupportedField {
                                    location: format!("$.collection.item[{request_name}].request.body.formdata"),
                                    reason: "absolute Body-file paths are excluded from Postman export".to_owned(),
                                },
                            ),
                        }
                        None
                    }
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "mode": "formdata",
                "formdata": rows
            })
        }
        RequestBody::Binary { file } => {
            match file.path {
                BodyFilePath::Relative { .. } => {
                    context.unsupported.push(PostmanUnsupportedField {
                        location: format!("$.collection.item[{request_name}].request.body"),
                        reason: "binary Body-file references are diagnostic-only in Postman export"
                            .to_owned(),
                    })
                }
                BodyFilePath::Absolute { .. } => {
                    context.unsupported.push(PostmanUnsupportedField {
                        location: format!("$.collection.item[{request_name}].request.body"),
                        reason: "absolute Body-file paths are excluded from Postman export"
                            .to_owned(),
                    })
                }
            }
            Value::Null
        }
    }
}

fn export_auth(auth: &RequestAuth, request_name: &str, context: &mut ConvertContext) -> Value {
    match auth {
        RequestAuth::None => serde_json::json!({"type": "noauth"}),
        RequestAuth::Basic { username, password } => serde_json::json!({
            "type": "basic",
            "basic": [
                {"key": "username", "value": username, "type": "string"},
                {"key": "password", "value": password, "type": "string"}
            ]
        }),
        RequestAuth::Bearer { token } => serde_json::json!({
            "type": "bearer",
            "bearer": [{"key": "token", "value": token, "type": "string"}]
        }),
        RequestAuth::ApiKey {
            placement,
            name,
            value,
        } => serde_json::json!({
            "type": "apikey",
            "apikey": [
                {"key": "key", "value": name, "type": "string"},
                {"key": "value", "value": value, "type": "string"},
                {"key": "in", "value": match placement {
                    ApiKeyPlacement::Header => "header",
                    ApiKeyPlacement::Query => "query",
                }, "type": "string"}
            ]
        }),
        RequestAuth::ClientCredentials { .. } => {
            context.unsupported.push(PostmanUnsupportedField {
                location: format!("$.collection.item[{request_name}].request.auth"),
                reason: "OAuth client credentials auth is diagnostic-only in Postman export"
                    .to_owned(),
            });
            serde_json::json!({"type": "noauth"})
        }
    }
}

fn convert_items(
    items: &[Value],
    parent_import_index: Option<usize>,
    path: &str,
    collections: &mut Vec<ConvertedCollection>,
    requests: &mut Vec<ConvertedSavedRequest>,
    context: &mut ConvertContext,
) -> Result<(), PostmanImportError> {
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}[{index}]");
        note_unsupported_keys(
            context,
            &item_path,
            item,
            &[
                "name",
                "item",
                "request",
                "event",
                "protocolProfileBehavior",
                "description",
            ],
        );
        note_if_present(
            context,
            &format!("{item_path}.event"),
            item,
            "event",
            "item scripts are out of scope",
        );
        if let Some(children) = item.get("item").and_then(Value::as_array) {
            let folder_index = collections.len();
            collections.push(ConvertedCollection {
                import_index: folder_index,
                parent_import_index,
                name: item_name(item, "Collection"),
            });
            convert_items(
                children,
                Some(folder_index),
                &format!("{item_path}.item"),
                collections,
                requests,
                context,
            )?;
            continue;
        }
        if let Some(request) = item.get("request") {
            requests.push(ConvertedSavedRequest {
                collection_import_index: parent_import_index,
                content: convert_request(item, request, &item_path, context)?,
            });
        } else {
            context.warnings.push(PostmanImportWarning {
                location: item_path,
                message: "item skipped because it has neither request nor child items".to_owned(),
            });
        }
    }
    Ok(())
}

fn convert_request(
    item: &Value,
    request: &Value,
    item_path: &str,
    context: &mut ConvertContext,
) -> Result<RequestContent, PostmanImportError> {
    let request_path = format!("{item_path}.request");
    if let Some(url) = request.as_str() {
        return Ok(RequestContent {
            name: item_name(item, "Request"),
            method: "GET".to_owned(),
            url: url.to_owned(),
            ..RequestContent::blank()
        });
    }
    note_unsupported_keys(
        context,
        &request_path,
        request,
        &["method", "header", "body", "url", "auth", "description"],
    );
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("GET")
        .to_uppercase();
    let (url, query_from_url) =
        convert_url(request.get("url"), &format!("{request_path}.url"), context);
    let mut query = query_from_url;
    query.sort_by_key(|field| field.order);
    Ok(RequestContent {
        name: item_name(item, "Request"),
        method,
        url,
        body: convert_body(
            request.get("body"),
            &format!("{request_path}.body"),
            context,
        ),
        query,
        headers: convert_headers(
            request.get("header"),
            &format!("{request_path}.header"),
            context,
        ),
        auth: convert_auth(
            request.get("auth"),
            &format!("{request_path}.auth"),
            context,
        ),
        redirect: Default::default(),
        tls: Default::default(),
        transport: TransportPolicy {
            timeouts: TimeoutPolicy::default(),
            ..TransportPolicy::default()
        },
    })
}

fn convert_url(
    url: Option<&Value>,
    path: &str,
    context: &mut ConvertContext,
) -> (String, Vec<OrderedField>) {
    match url {
        Some(Value::String(value)) => (value.clone(), Vec::new()),
        Some(Value::Object(_)) => {
            let value = url.unwrap();
            note_unsupported_keys(
                context,
                path,
                value,
                &["raw", "protocol", "host", "path", "query", "port", "hash"],
            );
            let raw = value.get("raw").and_then(Value::as_str).map(str::to_owned);
            let fallback = || {
                let protocol = value
                    .get("protocol")
                    .and_then(Value::as_str)
                    .unwrap_or("https");
                let host = string_array(value.get("host")).join(".");
                let path_segments = string_array(value.get("path")).join("/");
                if host.is_empty() {
                    String::new()
                } else if path_segments.is_empty() {
                    format!("{protocol}://{host}")
                } else {
                    format!("{protocol}://{host}/{path_segments}")
                }
            };
            let query = value
                .get("query")
                .and_then(Value::as_array)
                .map(|rows| convert_key_value_rows(rows, &format!("{path}.query"), context))
                .unwrap_or_default();
            (raw.unwrap_or_else(fallback), query)
        }
        _ => {
            context.warnings.push(PostmanImportWarning {
                location: path.to_owned(),
                message: "request URL is missing or unsupported".to_owned(),
            });
            (String::new(), Vec::new())
        }
    }
}

fn convert_headers(
    value: Option<&Value>,
    path: &str,
    context: &mut ConvertContext,
) -> Vec<OrderedField> {
    value
        .and_then(Value::as_array)
        .map(|rows| convert_key_value_rows(rows, path, context))
        .unwrap_or_default()
}

fn convert_key_value_rows(
    rows: &[Value],
    path: &str,
    context: &mut ConvertContext,
) -> Vec<OrderedField> {
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let row_path = format!("{path}[{index}]");
            note_unsupported_keys(
                context,
                &row_path,
                row,
                &["key", "value", "disabled", "description", "type"],
            );
            let name = row
                .get("key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            if name.is_empty() {
                context.warnings.push(PostmanImportWarning {
                    location: row_path,
                    message: "field skipped because key is empty".to_owned(),
                });
                return None;
            }
            Some(OrderedField {
                enabled: !row
                    .get("disabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                order: index as u32,
                name,
                value: value_to_string(row.get("value")),
            })
        })
        .collect()
}

fn convert_body(value: Option<&Value>, path: &str, context: &mut ConvertContext) -> RequestBody {
    let Some(body) = value else {
        return RequestBody::None;
    };
    note_unsupported_keys(
        context,
        path,
        body,
        &[
            "mode",
            "raw",
            "urlencoded",
            "formdata",
            "file",
            "graphql",
            "options",
            "disabled",
        ],
    );
    match body.get("mode").and_then(Value::as_str).unwrap_or("") {
        "raw" => RequestBody::Raw {
            content: value_to_string(body.get("raw")),
        },
        "urlencoded" => RequestBody::UrlEncoded {
            fields: body
                .get("urlencoded")
                .and_then(Value::as_array)
                .map(|rows| convert_key_value_rows(rows, &format!("{path}.urlencoded"), context))
                .unwrap_or_default(),
        },
        "formdata" => {
            let parts = body
                .get("formdata")
                .and_then(Value::as_array)
                .map(|rows| {
                    rows.iter()
                        .enumerate()
                        .filter_map(|(index, row)| {
                            let row_path = format!("{path}.formdata[{index}]");
                            note_unsupported_keys(
                                context,
                                &row_path,
                                row,
                                &["key", "value", "type", "disabled", "description"],
                            );
                            if row.get("type").and_then(Value::as_str) == Some("file") {
                                context.unsupported.push(PostmanUnsupportedField {
                                    location: row_path,
                                    reason: "multipart file references are not imported from Postman collections".to_owned(),
                                });
                                return None;
                            }
                            Some(crate::domain::request::MultipartPart::Field {
                                enabled: !row.get("disabled").and_then(Value::as_bool).unwrap_or(false),
                                order: index as u32,
                                name: row.get("key").and_then(Value::as_str).unwrap_or("").to_owned(),
                                value: value_to_string(row.get("value")),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            RequestBody::Multipart { parts }
        }
        "file" | "graphql" => {
            context.unsupported.push(PostmanUnsupportedField {
                location: path.to_owned(),
                reason: "body mode is out of scope for this import".to_owned(),
            });
            RequestBody::None
        }
        "" => RequestBody::None,
        other => {
            context.unsupported.push(PostmanUnsupportedField {
                location: path.to_owned(),
                reason: format!("body mode {other} is not supported"),
            });
            RequestBody::None
        }
    }
}

fn convert_auth(value: Option<&Value>, path: &str, context: &mut ConvertContext) -> RequestAuth {
    let Some(auth) = value else {
        return RequestAuth::None;
    };
    note_unsupported_keys(
        context,
        path,
        auth,
        &["type", "basic", "bearer", "apikey", "oauth2", "noauth"],
    );
    match auth.get("type").and_then(Value::as_str).unwrap_or("noauth") {
        "noauth" => RequestAuth::None,
        "basic" => RequestAuth::Basic {
            username: auth_array_value(auth, "basic", "username"),
            password: auth_array_value(auth, "basic", "password"),
        },
        "bearer" => RequestAuth::Bearer {
            token: auth_array_value(auth, "bearer", "token"),
        },
        "apikey" => RequestAuth::ApiKey {
            placement: match auth_array_value(auth, "apikey", "in").as_str() {
                "query" => ApiKeyPlacement::Query,
                _ => ApiKeyPlacement::Header,
            },
            name: auth_array_value(auth, "apikey", "key"),
            value: auth_array_value(auth, "apikey", "value"),
        },
        other => {
            context.unsupported.push(PostmanUnsupportedField {
                location: path.to_owned(),
                reason: format!("auth type {other} is not supported"),
            });
            RequestAuth::None
        }
    }
}

fn convert_environment(
    workspace_id: WorkspaceId,
    source_name: &str,
    root: &Value,
    secrets: Option<&Arc<dyn SecretStore>>,
    context: &mut ConvertContext,
) -> Result<ConvertedEnvironment, PostmanImportError> {
    note_unsupported_keys(
        context,
        "$.environment",
        root,
        &[
            "id",
            "name",
            "values",
            "_postman_variable_scope",
            "_postman_exported_at",
            "_postman_exported_using",
        ],
    );
    let name = root
        .get("name")
        .and_then(Value::as_str)
        .map(clean_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| clean_name(source_name));
    let values = root
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            PostmanImportError::InvalidInput("postman.environment.values.required".to_owned())
        })?;
    let mut variables = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let path = format!("$.environment.values[{index}]");
        note_unsupported_keys(
            context,
            &path,
            value,
            &["key", "value", "enabled", "type", "secret"],
        );
        if value.get("enabled").and_then(Value::as_bool) == Some(false) {
            context.warnings.push(PostmanImportWarning {
                location: path,
                message: "disabled environment variable skipped".to_owned(),
            });
            continue;
        }
        let key = value
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if key.is_empty() {
            context.warnings.push(PostmanImportWarning {
                location: path,
                message: "environment variable skipped because key is empty".to_owned(),
            });
            continue;
        }
        let raw_value = value_to_string(value.get("value"));
        let is_secret = value
            .get("secret")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || value.get("type").and_then(Value::as_str) == Some("secret");
        let variable_value = if is_secret {
            match secrets {
                Some(secrets) => {
                    let write = secrets
                        .put(
                            &SecretOwner::new(
                                workspace_id,
                                SecretClass::ProtectedVariable,
                                format!("postman:{name}:{key}"),
                            ),
                            &raw_value,
                        )
                        .map_err(|error| PostmanImportError::Secret(secret_error_detail(error)))?;
                    VariableValue::SecretReference(write.reference)
                }
                None => {
                    VariableValue::SecretReference("secret://postmite/import-preview".to_owned())
                }
            }
        } else {
            VariableValue::Plain(raw_value)
        };
        variables.push(ConvertedVariable {
            name: key.to_owned(),
            value: variable_value,
        });
    }
    Ok(ConvertedEnvironment { name, variables })
}

fn auth_array_value(auth: &Value, array_name: &str, key: &str) -> String {
    auth.get(array_name)
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("key").and_then(Value::as_str) == Some(key))
        })
        .and_then(|row| row.get("value"))
        .map(|value| value_to_string(Some(value)))
        .unwrap_or_default()
}

fn item_name(item: &Value, fallback: &str) -> String {
    item.get("name")
        .and_then(Value::as_str)
        .map(clean_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn clean_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "Imported Postman Data".to_owned()
    } else {
        trimmed.chars().take(256).collect()
    }
}

fn value_to_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => value.to_string(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| value_to_string(Some(value)))
            .collect(),
        Some(Value::String(value)) => vec![value.clone()],
        _ => Vec::new(),
    }
}

fn note_if_present(
    context: &mut ConvertContext,
    location: &str,
    value: &Value,
    key: &str,
    reason: &str,
) {
    if value.get(key).is_some() {
        context.unsupported.push(PostmanUnsupportedField {
            location: location.to_owned(),
            reason: reason.to_owned(),
        });
    }
}

fn note_unsupported_keys(
    context: &mut ConvertContext,
    location: &str,
    value: &Value,
    supported: &[&str],
) {
    if let Some(object) = value.as_object() {
        for key in object.keys() {
            if !supported.contains(&key.as_str()) {
                context.unsupported.push(PostmanUnsupportedField {
                    location: format!("{location}.{key}"),
                    reason: "field is not supported by Postmite import".to_owned(),
                });
            }
        }
    }
}

fn validate_size(kind: &str, json: &str) -> Result<(), PostmanImportError> {
    if json.len() > POSTMAN_IMPORT_MAX_BYTES {
        return Err(PostmanImportError::InvalidInput(format!(
            "postman.{kind}.oversized"
        )));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn secret_error_detail(error: super::secrets::SecretError) -> String {
    match error {
        super::secrets::SecretError::Locked => "secret.storage.locked".to_owned(),
        super::secrets::SecretError::Unavailable => "secret.storage.unavailable".to_owned(),
        super::secrets::SecretError::NotFound => "secret.reference.notFound".to_owned(),
        super::secrets::SecretError::Storage(_) => "secret.storage.failed".to_owned(),
    }
}

#[derive(Default)]
struct ConvertContext {
    warnings: Vec<PostmanImportWarning>,
    unsupported: Vec<PostmanUnsupportedField>,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use super::*;
    use crate::application::secrets::{
        SecretError, SecretPersistence, SecretStore, SecretWrite, SessionSecretStore,
    };
    use crate::domain::request::{
        BodyFilePath, BodyFileReference, CollectionFolder, CollectionId, Environment,
        EnvironmentId, EnvironmentVariable, SavedRequest, SavedRequestId, Variable,
    };

    fn input(collection: &str, environment: Option<&str>) -> PostmanImportInput {
        PostmanImportInput {
            workspace_id: WorkspaceId::new(),
            source_name: "Fixture".to_owned(),
            collection_json: collection.to_owned(),
            environment_json: environment.map(str::to_owned),
        }
    }

    #[test]
    fn supported_collection_and_environment_convert_to_internal_model() {
        let collection = r#"{
          "info": {"name": "Demo", "_postman_id": "postman-demo", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
          "item": [{
            "name": "Users",
            "item": [{
              "name": "List users",
              "request": {
                "method": "POST",
                "url": {"raw": "https://api.example.test/users?limit=10", "query": [{"key": "limit", "value": "10"}]},
                "header": [{"key": "Accept", "value": "application/json"}],
                "body": {"mode": "raw", "raw": "{\"ok\":true}"},
                "auth": {"type": "bearer", "bearer": [{"key": "token", "value": "{{token}}"}]}
              }
            }]
          }]
        }"#;
        let environment = r#"{
          "name": "Production",
          "values": [
            {"key": "baseUrl", "value": "https://api.example.test", "enabled": true, "type": "text"},
            {"key": "token", "value": "protected-value-marker", "enabled": true, "type": "secret"}
          ]
        }"#;
        let secrets: Arc<dyn SecretStore> = Arc::new(SessionSecretStore::new());
        let converted = convert_postman(&input(collection, Some(environment)), Some(&secrets))
            .expect("convert");

        assert_eq!(converted.collections.len(), 2);
        assert_eq!(converted.requests.len(), 1);
        assert_eq!(converted.environments.len(), 1);
        assert_eq!(converted.requests[0].content.method, "POST");
        assert_eq!(converted.requests[0].content.query[0].name, "limit");
        assert!(matches!(
            converted.requests[0].content.body,
            RequestBody::Raw { .. }
        ));
        assert!(matches!(
            converted.environments[0].variables[1].value,
            VariableValue::SecretReference(ref reference) if secrets.contains(reference)
        ));
    }

    #[test]
    fn unsupported_fields_are_reported_with_locations() {
        let collection = r#"{
          "info": {"name": "Demo", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
          "event": [],
          "item": [{
            "name": "GraphQL",
            "request": {"method": "POST", "url": "https://example.test", "body": {"mode": "graphql", "graphql": {"query": "{}"}}}
          }]
        }"#;
        let converted = convert_postman(&input(collection, None), None).expect("convert");

        assert!(converted
            .unsupported
            .iter()
            .any(|field| field.location == "$.collection.event"));
        assert!(converted
            .unsupported
            .iter()
            .any(|field| field.location.contains("request.body")));
    }

    #[test]
    fn malformed_and_oversized_inputs_are_rejected() {
        assert!(matches!(
            convert_postman(&input("{", None), None),
            Err(PostmanImportError::InvalidInput(detail)) if detail == "postman.collection.malformed"
        ));
        let mut oversized = r#"{"info":{"schema":"v2.1"},"item":[]}"#.to_owned();
        oversized.push_str(&" ".repeat(POSTMAN_IMPORT_MAX_BYTES));
        assert!(matches!(
            convert_postman(&input(&oversized, None), None),
            Err(PostmanImportError::InvalidInput(detail)) if detail == "postman.collection.oversized"
        ));
    }

    #[test]
    fn failed_persistence_removes_secrets_written_during_conversion() {
        let collection = r#"{
          "info": {"name": "Demo", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
          "item": []
        }"#;
        let environment = r#"{
          "name": "Production",
          "values": [
            {"key": "token", "value": "protected-value-marker", "enabled": true, "type": "secret"}
          ]
        }"#;
        let secrets = Arc::new(RecordingSecretStore::default());
        let secret_view = Arc::clone(&secrets);
        let service_secrets: Arc<dyn SecretStore> = secrets;
        let mut service = PostmanImportService::new(FailingRepository, service_secrets);

        let result = service.import(input(collection, Some(environment)));

        assert!(matches!(result, Err(PostmanImportError::Persistence(_))));
        assert_eq!(secret_view.values.lock().expect("secrets").len(), 0);
    }

    #[test]
    fn export_generates_postman_v21_json_without_secret_or_absolute_file_leaks() {
        let workspace_id = WorkspaceId::new();
        let collection_id = CollectionId::new();
        let environment_id = EnvironmentId::new();
        let absolute_path = "/tmp/private/payload.bin";
        let snapshot = RequestWorkspaceSnapshot {
            workspace_id,
            collection_folders: vec![CollectionFolder {
                id: collection_id,
                workspace_id,
                parent_collection_id: None,
                name: "Imported".to_owned(),
                position: 0,
            }],
            environments: vec![Environment {
                id: environment_id,
                workspace_id,
                name: "Production".to_owned(),
                position: 0,
                is_selected: true,
            }],
            collection_variables: Vec::new(),
            environment_variables: vec![EnvironmentVariable {
                environment_id,
                workspace_id,
                variable: Variable {
                    name: "token".to_owned(),
                    value: VariableValue::SecretReference("secret://token".to_owned()),
                },
            }],
            saved_requests: vec![SavedRequest {
                id: SavedRequestId::new(),
                workspace_id,
                collection_id: Some(collection_id),
                position: 0,
                content: RequestContent {
                    name: "Upload".to_owned(),
                    method: "POST".to_owned(),
                    url: "https://example.test/upload".to_owned(),
                    body: RequestBody::Binary {
                        file: BodyFileReference {
                            path: BodyFilePath::Absolute {
                                path: absolute_path.to_owned(),
                            },
                            file_name: "payload.bin".to_owned(),
                            size: 1,
                            modified_at_epoch_seconds: None,
                            sha256: "payload-hash".to_owned(),
                        },
                    },
                    ..RequestContent::blank()
                },
            }],
            drafts: Vec::new(),
            tabs: Vec::new(),
        };

        let result = export_postman(
            &PostmanExportInput {
                workspace_id,
                source_name: "Fixture".to_owned(),
            },
            &snapshot,
        )
        .expect("export");
        let collection: Value =
            serde_json::from_str(&result.collection_json).expect("collection json");
        let environment: Value =
            serde_json::from_str(&result.environments[0].environment_json).expect("environment");

        assert_eq!(
            collection["info"]["schema"],
            "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        );
        assert!(!result.collection_json.contains(absolute_path));
        assert_eq!(environment["values"][0]["value"], "");
        assert_eq!(environment["values"][0]["type"], "secret");
        assert!(result
            .unsupported
            .iter()
            .any(|field| field.reason.contains("absolute Body-file paths")));
    }

    #[test]
    fn exported_postman_json_imports_back_to_supported_internal_model() {
        let workspace_id = WorkspaceId::new();
        let collection_id = CollectionId::new();
        let environment_id = EnvironmentId::new();
        let snapshot = RequestWorkspaceSnapshot {
            workspace_id,
            collection_folders: vec![CollectionFolder {
                id: collection_id,
                workspace_id,
                parent_collection_id: None,
                name: "Imported".to_owned(),
                position: 0,
            }],
            environments: vec![Environment {
                id: environment_id,
                workspace_id,
                name: "Production".to_owned(),
                position: 0,
                is_selected: true,
            }],
            collection_variables: Vec::new(),
            environment_variables: vec![EnvironmentVariable {
                environment_id,
                workspace_id,
                variable: Variable {
                    name: "baseUrl".to_owned(),
                    value: VariableValue::Plain("https://example.test".to_owned()),
                },
            }],
            saved_requests: vec![SavedRequest {
                id: SavedRequestId::new(),
                workspace_id,
                collection_id: Some(collection_id),
                position: 0,
                content: RequestContent {
                    name: "Create user".to_owned(),
                    method: "POST".to_owned(),
                    url: "{{baseUrl}}/users".to_owned(),
                    headers: vec![OrderedField {
                        enabled: true,
                        order: 0,
                        name: "Content-Type".to_owned(),
                        value: "application/json".to_owned(),
                    }],
                    body: RequestBody::Raw {
                        content: r#"{"name":"Ada"}"#.to_owned(),
                    },
                    ..RequestContent::blank()
                },
            }],
            drafts: Vec::new(),
            tabs: Vec::new(),
        };
        let exported = export_postman(
            &PostmanExportInput {
                workspace_id,
                source_name: "Fixture".to_owned(),
            },
            &snapshot,
        )
        .expect("export");

        let converted = convert_postman(
            &PostmanImportInput {
                workspace_id,
                source_name: "Fixture".to_owned(),
                collection_json: exported.collection_json,
                environment_json: Some(exported.environments[0].environment_json.clone()),
            },
            None,
        )
        .expect("import exported json");

        assert_eq!(converted.collections.len(), 2);
        assert_eq!(converted.requests.len(), 1);
        assert_eq!(converted.environments.len(), 1);
        assert_eq!(converted.requests[0].content.name, "Create user");
        assert_eq!(
            converted.requests[0].content.headers[0].name,
            "Content-Type"
        );
        assert!(matches!(
            converted.requests[0].content.body,
            RequestBody::Raw { .. }
        ));
    }

    #[test]
    fn reimport_preview_reports_deterministic_prior_import_changes() {
        let import_preview = PostmanImportPreview {
            source_id: "postman-demo".to_owned(),
            source_name: "Demo".to_owned(),
            source_hash: "new-hash".to_owned(),
            collection_count: 1,
            request_count: 1,
            environment_count: 1,
            warning_count: 0,
            unsupported_count: 0,
            warnings: Vec::new(),
            unsupported: Vec::new(),
        };
        let preview = reimport_preview(
            import_preview,
            Some(StoredPostmanImportRecord {
                id: "record-1".to_owned(),
                source_id: "postman-demo".to_owned(),
                source_name: "Demo".to_owned(),
                source_hash: "old-hash".to_owned(),
                collection_ids: vec![CollectionId::new()],
                environment_ids: vec![EnvironmentId::new()],
            }),
        );

        assert!(preview.can_update);
        assert_eq!(
            preview
                .prior_import
                .as_ref()
                .map(|prior| prior.source_id.as_str()),
            Some("postman-demo")
        );
        assert_eq!(preview.changes[0].location, "$.source");
        assert_eq!(
            preview.changes[0].message,
            "incoming source differs from the previous import"
        );
        assert_eq!(
            preview.changes[1].message,
            "update would replace 1 imported collection root(s) and 1 environment(s)"
        );
    }

    struct FailingRepository;

    impl PostmanImportRepository for FailingRepository {
        fn list_postman_workspace(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<RequestWorkspaceSnapshot, PostmanImportError> {
            Ok(RequestWorkspaceSnapshot {
                workspace_id,
                collection_folders: Vec::new(),
                environments: Vec::new(),
                collection_variables: Vec::new(),
                environment_variables: Vec::new(),
                saved_requests: Vec::new(),
                drafts: Vec::new(),
                tabs: Vec::new(),
            })
        }

        fn find_latest_postman_import(
            &self,
            _workspace_id: WorkspaceId,
            _source_id: &str,
        ) -> Result<Option<StoredPostmanImportRecord>, PostmanImportError> {
            Ok(None)
        }

        fn import_postman(
            &mut self,
            _import: ConvertedPostmanImport,
        ) -> Result<RequestWorkspaceSnapshot, PostmanImportError> {
            Err(PostmanImportError::Persistence("forced".to_owned()))
        }

        fn update_postman_import(
            &mut self,
            _prior: &StoredPostmanImportRecord,
            _import: ConvertedPostmanImport,
        ) -> Result<RequestWorkspaceSnapshot, PostmanImportError> {
            Err(PostmanImportError::Persistence("forced".to_owned()))
        }
    }

    #[derive(Default)]
    struct RecordingSecretStore {
        values: Mutex<HashMap<String, String>>,
    }

    impl SecretStore for RecordingSecretStore {
        fn put(&self, owner: &SecretOwner, value: &str) -> Result<SecretWrite, SecretError> {
            let reference = format!("secret://test/{}", owner.name);
            self.values
                .lock()
                .expect("secrets")
                .insert(reference.clone(), value.to_owned());
            Ok(SecretWrite {
                reference,
                persistence: SecretPersistence::SessionOnly,
            })
        }

        fn get(&self, reference: &str) -> Result<String, SecretError> {
            self.values
                .lock()
                .expect("secrets")
                .get(reference)
                .cloned()
                .ok_or(SecretError::NotFound)
        }

        fn delete(&self, reference: &str) -> Result<(), SecretError> {
            self.values.lock().expect("secrets").remove(reference);
            Ok(())
        }

        fn delete_workspace(&self, _workspace_id: WorkspaceId) -> Result<(), SecretError> {
            self.values.lock().expect("secrets").clear();
            Ok(())
        }
    }
}
