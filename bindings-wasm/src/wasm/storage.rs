use anyhow::{Context, Result};
use async_trait::async_trait;
use js_sys::{Array, Uint8Array};
use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use faff_core::storage::Storage;

/// JavaScript storage interface for wasm bindings.
///
/// This is the external interface that JavaScript must implement.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "StorageAdapter")]
    pub type JsStorageAdapter;

    #[wasm_bindgen(structural, method, js_name = baseDir)]
    pub fn base_dir(this: &JsStorageAdapter) -> String;

    /// Read file as bytes. Returns Promise<Uint8Array>.
    #[wasm_bindgen(structural, method, js_name = readBytes, catch)]
    pub fn read_bytes_promise(
        this: &JsStorageAdapter,
        path: &str,
    ) -> Result<js_sys::Promise, JsValue>;

    /// Read file as string. Returns Promise<string>.
    #[wasm_bindgen(structural, method, js_name = readString, catch)]
    pub fn read_string_promise(
        this: &JsStorageAdapter,
        path: &str,
    ) -> Result<js_sys::Promise, JsValue>;

    /// Write bytes to file. Returns Promise<void>.
    #[wasm_bindgen(structural, method, js_name = writeBytes, catch)]
    pub fn write_bytes_promise(
        this: &JsStorageAdapter,
        path: &str,
        data: &Uint8Array,
    ) -> Result<js_sys::Promise, JsValue>;

    /// Write string to file. Returns Promise<void>.
    #[wasm_bindgen(structural, method, js_name = writeString, catch)]
    pub fn write_string_promise(
        this: &JsStorageAdapter,
        path: &str,
        data: &str,
    ) -> Result<js_sys::Promise, JsValue>;

    /// Delete file. Returns Promise<void>.
    #[wasm_bindgen(structural, method, js_name = delete, catch)]
    pub fn delete_promise(this: &JsStorageAdapter, path: &str) -> Result<js_sys::Promise, JsValue>;

    /// Check if file exists (synchronous).
    #[wasm_bindgen(structural, method)]
    pub fn exists(this: &JsStorageAdapter, path: &str) -> bool;

    /// Create directory and all parent directories. Returns Promise<void>.
    #[wasm_bindgen(structural, method, js_name = createDirAll, catch)]
    pub fn create_dir_all_promise(
        this: &JsStorageAdapter,
        path: &str,
    ) -> Result<js_sys::Promise, JsValue>;

    /// List files matching pattern. Returns Promise<string[]>.
    #[wasm_bindgen(structural, method, js_name = listFiles, catch)]
    pub fn list_files_promise(
        this: &JsStorageAdapter,
        dir: &str,
        pattern: &str,
    ) -> Result<js_sys::Promise, JsValue>;
}

/// Wrapper that implements Storage trait for JavaScript storage adapter.
///
/// This bridges JavaScript async storage to Rust's async Storage trait.
/// JavaScript methods return Promises which we await using wasm-bindgen-futures.
pub struct JsStorage {
    adapter: JsStorageAdapter,
}

impl JsStorage {
    pub fn new(adapter: JsStorageAdapter) -> Self {
        Self { adapter }
    }
}

#[async_trait(?Send)]
impl Storage for JsStorage {
    fn base_dir(&self) -> PathBuf {
        PathBuf::from(self.adapter.base_dir())
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        let path_str = path.to_str().context("Path contains invalid UTF-8")?;

        let promise = self
            .adapter
            .read_bytes_promise(path_str)
            .map_err(|e| anyhow::anyhow!("Failed to call readBytes: {:?}", e))?;

        let result = JsFuture::from(promise)
            .await
            .map_err(|e| anyhow::anyhow!("readBytes promise rejected: {:?}", e))?;

        let array = Uint8Array::from(result);
        Ok(array.to_vec())
    }

    async fn read_string(&self, path: &Path) -> Result<String> {
        let path_str = path.to_str().context("Path contains invalid UTF-8")?;

        let promise = self
            .adapter
            .read_string_promise(path_str)
            .map_err(|e| anyhow::anyhow!("Failed to call readString: {:?}", e))?;

        let result = JsFuture::from(promise)
            .await
            .map_err(|e| anyhow::anyhow!("readString promise rejected: {:?}", e))?;

        result
            .as_string()
            .ok_or_else(|| anyhow::anyhow!("readString did not return a string"))
    }

    async fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()> {
        let path_str = path.to_str().context("Path contains invalid UTF-8")?;

        let array = Uint8Array::from(data);
        let promise = self
            .adapter
            .write_bytes_promise(path_str, &array)
            .map_err(|e| anyhow::anyhow!("Failed to call writeBytes: {:?}", e))?;

        JsFuture::from(promise)
            .await
            .map_err(|e| anyhow::anyhow!("writeBytes promise rejected: {:?}", e))?;

        Ok(())
    }

    async fn write_string(&self, path: &Path, data: &str) -> Result<()> {
        let path_str = path.to_str().context("Path contains invalid UTF-8")?;

        let promise = self
            .adapter
            .write_string_promise(path_str, data)
            .map_err(|e| anyhow::anyhow!("Failed to call writeString: {:?}", e))?;

        JsFuture::from(promise)
            .await
            .map_err(|e| anyhow::anyhow!("writeString promise rejected: {:?}", e))?;

        Ok(())
    }

    async fn delete(&self, path: &Path) -> Result<()> {
        let path_str = path.to_str().context("Path contains invalid UTF-8")?;

        let promise = self
            .adapter
            .delete_promise(path_str)
            .map_err(|e| anyhow::anyhow!("Failed to call delete: {:?}", e))?;

        JsFuture::from(promise)
            .await
            .map_err(|e| anyhow::anyhow!("delete promise rejected: {:?}", e))?;

        Ok(())
    }

    fn exists(&self, path: &Path) -> bool {
        let path_str = path.to_str().unwrap_or("");
        self.adapter.exists(path_str)
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        let path_str = path.to_str().context("Path contains invalid UTF-8")?;

        let promise = self
            .adapter
            .create_dir_all_promise(path_str)
            .map_err(|e| anyhow::anyhow!("Failed to call createDirAll: {:?}", e))?;

        JsFuture::from(promise)
            .await
            .map_err(|e| anyhow::anyhow!("createDirAll promise rejected: {:?}", e))?;

        Ok(())
    }

    async fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        let dir_str = dir.to_str().context("Path contains invalid UTF-8")?;

        let promise = self
            .adapter
            .list_files_promise(dir_str, pattern)
            .map_err(|e| anyhow::anyhow!("Failed to call listFiles: {:?}", e))?;

        let result = JsFuture::from(promise)
            .await
            .map_err(|e| anyhow::anyhow!("listFiles promise rejected: {:?}", e))?;

        let array = Array::from(&result);
        let mut files = Vec::new();

        for i in 0..array.length() {
            if let Some(s) = array.get(i).as_string() {
                files.push(PathBuf::from(s));
            }
        }

        Ok(files)
    }
}
