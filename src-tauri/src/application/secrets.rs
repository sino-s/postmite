use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::workspace::WorkspaceId;

const SECRET_REFERENCE_PREFIX: &str = "secret://postmite/";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SecretOwner {
    pub workspace_id: WorkspaceId,
    pub class: SecretClass,
    pub name: String,
}

impl SecretOwner {
    pub fn new(workspace_id: WorkspaceId, class: SecretClass, name: impl Into<String>) -> Self {
        Self {
            workspace_id,
            class,
            name: name.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SecretClass {
    ProtectedVariable,
    CookieValue,
    AuthCredential,
    ProxyCredential,
    PrivateKeyPassphrase,
}

impl SecretClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProtectedVariable => "protected-variable",
            Self::CookieValue => "cookie-value",
            Self::AuthCredential => "auth-credential",
            Self::ProxyCredential => "proxy-credential",
            Self::PrivateKeyPassphrase => "private-key-passphrase",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretPersistence {
    Native,
    SessionOnly,
}

pub trait SecretStore: Send + Sync {
    fn put(&self, owner: &SecretOwner, value: &str) -> Result<SecretWrite, SecretError>;
    fn get(&self, reference: &str) -> Result<String, SecretError>;
    fn delete(&self, reference: &str) -> Result<(), SecretError>;
    fn delete_workspace(&self, workspace_id: WorkspaceId) -> Result<(), SecretError>;
    fn contains(&self, reference: &str) -> bool {
        self.get(reference).is_ok()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretWrite {
    pub reference: String,
    pub persistence: SecretPersistence,
}

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("secret storage is locked")]
    Locked,
    #[error("native secret storage is unavailable; using session-only storage")]
    Unavailable,
    #[error("secret reference was not found")]
    NotFound,
    #[error("secret storage failed: {0}")]
    Storage(String),
}

impl SecretError {
    pub fn storage(error: impl std::error::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

#[derive(Default)]
pub struct SessionSecretStore {
    values: Mutex<HashMap<String, SessionSecret>>,
}

#[derive(Clone)]
struct SessionSecret {
    owner: SecretOwner,
    value: String,
}

impl SessionSecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for SessionSecretStore {
    fn put(&self, owner: &SecretOwner, value: &str) -> Result<SecretWrite, SecretError> {
        let reference = new_reference();
        self.values
            .lock()
            .map_err(|error| SecretError::Storage(error.to_string()))?
            .insert(
                reference.clone(),
                SessionSecret {
                    owner: owner.clone(),
                    value: value.to_owned(),
                },
            );
        Ok(SecretWrite {
            reference,
            persistence: SecretPersistence::SessionOnly,
        })
    }

    fn get(&self, reference: &str) -> Result<String, SecretError> {
        self.values
            .lock()
            .map_err(|error| SecretError::Storage(error.to_string()))?
            .get(reference)
            .map(|secret| secret.value.clone())
            .ok_or(SecretError::NotFound)
    }

    fn delete(&self, reference: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .map_err(|error| SecretError::Storage(error.to_string()))?
            .remove(reference);
        Ok(())
    }

    fn delete_workspace(&self, workspace_id: WorkspaceId) -> Result<(), SecretError> {
        self.values
            .lock()
            .map_err(|error| SecretError::Storage(error.to_string()))?
            .retain(|_, secret| secret.owner.workspace_id != workspace_id);
        Ok(())
    }
}

pub struct FallbackSecretStore<N> {
    native: N,
    session: Arc<SessionSecretStore>,
}

impl<N> FallbackSecretStore<N> {
    pub fn new(native: N, session: Arc<SessionSecretStore>) -> Self {
        Self { native, session }
    }
}

impl<N> SecretStore for FallbackSecretStore<N>
where
    N: SecretStore,
{
    fn put(&self, owner: &SecretOwner, value: &str) -> Result<SecretWrite, SecretError> {
        match self.native.put(owner, value) {
            Ok(write) => Ok(write),
            Err(SecretError::Unavailable | SecretError::Locked) => self.session.put(owner, value),
            Err(error) => Err(error),
        }
    }

    fn get(&self, reference: &str) -> Result<String, SecretError> {
        self.native
            .get(reference)
            .or_else(|_| self.session.get(reference))
    }

    fn delete(&self, reference: &str) -> Result<(), SecretError> {
        match self.native.delete(reference) {
            Ok(()) | Err(SecretError::NotFound | SecretError::Unavailable) => {
                self.session.delete(reference)
            }
            Err(error) => Err(error),
        }
    }

    fn delete_workspace(&self, workspace_id: WorkspaceId) -> Result<(), SecretError> {
        match self.native.delete_workspace(workspace_id) {
            Ok(()) | Err(SecretError::Unavailable) => {}
            Err(error) => return Err(error),
        }
        self.session.delete_workspace(workspace_id)
    }

    fn contains(&self, reference: &str) -> bool {
        self.native.contains(reference) || self.session.contains(reference)
    }
}

pub fn new_reference() -> String {
    format!("{SECRET_REFERENCE_PREFIX}{}", Uuid::new_v4())
}

pub fn parse_postmite_reference(reference: &str) -> Option<&str> {
    reference.strip_prefix(SECRET_REFERENCE_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct UnavailableStore;

    impl SecretStore for UnavailableStore {
        fn put(&self, _owner: &SecretOwner, _value: &str) -> Result<SecretWrite, SecretError> {
            Err(SecretError::Unavailable)
        }

        fn get(&self, _reference: &str) -> Result<String, SecretError> {
            Err(SecretError::Unavailable)
        }

        fn delete(&self, _reference: &str) -> Result<(), SecretError> {
            Err(SecretError::Unavailable)
        }

        fn delete_workspace(&self, _workspace_id: WorkspaceId) -> Result<(), SecretError> {
            Err(SecretError::Unavailable)
        }
    }

    #[test]
    fn secret_session_fallback_stores_values_without_plaintext_reference() {
        let store = FallbackSecretStore::new(UnavailableStore, Arc::new(SessionSecretStore::new()));
        let owner = SecretOwner::new(WorkspaceId::new(), SecretClass::ProtectedVariable, "token");

        let write = store
            .put(&owner, "session-test-value")
            .expect("session fallback write");

        assert_eq!(write.persistence, SecretPersistence::SessionOnly);
        assert!(parse_postmite_reference(&write.reference).is_some());
        assert!(!write.reference.contains("session-test-value"));
        assert_eq!(
            store
                .get(&write.reference)
                .expect("resolve fallback secret"),
            "session-test-value"
        );
    }

    #[test]
    fn secret_session_workspace_delete_removes_owned_values() {
        let store = SessionSecretStore::new();
        let workspace_id = WorkspaceId::new();
        let owner = SecretOwner::new(workspace_id, SecretClass::CookieValue, "cookie");
        let write = store.put(&owner, "session-cookie-value").expect("write");

        store
            .delete_workspace(workspace_id)
            .expect("delete workspace secrets");

        assert_eq!(
            store.get(&write.reference).unwrap_err().to_string(),
            "secret reference was not found"
        );
    }
}
