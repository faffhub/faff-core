use crate::storage::Storage;
use anyhow::{Context, Result};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Manages Ed25519 identity keypairs for signing timesheets
#[derive(Clone)]
pub struct IdentityManager {
    storage: Arc<dyn Storage>,
}

impl IdentityManager {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// Get the path for a private key file
    fn get_key_path(&self, name: &str) -> PathBuf {
        self.storage.identity_dir().join(format!("id_{name}"))
    }

    /// Get the path for a public key file
    fn get_pub_path(&self, name: &str) -> PathBuf {
        self.storage.identity_dir().join(format!("id_{name}.pub"))
    }

    /// Create a new Ed25519 identity keypair
    ///
    /// Keys are stored as base64-encoded strings:
    /// - Private key: ~/.faff/identities/id_{name}
    /// - Public key: ~/.faff/identities/id_{name}.pub
    pub async fn create_identity(&self, name: &str, overwrite: bool) -> Result<SigningKey> {
        let private_path = self.get_key_path(name);
        let public_path = self.get_pub_path(name);

        if !overwrite && self.storage.exists(&private_path) {
            anyhow::bail!("Identity '{}' already exists", name);
        }

        // Ensure identity directory exists
        let identity_dir = self.storage.identity_dir();
        self.storage
            .create_dir_all(&identity_dir)
            .await
            .context("Failed to create identity directory")?;

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
        self.storage
            .write_string(&private_path, &b64_private)
            .await
            .with_context(|| format!("Failed to write private key for identity '{name}'"))?;
        self.storage
            .write_string(&public_path, &b64_public)
            .await
            .with_context(|| format!("Failed to write public key for identity '{name}'"))?;

        // Note: File permissions (chmod 0o600) should be handled by the Storage implementation
        // if it's a real filesystem. For testing with mock storage, this is skipped.

        Ok(signing_key)
    }

    /// Check if an identity exists
    pub fn identity_exists(&self, name: &str) -> bool {
        self.storage.exists(&self.get_key_path(name))
    }

    /// Delete an identity
    ///
    /// Removes both the private and public key files
    pub async fn delete_identity(&self, name: &str) -> Result<()> {
        let private_path = self.get_key_path(name);
        let public_path = self.get_pub_path(name);

        if !self.storage.exists(&private_path) {
            anyhow::bail!("Identity '{}' does not exist", name);
        }

        // Delete private key
        self.storage
            .delete(&private_path)
            .await
            .with_context(|| format!("Failed to delete private key for identity '{name}'"))?;

        // Delete public key if it exists
        if self.storage.exists(&public_path) {
            self.storage
                .delete(&public_path)
                .await
                .with_context(|| format!("Failed to delete public key for identity '{name}'"))?;
        }

        Ok(())
    }

    /// Get a specific identity by name
    pub async fn get_identity(&self, name: &str) -> Result<Option<SigningKey>> {
        let identities = self.list_identities().await?;
        Ok(identities.get(name).cloned())
    }

    /// List all identities
    ///
    /// Returns a HashMap where keys are identity names and values are SigningKeys
    pub async fn list_identities(&self) -> Result<HashMap<String, SigningKey>> {
        let identity_dir = self.storage.identity_dir();
        let mut identities = HashMap::new();

        // List all files matching "id_*" pattern
        let files = self
            .storage
            .list_files(&identity_dir, "id_*")
            .await
            .context("Failed to list identity files")?;

        for file in files {
            // Skip public key files
            if file.extension().and_then(|s| s.to_str()) == Some("pub") {
                continue;
            }

            // Extract identity name (remove "id_" prefix)
            let filename = file
                .file_name()
                .and_then(|s| s.to_str())
                .with_context(|| format!("Invalid filename in identity directory: {file:?}"))?;

            if !filename.starts_with("id_") {
                continue;
            }

            let name = &filename[3..]; // Remove "id_" prefix

            // Read and decode the private key
            let b64_private = self
                .storage
                .read_string(&file)
                .await
                .with_context(|| format!("Failed to read identity file '{name}'"))?;

            let key_bytes = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                b64_private.trim(),
            )
            .with_context(|| format!("Failed to decode base64 key for identity '{name}'"))?;

            if key_bytes.len() != 32 {
                anyhow::bail!(
                    "Invalid key length for identity '{}': expected 32 bytes, got {}",
                    name,
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
    use crate::utils::test_utils::mock_storage::MockStorage;

    #[tokio::test]
    async fn test_create_identity() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        let key = manager.create_identity("test", false).await.unwrap();

        // Verify private key file exists
        let private_path = PathBuf::from("/faff/keys/id_test");
        assert!(storage.exists(&private_path));

        // Verify public key file exists
        let public_path = PathBuf::from("/faff/keys/id_test.pub");
        assert!(storage.exists(&public_path));

        // Verify the key can be read back
        let loaded_key = manager.get_identity("test").await.unwrap().unwrap();
        assert_eq!(key.to_bytes(), loaded_key.to_bytes());
    }

    #[tokio::test]
    async fn test_create_identity_no_overwrite() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        manager.create_identity("test", false).await.unwrap();

        // Try to create again without overwrite flag
        let result = manager.create_identity("test", false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_create_identity_with_overwrite() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        let key1 = manager.create_identity("test", false).await.unwrap();
        let key2 = manager.create_identity("test", true).await.unwrap();

        // Keys should be different
        assert_ne!(key1.to_bytes(), key2.to_bytes());
    }

    #[tokio::test]
    async fn test_get_identity() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        let key = manager.create_identity("alice", false).await.unwrap();

        let loaded_key = manager.get_identity("alice").await.unwrap().unwrap();
        assert_eq!(key.to_bytes(), loaded_key.to_bytes());

        // Non-existent identity
        let result = manager.get_identity("bob").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_identities() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        let key1 = manager.create_identity("alice", false).await.unwrap();
        let key2 = manager.create_identity("bob", false).await.unwrap();

        let identities = manager.list_identities().await.unwrap();
        assert_eq!(identities.len(), 2);
        assert_eq!(identities["alice"].to_bytes(), key1.to_bytes());
        assert_eq!(identities["bob"].to_bytes(), key2.to_bytes());
    }

    #[tokio::test]
    async fn test_identity_exists() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        assert!(!manager.identity_exists("test"));

        manager.create_identity("test", false).await.unwrap();

        assert!(manager.identity_exists("test"));
    }

    #[tokio::test]
    async fn test_delete_identity() {
        let storage = Arc::new(MockStorage::new());
        let manager = IdentityManager::new(storage.clone());

        manager.create_identity("test", false).await.unwrap();
        assert!(manager.identity_exists("test"));

        manager.delete_identity("test").await.unwrap();
        assert!(!manager.identity_exists("test"));

        // Try to delete non-existent identity
        let result = manager.delete_identity("nonexistent").await;
        assert!(result.is_err());
    }
}
