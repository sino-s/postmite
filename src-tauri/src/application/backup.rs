use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{Cursor, Read, Seek, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

use crate::domain::{
    request::{
        BodyFilePath, BodyFileReference, MultipartPart, RequestAuth, RequestBody, RequestContent,
        TlsPolicy, VariableValue, WorkspaceCookie,
    },
    workspace::{WorkspaceId, WorkspaceName},
};

use super::{
    request::{ExecutionHistorySnapshot, RequestWorkspaceSnapshot},
    workspace::WorkspaceSnapshot,
};

const FORMAT_VERSION: u32 = 1;
const MANIFEST_ENTRY: &str = "manifest.json";
const WORKSPACE_ENTRY: &str = "workspace.json";
const BODY_FILE_PREFIX: &str = "body-files/";
const MAX_ENTRY_BYTES: u64 = 25 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeBackupExportInput {
    pub workspace_id: WorkspaceId,
    pub backup_path: String,
    pub include_body_files: bool,
    pub body_files_directory: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeBackupRestorePreviewInput {
    pub backup_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeBackupRestoreInput {
    pub backup_path: String,
    pub workspace_name: String,
    pub body_files_directory: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeBackupExportResult {
    pub backup_path: String,
    pub manifest: NativeBackupManifest,
    pub preview: NativeBackupRestorePreview,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NativeBackupRestoreResult {
    pub preview: NativeBackupRestorePreview,
    pub workspace_snapshot: WorkspaceSnapshot,
    pub request_snapshot: RequestWorkspaceSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupRestorePreview {
    pub source_workspace_name: String,
    pub collection_count: u32,
    pub request_count: u32,
    pub environment_count: u32,
    pub history_record_count: u32,
    pub cookie_count: u32,
    pub body_file_count: u32,
    pub expanded_bytes: u64,
    pub exclusions: Vec<NativeBackupExclusion>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupExclusion {
    pub location: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupManifest {
    pub format: String,
    pub version: u32,
    pub required_features: Vec<String>,
    pub entries: Vec<NativeBackupManifestEntry>,
    pub exclusions: Vec<NativeBackupExclusion>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupManifestEntry {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupData {
    pub workspace: NativeBackupWorkspace,
    pub requests: RequestWorkspaceSnapshot,
    pub execution_history: ExecutionHistorySnapshot,
    pub cookies: Vec<WorkspaceCookie>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBackupWorkspace {
    pub id: WorkspaceId,
    pub name: String,
    pub base_directory: Option<String>,
}

pub trait NativeBackupRepository {
    fn export_native_backup(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<NativeBackupData, NativeBackupError>;
    fn restore_native_backup(
        &mut self,
        backup: NativeBackupData,
        workspace_name: WorkspaceName,
    ) -> Result<(WorkspaceSnapshot, RequestWorkspaceSnapshot), NativeBackupError>;
}

pub struct NativeBackupService<R> {
    repository: R,
}

impl<R> NativeBackupService<R>
where
    R: NativeBackupRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn export(
        &self,
        input: NativeBackupExportInput,
    ) -> Result<NativeBackupExportResult, NativeBackupError> {
        let mut data =
            sanitize_backup_data(self.repository.export_native_backup(input.workspace_id)?);
        let mut body_files = BTreeMap::new();
        if input.include_body_files {
            let base = input.body_files_directory.as_deref().map(Path::new);
            for reference in collect_body_file_references(&data.requests) {
                let source = resolve_body_file_path(base, &reference)?;
                let bytes = fs::read(&source).map_err(NativeBackupError::io)?;
                verify_body_file_bytes(&reference, &bytes)?;
                let entry = format!("{BODY_FILE_PREFIX}{}", reference.sha256);
                body_files.insert(entry.clone(), bytes);
                rewrite_body_file_paths(&mut data.requests, &reference.sha256, entry);
            }
        }

        let workspace_json = canonical_json(&data)?;
        let mut manifest = NativeBackupManifest {
            format: "postmite.native-backup".to_owned(),
            version: FORMAT_VERSION,
            required_features: Vec::new(),
            entries: vec![manifest_entry(WORKSPACE_ENTRY, &workspace_json)?],
            exclusions: data_exclusions(&data),
        };
        for (path, bytes) in &body_files {
            manifest.entries.push(manifest_entry(path, bytes)?);
        }
        let manifest_json = canonical_json(&manifest)?;

        let path = PathBuf::from(&input.backup_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(NativeBackupError::io)?;
        }
        let file = File::create(&path).map_err(NativeBackupError::io)?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(MANIFEST_ENTRY, options)
            .map_err(NativeBackupError::zip)?;
        zip.write_all(&manifest_json)
            .map_err(NativeBackupError::io)?;
        zip.start_file(WORKSPACE_ENTRY, options)
            .map_err(NativeBackupError::zip)?;
        zip.write_all(&workspace_json)
            .map_err(NativeBackupError::io)?;
        for (entry, bytes) in &body_files {
            zip.start_file(entry, options)
                .map_err(NativeBackupError::zip)?;
            zip.write_all(bytes).map_err(NativeBackupError::io)?;
        }
        zip.finish().map_err(NativeBackupError::zip)?;

        let preview = preview_from_data(
            &data,
            manifest.entries.iter().map(|entry| entry.bytes).sum(),
        );
        Ok(NativeBackupExportResult {
            backup_path: input.backup_path,
            manifest,
            preview,
        })
    }

    pub fn preview_restore(
        &self,
        input: NativeBackupRestorePreviewInput,
    ) -> Result<NativeBackupRestorePreview, NativeBackupError> {
        read_verified_archive(&input.backup_path).map(|archive| archive.preview)
    }

    pub fn restore(
        &mut self,
        input: NativeBackupRestoreInput,
    ) -> Result<NativeBackupRestoreResult, NativeBackupError> {
        let archive = read_verified_archive(&input.backup_path)?;
        let workspace_name = WorkspaceName::new(input.workspace_name)
            .map_err(|error| NativeBackupError::InvalidInput(format!("workspace.name.{error}")))?;
        let (workspace_snapshot, request_snapshot) = self
            .repository
            .restore_native_backup(archive.data.clone(), workspace_name)?;
        if let Some(directory) = input.body_files_directory {
            restore_body_files(&directory, &archive.body_files)?;
        } else if !archive.body_files.is_empty() {
            return Err(NativeBackupError::InvalidInput(
                "backup.restore.bodyFilesDirectoryRequired".to_owned(),
            ));
        }
        Ok(NativeBackupRestoreResult {
            preview: archive.preview,
            workspace_snapshot,
            request_snapshot,
        })
    }
}

#[derive(Debug, Error)]
pub enum NativeBackupError {
    #[error("backup input is invalid: {0}")]
    InvalidInput(String),
    #[error("workspace was not found")]
    WorkspaceNotFound,
    #[error("workspace name already exists")]
    WorkspaceAlreadyExists,
    #[error("backup archive is invalid: {0}")]
    InvalidArchive(String),
    #[error("backup persistence failed: {0}")]
    Persistence(String),
}

impl NativeBackupError {
    pub fn persistence(error: impl std::error::Error) -> Self {
        Self::Persistence(error.to_string())
    }

    fn io(error: std::io::Error) -> Self {
        Self::InvalidArchive(error.to_string())
    }

    fn zip(error: zip::result::ZipError) -> Self {
        Self::InvalidArchive(error.to_string())
    }
}

struct VerifiedArchive {
    data: NativeBackupData,
    body_files: BTreeMap<String, Vec<u8>>,
    preview: NativeBackupRestorePreview,
}

fn read_verified_archive(path: &str) -> Result<VerifiedArchive, NativeBackupError> {
    let file = File::open(path).map_err(NativeBackupError::io)?;
    let mut archive = ZipArchive::new(file).map_err(NativeBackupError::zip)?;
    let manifest_bytes = read_zip_entry(&mut archive, MANIFEST_ENTRY)?;
    let manifest: NativeBackupManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|error| {
            NativeBackupError::InvalidArchive(format!("manifest.invalidJson: {error}"))
        })?;
    validate_manifest(&manifest)?;

    let expected = manifest
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut total = 0_u64;
    let mut body_files = BTreeMap::new();
    let mut workspace_bytes = None;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(NativeBackupError::zip)?;
        let name = file.name().to_owned();
        validate_archive_path(&name)?;
        if name == MANIFEST_ENTRY {
            continue;
        }
        let Some(entry) = expected.get(name.as_str()) else {
            return Err(NativeBackupError::InvalidArchive(format!(
                "backup.entry.unexpected:{name}"
            )));
        };
        if file.size() > MAX_ENTRY_BYTES {
            return Err(NativeBackupError::InvalidArchive(
                "backup.entry.tooLarge".to_owned(),
            ));
        }
        total = total.saturating_add(file.size());
        if total > MAX_TOTAL_BYTES {
            return Err(NativeBackupError::InvalidArchive(
                "backup.expandedSize.tooLarge".to_owned(),
            ));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(NativeBackupError::io)?;
        if bytes.len() as u64 != entry.bytes || sha256_hex(&bytes) != entry.sha256 {
            return Err(NativeBackupError::InvalidArchive(format!(
                "backup.entry.hashMismatch:{name}"
            )));
        }
        if name == WORKSPACE_ENTRY {
            workspace_bytes = Some(bytes);
        } else if name.starts_with(BODY_FILE_PREFIX) {
            body_files.insert(name, bytes);
        }
    }
    let workspace_bytes = workspace_bytes
        .ok_or_else(|| NativeBackupError::InvalidArchive("backup.workspace.missing".to_owned()))?;
    let data: NativeBackupData = serde_json::from_slice(&workspace_bytes).map_err(|error| {
        NativeBackupError::InvalidArchive(format!("workspace.invalidJson: {error}"))
    })?;
    let preview = preview_from_data(&data, total);
    Ok(VerifiedArchive {
        data,
        body_files,
        preview,
    })
}

fn read_zip_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, NativeBackupError> {
    let mut file = archive.by_name(name).map_err(NativeBackupError::zip)?;
    if file.size() > MAX_ENTRY_BYTES {
        return Err(NativeBackupError::InvalidArchive(
            "backup.entry.tooLarge".to_owned(),
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(NativeBackupError::io)?;
    Ok(bytes)
}

fn validate_manifest(manifest: &NativeBackupManifest) -> Result<(), NativeBackupError> {
    if manifest.format != "postmite.native-backup" || manifest.version != FORMAT_VERSION {
        return Err(NativeBackupError::InvalidArchive(
            "backup.manifest.unsupportedVersion".to_owned(),
        ));
    }
    if !manifest.required_features.is_empty() {
        return Err(NativeBackupError::InvalidArchive(
            "backup.manifest.unknownRequiredFeature".to_owned(),
        ));
    }
    let mut paths = HashSet::new();
    for entry in &manifest.entries {
        validate_archive_path(&entry.path)?;
        if entry.path == MANIFEST_ENTRY || !paths.insert(entry.path.as_str()) {
            return Err(NativeBackupError::InvalidArchive(
                "backup.manifest.duplicateEntry".to_owned(),
            ));
        }
        if entry.bytes > MAX_ENTRY_BYTES {
            return Err(NativeBackupError::InvalidArchive(
                "backup.manifest.entryTooLarge".to_owned(),
            ));
        }
    }
    if !paths.contains(WORKSPACE_ENTRY) {
        return Err(NativeBackupError::InvalidArchive(
            "backup.manifest.workspaceMissing".to_owned(),
        ));
    }
    Ok(())
}

fn validate_archive_path(path: &str) -> Result<(), NativeBackupError> {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return Err(NativeBackupError::InvalidArchive(
            "backup.path.absolute".to_owned(),
        ));
    }
    let path = Path::new(path);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(NativeBackupError::InvalidArchive(
            "backup.path.traversal".to_owned(),
        ));
    }
    Ok(())
}

fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, NativeBackupError> {
    serde_json::to_vec_pretty(value)
        .map_err(|error| NativeBackupError::Persistence(error.to_string()))
}

fn manifest_entry(
    path: &str,
    bytes: &[u8],
) -> Result<NativeBackupManifestEntry, NativeBackupError> {
    validate_archive_path(path)?;
    Ok(NativeBackupManifestEntry {
        path: path.to_owned(),
        sha256: sha256_hex(bytes),
        bytes: bytes.len() as u64,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sanitize_backup_data(mut data: NativeBackupData) -> NativeBackupData {
    for variable in &mut data.requests.collection_variables {
        if matches!(variable.variable.value, VariableValue::SecretReference(_)) {
            variable.variable.value = VariableValue::SecretReference("excluded".to_owned());
        }
    }
    for variable in &mut data.requests.environment_variables {
        if matches!(variable.variable.value, VariableValue::SecretReference(_)) {
            variable.variable.value = VariableValue::SecretReference("excluded".to_owned());
        }
    }
    for request in &mut data.requests.saved_requests {
        sanitize_content(&mut request.content);
    }
    for draft in &mut data.requests.drafts {
        sanitize_content(&mut draft.content);
    }
    for record in &mut data.execution_history.records {
        sanitize_content(&mut record.request);
    }
    for cookie in &mut data.cookies {
        cookie.has_value = false;
        cookie.secret_reference = None;
    }
    data
}

fn sanitize_content(content: &mut RequestContent) {
    content.auth = match &content.auth {
        RequestAuth::Basic { username, .. } => RequestAuth::Basic {
            username: username.clone(),
            password: "excluded".to_owned(),
        },
        RequestAuth::Bearer { .. } => RequestAuth::Bearer {
            token: "excluded".to_owned(),
        },
        RequestAuth::ApiKey {
            placement, name, ..
        } => RequestAuth::ApiKey {
            placement: *placement,
            name: name.clone(),
            value: "excluded".to_owned(),
        },
        RequestAuth::ClientCredentials {
            token_endpoint,
            client_id,
            scopes,
            ..
        } => RequestAuth::ClientCredentials {
            token_endpoint: token_endpoint.clone(),
            client_id: client_id.clone(),
            client_secret: "excluded".to_owned(),
            scopes: scopes.clone(),
        },
        RequestAuth::None => RequestAuth::None,
    };
    content.tls = TlsPolicy {
        verify: content.tls.verify,
        custom_ca_reference: None,
        client_certificate_reference: None,
        client_key_reference: None,
    };
}

fn data_exclusions(data: &NativeBackupData) -> Vec<NativeBackupExclusion> {
    let mut exclusions = Vec::new();
    for variable in &data.requests.collection_variables {
        if matches!(variable.variable.value, VariableValue::SecretReference(_)) {
            exclusions.push(exclusion(
                format!("collectionVariables.{}", variable.variable.name),
                "secretValue",
            ));
        }
    }
    for variable in &data.requests.environment_variables {
        if matches!(variable.variable.value, VariableValue::SecretReference(_)) {
            exclusions.push(exclusion(
                format!("environmentVariables.{}", variable.variable.name),
                "secretValue",
            ));
        }
    }
    if !data.cookies.is_empty() {
        exclusions.push(exclusion("cookies", "cookieValues"));
    }
    exclusions
}

fn exclusion(location: impl Into<String>, reason: impl Into<String>) -> NativeBackupExclusion {
    NativeBackupExclusion {
        location: location.into(),
        reason: reason.into(),
    }
}

fn preview_from_data(data: &NativeBackupData, expanded_bytes: u64) -> NativeBackupRestorePreview {
    NativeBackupRestorePreview {
        source_workspace_name: data.workspace.name.clone(),
        collection_count: data.requests.collection_folders.len() as u32,
        request_count: data.requests.saved_requests.len() as u32,
        environment_count: data.requests.environments.len() as u32,
        history_record_count: data.execution_history.records.len() as u32,
        cookie_count: data.cookies.len() as u32,
        body_file_count: collect_body_file_references(&data.requests).len() as u32,
        expanded_bytes,
        exclusions: data_exclusions(data),
        warnings: Vec::new(),
    }
}

fn collect_body_file_references(snapshot: &RequestWorkspaceSnapshot) -> Vec<BodyFileReference> {
    let mut files = Vec::new();
    for content in snapshot
        .saved_requests
        .iter()
        .map(|request| &request.content)
        .chain(snapshot.drafts.iter().map(|draft| &draft.content))
    {
        collect_body_file_references_from_body(&content.body, &mut files);
    }
    files
}

fn collect_body_file_references_from_body(body: &RequestBody, files: &mut Vec<BodyFileReference>) {
    match body {
        RequestBody::Binary { file } => files.push(file.clone()),
        RequestBody::Multipart { parts } => {
            for part in parts {
                if let MultipartPart::File { file, .. } = part {
                    files.push(file.clone());
                }
            }
        }
        RequestBody::None | RequestBody::Raw { .. } | RequestBody::UrlEncoded { .. } => {}
    }
}

fn resolve_body_file_path(
    base: Option<&Path>,
    reference: &BodyFileReference,
) -> Result<PathBuf, NativeBackupError> {
    match &reference.path {
        BodyFilePath::Absolute { path } => Ok(PathBuf::from(path)),
        BodyFilePath::Relative { path } => base.map(|base| base.join(path)).ok_or_else(|| {
            NativeBackupError::InvalidInput("backup.bodyFiles.baseDirectoryRequired".to_owned())
        }),
    }
}

fn verify_body_file_bytes(
    reference: &BodyFileReference,
    bytes: &[u8],
) -> Result<(), NativeBackupError> {
    if bytes.len() as u64 != reference.size || sha256_hex(bytes) != reference.sha256 {
        return Err(NativeBackupError::InvalidInput(
            "backup.bodyFile.hashMismatch".to_owned(),
        ));
    }
    Ok(())
}

fn rewrite_body_file_paths(snapshot: &mut RequestWorkspaceSnapshot, sha256: &str, entry: String) {
    for content in snapshot
        .saved_requests
        .iter_mut()
        .map(|request| &mut request.content)
        .chain(snapshot.drafts.iter_mut().map(|draft| &mut draft.content))
    {
        rewrite_body_file_paths_in_body(&mut content.body, sha256, &entry);
    }
}

fn rewrite_body_file_paths_in_body(body: &mut RequestBody, sha256: &str, entry: &str) {
    match body {
        RequestBody::Binary { file } if file.sha256 == sha256 => {
            file.path = BodyFilePath::Relative {
                path: entry.to_owned(),
            };
        }
        RequestBody::Multipart { parts } => {
            for part in parts {
                if let MultipartPart::File { file, .. } = part {
                    if file.sha256 == sha256 {
                        file.path = BodyFilePath::Relative {
                            path: entry.to_owned(),
                        };
                    }
                }
            }
        }
        _ => {}
    }
}

fn restore_body_files(
    directory: &str,
    body_files: &BTreeMap<String, Vec<u8>>,
) -> Result<(), NativeBackupError> {
    let base = PathBuf::from(directory);
    fs::create_dir_all(&base).map_err(NativeBackupError::io)?;
    for (entry, bytes) in body_files {
        validate_archive_path(entry)?;
        let _relative = entry.strip_prefix(BODY_FILE_PREFIX).ok_or_else(|| {
            NativeBackupError::InvalidArchive("backup.bodyFile.path.invalid".to_owned())
        })?;
        let destination = base.join(entry);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(NativeBackupError::io)?;
        }
        let mut cursor = Cursor::new(bytes);
        let mut file = File::create(destination).map_err(NativeBackupError::io)?;
        std::io::copy(&mut cursor, &mut file).map_err(NativeBackupError::io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, io::Write};

    use tempfile::{tempdir, NamedTempFile};
    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::domain::{
        request::{
            BodyFilePath, BodyFileReference, CollectionVariable, CookieId, OrderedField,
            RequestBody, SavedRequest, SavedRequestId, Variable, VariableValue,
        },
        workspace::{WorkspaceId, WorkspaceName},
    };

    #[derive(Clone)]
    struct FakeRepository {
        data: NativeBackupData,
        restored: RefCell<Option<NativeBackupData>>,
    }

    impl NativeBackupRepository for FakeRepository {
        fn export_native_backup(
            &self,
            _workspace_id: WorkspaceId,
        ) -> Result<NativeBackupData, NativeBackupError> {
            Ok(self.data.clone())
        }

        fn restore_native_backup(
            &mut self,
            backup: NativeBackupData,
            _workspace_name: WorkspaceName,
        ) -> Result<(WorkspaceSnapshot, RequestWorkspaceSnapshot), NativeBackupError> {
            self.restored.replace(Some(backup.clone()));
            Ok((
                WorkspaceSnapshot {
                    selected_workspace_id: backup.workspace.id,
                    workspaces: Vec::new(),
                },
                backup.requests,
            ))
        }
    }

    #[test]
    fn export_native_backup_omits_secret_and_cookie_values() {
        let workspace_id = WorkspaceId::new();
        let file = NamedTempFile::new().expect("backup file");
        let service = NativeBackupService::new(FakeRepository {
            data: fixture_data(workspace_id),
            restored: RefCell::new(None),
        });

        let result = service
            .export(NativeBackupExportInput {
                workspace_id,
                backup_path: file.path().to_string_lossy().to_string(),
                include_body_files: false,
                body_files_directory: None,
            })
            .expect("export backup");

        assert_eq!(result.preview.request_count, 1);
        assert!(result
            .manifest
            .exclusions
            .iter()
            .any(|exclusion| exclusion.reason == "cookieValues"));
        let mut archive =
            ZipArchive::new(File::open(file.path()).expect("open backup")).expect("zip");
        let mut workspace_json = String::new();
        archive
            .by_name(WORKSPACE_ENTRY)
            .expect("workspace entry")
            .read_to_string(&mut workspace_json)
            .expect("read workspace");
        assert!(!workspace_json.contains("plain-secret-token"));
        assert!(!workspace_json.contains("cookie-secret-ref"));
    }

    #[test]
    fn preview_restore_rejects_zip_slip_manifest_entries() {
        let file = NamedTempFile::new().expect("backup file");
        write_manifest_only_archive(
            file.path(),
            br#"{
              "format": "postmite.native-backup",
              "version": 1,
              "requiredFeatures": [],
              "entries": [{"path": "../workspace.json", "sha256": "abc", "bytes": 1}],
              "exclusions": []
            }"#,
        );
        let service = NativeBackupService::new(FakeRepository {
            data: fixture_data(WorkspaceId::new()),
            restored: RefCell::new(None),
        });

        let error = service
            .preview_restore(NativeBackupRestorePreviewInput {
                backup_path: file.path().to_string_lossy().to_string(),
            })
            .expect_err("reject traversal");

        assert!(matches!(error, NativeBackupError::InvalidArchive(_)));
    }

    #[test]
    fn preview_restore_rejects_unknown_required_features() {
        let file = NamedTempFile::new().expect("backup file");
        write_manifest_only_archive(
            file.path(),
            br#"{
              "format": "postmite.native-backup",
              "version": 1,
              "requiredFeatures": ["future"],
              "entries": [{"path": "workspace.json", "sha256": "abc", "bytes": 1}],
              "exclusions": []
            }"#,
        );
        let service = NativeBackupService::new(FakeRepository {
            data: fixture_data(WorkspaceId::new()),
            restored: RefCell::new(None),
        });

        let error = service
            .preview_restore(NativeBackupRestorePreviewInput {
                backup_path: file.path().to_string_lossy().to_string(),
            })
            .expect_err("reject required feature");

        assert!(matches!(error, NativeBackupError::InvalidArchive(_)));
    }

    #[test]
    fn preview_restore_rejects_hash_mismatch_and_size_limits() {
        let file = NamedTempFile::new().expect("backup file");
        let mut zip = ZipWriter::new(file.reopen().expect("reopen backup"));
        let options = SimpleFileOptions::default();
        zip.start_file(MANIFEST_ENTRY, options).expect("manifest");
        zip.write_all(
            br#"{
              "format": "postmite.native-backup",
              "version": 1,
              "requiredFeatures": [],
              "entries": [{"path": "workspace.json", "sha256": "0000", "bytes": 2}],
              "exclusions": []
            }"#,
        )
        .expect("write manifest");
        zip.start_file(WORKSPACE_ENTRY, options).expect("workspace");
        zip.write_all(b"{}").expect("write workspace");
        zip.finish().expect("finish zip");
        let service = NativeBackupService::new(FakeRepository {
            data: fixture_data(WorkspaceId::new()),
            restored: RefCell::new(None),
        });
        let error = service
            .preview_restore(NativeBackupRestorePreviewInput {
                backup_path: file.path().to_string_lossy().to_string(),
            })
            .expect_err("reject hash mismatch");
        assert!(matches!(error, NativeBackupError::InvalidArchive(_)));

        let oversized = NamedTempFile::new().expect("backup file");
        write_manifest_only_archive(
            oversized.path(),
            format!(
                r#"{{
                  "format": "postmite.native-backup",
                  "version": 1,
                  "requiredFeatures": [],
                  "entries": [{{"path": "workspace.json", "sha256": "abc", "bytes": {}}}],
                  "exclusions": []
                }}"#,
                MAX_ENTRY_BYTES + 1
            )
            .as_bytes(),
        );
        let error = service
            .preview_restore(NativeBackupRestorePreviewInput {
                backup_path: oversized.path().to_string_lossy().to_string(),
            })
            .expect_err("reject size limit");
        assert!(matches!(error, NativeBackupError::InvalidArchive(_)));
    }

    #[test]
    fn export_native_backup_includes_optional_body_files_by_hash() {
        let workspace_id = WorkspaceId::new();
        let body_dir = tempdir().expect("body dir");
        let body_bytes = b"body file";
        let sha256 = sha256_hex(body_bytes);
        std::fs::write(body_dir.path().join("upload.bin"), body_bytes).expect("body file");
        let mut data = fixture_data(workspace_id);
        data.requests.saved_requests[0].content.body = RequestBody::Binary {
            file: BodyFileReference {
                path: BodyFilePath::Relative {
                    path: "upload.bin".to_owned(),
                },
                file_name: "upload.bin".to_owned(),
                size: body_bytes.len() as u64,
                modified_at_epoch_seconds: None,
                sha256: sha256.clone(),
            },
        };
        let backup = NamedTempFile::new().expect("backup file");
        let service = NativeBackupService::new(FakeRepository {
            data,
            restored: RefCell::new(None),
        });

        let result = service
            .export(NativeBackupExportInput {
                workspace_id,
                backup_path: backup.path().to_string_lossy().to_string(),
                include_body_files: true,
                body_files_directory: Some(body_dir.path().to_string_lossy().to_string()),
            })
            .expect("export backup");

        assert_eq!(result.preview.body_file_count, 1);
        let mut archive =
            ZipArchive::new(File::open(backup.path()).expect("open backup")).expect("zip");
        assert!(archive
            .by_name(&format!("{BODY_FILE_PREFIX}{sha256}"))
            .is_ok());
    }

    fn write_manifest_only_archive(path: &Path, manifest: &[u8]) {
        let mut zip = ZipWriter::new(File::create(path).expect("create backup"));
        let options = SimpleFileOptions::default();
        zip.start_file(MANIFEST_ENTRY, options).expect("manifest");
        zip.write_all(manifest).expect("write manifest");
        zip.finish().expect("finish zip");
    }

    fn fixture_data(workspace_id: WorkspaceId) -> NativeBackupData {
        NativeBackupData {
            workspace: NativeBackupWorkspace {
                id: workspace_id,
                name: "Source".to_owned(),
                base_directory: None,
            },
            requests: RequestWorkspaceSnapshot {
                workspace_id,
                collection_folders: Vec::new(),
                environments: Vec::new(),
                collection_variables: vec![CollectionVariable {
                    workspace_id,
                    variable: Variable {
                        name: "token".to_owned(),
                        value: VariableValue::SecretReference("plain-secret-token".to_owned()),
                    },
                }],
                environment_variables: Vec::new(),
                saved_requests: vec![SavedRequest {
                    id: SavedRequestId::new(),
                    workspace_id,
                    collection_id: None,
                    position: 0,
                    content: RequestContent {
                        name: "Secret request".to_owned(),
                        method: "GET".to_owned(),
                        url: "https://example.test".to_owned(),
                        body: RequestBody::None,
                        query: Vec::new(),
                        headers: vec![OrderedField {
                            enabled: true,
                            order: 0,
                            name: "Authorization".to_owned(),
                            value: "Bearer {{token}}".to_owned(),
                        }],
                        auth: RequestAuth::Bearer {
                            token: "plain-secret-token".to_owned(),
                        },
                        ..RequestContent::blank()
                    },
                }],
                drafts: Vec::new(),
                tabs: Vec::new(),
            },
            execution_history: ExecutionHistorySnapshot {
                workspace_id,
                disabled: false,
                records: Vec::new(),
                warning: ExecutionHistorySnapshot::warning_text(),
            },
            cookies: vec![WorkspaceCookie {
                id: CookieId::new(),
                workspace_id,
                name: "sid".to_owned(),
                domain: "example.test".to_owned(),
                path: "/".to_owned(),
                secure: true,
                http_only: true,
                same_site: None,
                expires_at_epoch_seconds: None,
                session: true,
                has_value: true,
                secret_reference: Some("cookie-secret-ref".to_owned()),
            }],
        }
    }
}
