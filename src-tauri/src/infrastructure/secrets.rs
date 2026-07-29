use std::collections::HashMap;

use secret_service::{blocking::SecretService, EncryptionType, Error as SecretServiceError};

use crate::{
    application::secrets::{
        new_reference, parse_postmite_reference, SecretError, SecretOwner, SecretPersistence,
        SecretStore, SecretWrite,
    },
    domain::workspace::WorkspaceId,
};

const ATTR_APP: &str = "app";
const ATTR_CLASS: &str = "class";
const ATTR_REFERENCE: &str = "reference";
const ATTR_WORKSPACE_ID: &str = "workspace-id";
const CONTENT_TYPE_TEXT: &str = "text/plain";
const POSTMITE_APP: &str = "postmite";

#[derive(Clone, Default)]
pub struct LinuxSecretServiceStore;

impl LinuxSecretServiceStore {
    pub fn new() -> Self {
        Self
    }
}

impl SecretStore for LinuxSecretServiceStore {
    fn put(&self, owner: &SecretOwner, value: &str) -> Result<SecretWrite, SecretError> {
        let reference = new_reference();
        let service = connect()?;
        let collection = service.get_default_collection().map_err(map_error)?;
        collection.ensure_unlocked().map_err(map_error)?;
        let workspace_id = owner.workspace_id.to_string();
        let attributes = HashMap::from([
            (ATTR_APP, POSTMITE_APP),
            (ATTR_REFERENCE, reference.as_str()),
            (ATTR_WORKSPACE_ID, workspace_id.as_str()),
            (ATTR_CLASS, owner.class.as_str()),
        ]);
        collection
            .create_item(
                &format!("Postmite {}", owner.class.as_str()),
                attributes,
                value.as_bytes(),
                false,
                CONTENT_TYPE_TEXT,
            )
            .map_err(map_error)?;
        Ok(SecretWrite {
            reference,
            persistence: SecretPersistence::Native,
        })
    }

    fn get(&self, reference: &str) -> Result<String, SecretError> {
        let service = connect()?;
        let item = find_one(&service, reference)?.ok_or(SecretError::NotFound)?;
        let bytes = item.get_secret().map_err(map_error)?;
        String::from_utf8(bytes).map_err(|error| SecretError::Storage(error.to_string()))
    }

    fn delete(&self, reference: &str) -> Result<(), SecretError> {
        let service = connect()?;
        if let Some(item) = find_one(&service, reference)? {
            item.delete().map_err(map_error)?;
        }
        Ok(())
    }

    fn delete_workspace(&self, workspace_id: WorkspaceId) -> Result<(), SecretError> {
        let service = connect()?;
        let workspace = workspace_id.to_string();
        let search = service
            .search_items(HashMap::from([
                (ATTR_APP, POSTMITE_APP),
                (ATTR_WORKSPACE_ID, workspace.as_str()),
            ]))
            .map_err(map_error)?;
        for item in search.unlocked {
            item.delete().map_err(map_error)?;
        }
        if !search.locked.is_empty() {
            return Err(SecretError::Locked);
        }
        Ok(())
    }

    fn contains(&self, reference: &str) -> bool {
        connect()
            .and_then(|service| find_one(&service, reference).map(|item| item.is_some()))
            .unwrap_or(false)
    }
}

fn connect() -> Result<SecretService<'static>, SecretError> {
    SecretService::connect(EncryptionType::Dh).map_err(map_error)
}

fn find_one<'a>(
    service: &'a SecretService<'static>,
    reference: &str,
) -> Result<Option<secret_service::blocking::Item<'a>>, SecretError> {
    if parse_postmite_reference(reference).is_none() {
        return Ok(None);
    }
    let search = service
        .search_items(HashMap::from([
            (ATTR_APP, POSTMITE_APP),
            (ATTR_REFERENCE, reference),
        ]))
        .map_err(map_error)?;
    if let Some(item) = search.unlocked.into_iter().next() {
        return Ok(Some(item));
    }
    if !search.locked.is_empty() {
        return Err(SecretError::Locked);
    }
    Ok(None)
}

fn map_error(error: SecretServiceError) -> SecretError {
    match error {
        SecretServiceError::Locked | SecretServiceError::Prompt => SecretError::Locked,
        SecretServiceError::Unavailable => SecretError::Unavailable,
        SecretServiceError::NoResult => SecretError::NotFound,
        error => SecretError::Storage(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::secrets::SecretClass;

    #[test]
    #[ignore = "requires an isolated unlocked Secret Service session"]
    fn secret_service_round_trips_and_deletes_postmite_references() {
        let store = LinuxSecretServiceStore::new();
        let owner = SecretOwner::new(WorkspaceId::new(), SecretClass::ProtectedVariable, "token");
        let write = store
            .put(&owner, "native-secret-service-value")
            .expect("write native secret");

        assert_eq!(
            store.get(&write.reference).expect("read native secret"),
            "native-secret-service-value"
        );

        store
            .delete(&write.reference)
            .expect("delete native secret");
        assert!(matches!(
            store.get(&write.reference),
            Err(SecretError::NotFound | SecretError::Unavailable | SecretError::Locked)
        ));
    }
}
