use crate::storage::Storage;
use anyhow::{bail, Result};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Manages Ed25519 identity keypairs for signing timesheets
pub struct IdentityManager {
    storage: Arc<dyn Storage>,
}

impl IdentityManager {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// Get the path for a private key file
    fn get_key_path(&self, name: &str) -> PathBuf {
        self.storage.identity_dir().join(format!("id_{}", name))
    }

    /// Get the path for a public key file
    fn get_pub_path(&self, name: &str) -> PathBuf {
        self.storage.identity_dir().join(format!("id_{}.pub", name))
    }

    /// Create a new Ed25519 identity keypair
    ///
    /// Keys are stored as base64-encoded strings:
    /// - Private key: ~/.faff/identities/id_{name}
    /// - Public key: ~/.faff/identities/id_{name}.pub
    pub fn create_identity(&self, name: &str, overwrite: bool) -> Result<SigningKey> {
        let private_path = self.get_key_path(name);
        let public_path = self.get_pub_path(name);

        if !overwrite && self.storage.exists(&private_path) {
            bail!("Identity '{}' already exists at {:?}", name, private_path);
        }

        // Ensure identity directory exists
        let identity_dir = self.storage.identity_dir();
        self.storage.create_dir_all(&identity_dir)?;

        // Generate new keypair
        let mut csprng = OsRng;
        let mut secret_bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut csprng, &mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        // Encode keys as base64
        let b64_private = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            signing_key.to_bytes(),
        );
        let b64_public = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            verifying_key.to_bytes(),
        );

        // Write keys to files
        self.storage.write_string(&private_path, &b64_private)?;
        self.storage.write_string(&public_path, &b64_public)?;

        // Note: File permissions (chmod 0o600) should be handled by the Storage implementation
        // if it's a real filesystem. For testing with mock storage, this is skipped.

        Ok(signing_key)
    }

    /// Get a specific identity by name
    pub fn get_identity(&self, name: &str) -> Result<Option<SigningKey>> {
        let identities = self.get()?;
        Ok(identities.get(name).cloned())
    }

    /// Get all identities
    ///
    /// Returns a HashMap where keys are identity names and values are SigningKeys
    pub fn get(&self) -> Result<HashMap<String, SigningKey>> {
        let identity_dir = self.storage.identity_dir();
        let mut identities = HashMap::new();

        // List all files matching "id_*" pattern
        let files = self.storage.list_files(&identity_dir, "id_*")?;

        for file in files {
            // Skip public key files
            if file.extension().and_then(|s| s.to_str()) == Some("pub") {
                continue;
            }

            // Extract identity name (remove "id_" prefix)
            let filename = file
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

            if !filename.starts_with("id_") {
                continue;
            }

            let name = &filename[3..]; // Remove "id_" prefix

            // Read and decode the private key
            let b64_private = self.storage.read_string(&file)?;
            let key_bytes = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                b64_private.trim(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to decode key in {:?}: {}", file, e))?;

            if key_bytes.len() != 32 {
                bail!(
                    "Invalid key length in {:?}: expected 32 bytes, got {}",
                    file,
                    key_bytes.len()
                );
            }

            let mut key_array = [0u8; 32];
            key_array.copy_from_slice(&key_bytes);
            let signing_key = SigningKey::from_bytes(&key_array);

            identities.insert(name.to_string(), signing_key);
        }

        Ok(identities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_storage::MockStorage;

    #[test]
    fn test_create_identity() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        let key = manager.create_identity("test", false).unwrap();

        // Verify private key file exists
        let private_path = PathBuf::from("/faff/keys/id_test");
        assert!(storage.exists(&private_path));

        // Verify public key file exists
        let public_path = PathBuf::from("/faff/keys/id_test.pub");
        assert!(storage.exists(&public_path));

        // Verify the key can be read back
        let loaded_key = manager.get_identity("test").unwrap().unwrap();
        assert_eq!(key.to_bytes(), loaded_key.to_bytes());
    }

    #[test]
    fn test_create_identity_no_overwrite() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        manager.create_identity("test", false).unwrap();

        // Try to create again without overwrite flag
        let result = manager.create_identity("test", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_create_identity_with_overwrite() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        let key1 = manager.create_identity("test", false).unwrap();
        let key2 = manager.create_identity("test", true).unwrap();

        // Keys should be different
        assert_ne!(key1.to_bytes(), key2.to_bytes());
    }

    #[test]
    fn test_get_identity() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        let key = manager.create_identity("alice", false).unwrap();

        let loaded_key = manager.get_identity("alice").unwrap().unwrap();
        assert_eq!(key.to_bytes(), loaded_key.to_bytes());

        // Non-existent identity
        let result = manager.get_identity("bob").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_all_identities() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        let key1 = manager.create_identity("alice", false).unwrap();
        let key2 = manager.create_identity("bob", false).unwrap();

        let identities = manager.get().unwrap();
        assert_eq!(identities.len(), 2);
        assert_eq!(identities["alice"].to_bytes(), key1.to_bytes());
        assert_eq!(identities["bob"].to_bytes(), key2.to_bytes());
    }
}
