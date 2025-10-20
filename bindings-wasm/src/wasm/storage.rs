use js_sys::{Array, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// JavaScript storage interface that wasm bindings will call.
///
/// Implementers (e.g., Obsidian Vault wrapper) should provide async methods
/// for file operations.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "StorageAdapter")]
    pub type JsStorage;

    #[wasm_bindgen(structural, method, js_name = rootDir)]
    pub fn root_dir(this: &JsStorage) -> String;

    #[wasm_bindgen(structural, method, js_name = logDir)]
    pub fn log_dir(this: &JsStorage) -> String;

    #[wasm_bindgen(structural, method, js_name = planDir)]
    pub fn plan_dir(this: &JsStorage) -> String;

    #[wasm_bindgen(structural, method, js_name = identityDir)]
    pub fn identity_dir(this: &JsStorage) -> String;

    #[wasm_bindgen(structural, method, js_name = timesheetDir)]
    pub fn timesheet_dir(this: &JsStorage) -> String;

    #[wasm_bindgen(structural, method, js_name = configFile)]
    pub fn config_file(this: &JsStorage) -> String;

    /// Read file as bytes. Returns Promise<Uint8Array>.
    #[wasm_bindgen(structural, method, js_name = readBytes, catch)]
    pub fn read_bytes_promise(this: &JsStorage, path: &str) -> Result<js_sys::Promise, JsValue>;

    /// Read file as string. Returns Promise<string>.
    #[wasm_bindgen(structural, method, js_name = readString, catch)]
    pub fn read_string_promise(this: &JsStorage, path: &str) -> Result<js_sys::Promise, JsValue>;

    /// Write bytes to file. Returns Promise<void>.
    #[wasm_bindgen(structural, method, js_name = writeBytes, catch)]
    pub fn write_bytes_promise(
        this: &JsStorage,
        path: &str,
        data: &Uint8Array,
    ) -> Result<js_sys::Promise, JsValue>;

    /// Write string to file. Returns Promise<void>.
    #[wasm_bindgen(structural, method, js_name = writeString, catch)]
    pub fn write_string_promise(
        this: &JsStorage,
        path: &str,
        data: &str,
    ) -> Result<js_sys::Promise, JsValue>;

    /// Check if file exists (synchronous).
    #[wasm_bindgen(structural, method)]
    pub fn exists(this: &JsStorage, path: &str) -> bool;

    /// Create directory and all parent directories. Returns Promise<void>.
    #[wasm_bindgen(structural, method, js_name = createDirAll, catch)]
    pub fn create_dir_all_promise(this: &JsStorage, path: &str)
        -> Result<js_sys::Promise, JsValue>;

    /// List files matching pattern. Returns Promise<string[]>.
    #[wasm_bindgen(structural, method, js_name = listFiles, catch)]
    pub fn list_files_promise(
        this: &JsStorage,
        dir: &str,
        pattern: &str,
    ) -> Result<js_sys::Promise, JsValue>;
}

// Async helper methods for JsStorage
impl JsStorage {
    pub async fn read_bytes(&self, path: &str) -> Result<Vec<u8>, JsValue> {
        let promise = self.read_bytes_promise(path)?;
        let result = JsFuture::from(promise).await?;
        let array = Uint8Array::from(result);
        Ok(array.to_vec())
    }

    pub async fn read_string(&self, path: &str) -> Result<String, JsValue> {
        let promise = self.read_string_promise(path)?;
        let result = JsFuture::from(promise).await?;
        result
            .as_string()
            .ok_or_else(|| JsValue::from_str("Expected string result"))
    }

    pub async fn write_bytes(&self, path: &str, data: &[u8]) -> Result<(), JsValue> {
        let array = Uint8Array::from(data);
        let promise = self.write_bytes_promise(path, &array)?;
        JsFuture::from(promise).await?;
        Ok(())
    }

    pub async fn write_string(&self, path: &str, data: &str) -> Result<(), JsValue> {
        let promise = self.write_string_promise(path, data)?;
        JsFuture::from(promise).await?;
        Ok(())
    }

    pub async fn create_dir_all(&self, path: &str) -> Result<(), JsValue> {
        let promise = self.create_dir_all_promise(path)?;
        JsFuture::from(promise).await?;
        Ok(())
    }

    pub async fn list_files(&self, dir: &str, pattern: &str) -> Result<Vec<String>, JsValue> {
        let promise = self.list_files_promise(dir, pattern)?;
        let result = JsFuture::from(promise).await?;
        let array = Array::from(&result);

        let mut files = Vec::new();
        for i in 0..array.length() {
            if let Some(s) = array.get(i).as_string() {
                files.push(s);
            }
        }
        Ok(files)
    }
}
