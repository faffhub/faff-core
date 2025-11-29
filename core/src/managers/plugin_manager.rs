use anyhow::{Context, Result};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::models::remote::Remote;
use crate::storage::Storage;

/// A plugin entry: (plugin_path, plugin_instance)
type PluginEntry = (PathBuf, Py<PyAny>);
/// Cache of loaded plugins: plugin_name -> (path, instance)
type PluginCache = HashMap<String, PluginEntry>;

/// Manages loading and executing Python plugins
#[derive(Clone)]
pub struct PluginManager {
    storage: Arc<dyn Storage>,
    pub plugins_cache: Arc<Mutex<Option<PluginCache>>>,
}

impl PluginManager {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            storage,
            plugins_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the plugin directory path
    fn plugin_dir(&self) -> PathBuf {
        self.storage.plugins_dir()
    }

    /// Load all available plugins from the plugins directory
    ///
    /// Ensures plugins are loaded into the cache
    pub async fn load_plugins(&mut self) -> Result<()> {
        // Check if already loaded
        {
            let cache = self.plugins_cache.lock().unwrap();
            if cache.is_some() {
                return Ok(());
            }
        }

        let plugin_dir = self.plugin_dir();
        let mut plugins = HashMap::new();

        // Ensure plugin directory exists
        if !self.storage.exists(&plugin_dir) {
            let mut cache = self.plugins_cache.lock().unwrap();
            *cache = Some(plugins);
            return Ok(());
        }

        // List all plugin directories
        let plugin_dirs = self
            .storage
            .list_files(&plugin_dir, "*")
            .await
            .context("Failed to list plugin directories")?;

        Python::attach(|py| -> PyResult<()> {
            // Import the base Plugin classes from faff_core.plugins
            let faff_plugins = py.import("faff_core.plugins")?;
            let plan_source_cls = faff_plugins.getattr("PlanSource")?;
            let audience_cls = faff_plugins.getattr("Audience")?;

            for plugin_candidate in plugin_dirs {
                // Skip non-directories and hidden directories
                if !plugin_candidate.is_dir() {
                    continue;
                }
                let dir_name = plugin_candidate
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| {
                        pyo3::exceptions::PyValueError::new_err("Invalid plugin directory name")
                    })?;

                if dir_name.starts_with('.') {
                    continue;
                }

                // Check for standardized entry point: plugin/plugin.py
                let plugin_file = plugin_candidate.join("plugin").join("plugin.py");
                if !plugin_file.exists() {
                    // Skip directories without the standard entry point
                    continue;
                }

                // Use directory name as the plugin identifier
                let plugin_name = dir_name;

                // Load the module using importlib
                let importlib = py.import("importlib.util")?;
                let spec = importlib.call_method1(
                    "spec_from_file_location",
                    (plugin_name, plugin_file.to_str()),
                )?;

                if spec.is_none() {
                    continue;
                }

                let module = importlib.call_method1("module_from_spec", (&spec,))?;
                let loader = spec.getattr("loader")?;
                loader.call_method1("exec_module", (&module,))?;

                // Find all classes that are subclasses of PlanSource or Audience
                let module_dict_attr = module.getattr("__dict__")?;
                let module_dict = module_dict_attr.downcast::<PyDict>()?;

                for (_attr_name, attr_value) in module_dict.iter() {
                    // Check if it's a type/class
                    if !attr_value.hasattr("__bases__")? {
                        continue;
                    }

                    // Check if it's a subclass using Python's issubclass
                    let builtins = py.import("builtins")?;

                    // Try to check if it's a subclass - if this fails (e.g. not a class), skip it
                    let is_plan_source: bool = match builtins
                        .call_method1("issubclass", (&attr_value, &plan_source_cls))
                    {
                        Ok(result) => result.extract().unwrap_or(false),
                        Err(_) => {
                            // Not a class or other error, skip this attribute
                            continue;
                        }
                    };
                    let is_audience: bool =
                        match builtins.call_method1("issubclass", (&attr_value, &audience_cls)) {
                            Ok(result) => result.extract().unwrap_or(false),
                            Err(_) => false, // Already checked if it's a class above
                        };

                    if !is_plan_source && !is_audience {
                        continue;
                    }

                    // Check if it's abstract (skip abstract classes)
                    let inspect = py.import("inspect")?;
                    let is_abstract: bool = inspect
                        .call_method1("isabstract", (&attr_value,))?
                        .extract()?;

                    if is_abstract {
                        continue;
                    }

                    // This is a concrete plugin class - store both the directory path and the class
                    // Use directory name as the plugin identifier
                    plugins.insert(
                        plugin_name.to_string(),
                        (plugin_file.clone(), attr_value.into()),
                    );
                }
            }

            Ok(())
        })
        .map_err(|e: PyErr| anyhow::anyhow!("Python error: {}", e))?;

        let mut cache = self.plugins_cache.lock().unwrap();
        *cache = Some(plugins);
        Ok(())
    }

    /// Instantiate a plugin with the given config
    ///
    /// Returns a Python object instance of the plugin
    pub async fn instantiate_plugin(
        &mut self,
        plugin_name: &str,
        instance_name: &str,
        config: HashMap<String, toml::Value>,
        defaults: HashMap<String, toml::Value>,
    ) -> Result<Py<PyAny>> {
        // Ensure plugins are loaded
        self.load_plugins().await?;

        // Verify plugin exists and get its file path
        let plugin_file_path = {
            let cache = self.plugins_cache.lock().unwrap();
            let plugins = cache
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Plugins not loaded"))?;

            let (file_path, _) = plugins
                .get(plugin_name)
                .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;

            file_path.clone()
        }; // Lock released here

        // Get paths needed for plugin instantiation (can now access self.storage)
        let state_path = self.storage.plugin_state_dir().join(instance_name);

        // Ensure state directory exists
        self.storage
            .create_dir_all(&state_path)
            .await
            .context("Failed to create plugin state directory")?;

        // Get the plugin class inside Python::attach to avoid borrowing issues
        let plugin_name_owned = plugin_name.to_string();
        let instance_name_owned = instance_name.to_string();
        let state_path_str = state_path.to_str().unwrap().to_string();
        let plugin_file_str = plugin_file_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid plugin file path"))?
            .to_string();

        Python::attach(move |py| -> PyResult<Py<PyAny>> {
            // Re-import to get the plugin class (avoids lifetime issues)
            let importlib = py.import("importlib.util")?;

            let spec = importlib.call_method1(
                "spec_from_file_location",
                (&plugin_name_owned, &plugin_file_str),
            )?;
            let module = importlib.call_method1("module_from_spec", (&spec,))?;
            let loader = spec.getattr("loader")?;
            loader.call_method1("exec_module", (&module,))?;

            // Find the plugin class in the module
            let module_dict_attr = module.getattr("__dict__")?;
            let module_dict = module_dict_attr.downcast::<PyDict>()?;
            let faff_plugins = py.import("faff_core.plugins")?;
            let plan_source_cls = faff_plugins.getattr("PlanSource")?;
            let audience_cls = faff_plugins.getattr("Audience")?;

            let mut plugin_class = None;
            for (_attr_name, attr_value) in module_dict.iter() {
                if !attr_value.hasattr("__bases__")? {
                    continue;
                }

                // Check if it's a subclass using Python's issubclass
                let builtins = py.import("builtins")?;
                let is_plan_source: bool = builtins
                    .call_method1("issubclass", (&attr_value, &plan_source_cls))?
                    .extract()?;
                let is_audience: bool = builtins
                    .call_method1("issubclass", (&attr_value, &audience_cls))?
                    .extract()?;
                let is_subclass = is_plan_source || is_audience;
                if !is_subclass {
                    continue;
                }

                let inspect = py.import("inspect")?;
                let is_abstract: bool = inspect
                    .call_method1("isabstract", (&attr_value,))?
                    .extract()?;
                if !is_abstract {
                    plugin_class = Some(attr_value);
                    break;
                }
            }

            let plugin_class = plugin_class.ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "No concrete plugin class found in {plugin_name_owned}"
                ))
            })?;

            // Convert config and defaults to Python dicts
            let py_config = pythonize::pythonize(py, &config)?;
            let py_defaults = pythonize::pythonize(py, &defaults)?;

            // Convert state_path to Python Path object
            let pathlib = py.import("pathlib")?;
            let path_cls = pathlib.getattr("Path")?;
            let py_state_path = path_cls.call1((&state_path_str,))?;

            // Instantiate the plugin
            let instance = plugin_class.call1((
                &plugin_name_owned,
                &instance_name_owned,
                py_config,
                py_defaults,
                py_state_path,
            ))?;

            Ok(instance.into())
        })
        .map_err(|e: PyErr| anyhow::anyhow!("Failed to instantiate plugin: {}", e))
    }

    /// Get instantiated plan remote plugins from remotes directory
    pub async fn plan_remotes(&mut self) -> Result<Vec<Py<PyAny>>> {
        self.load_plugins().await?;

        // List all remote config files
        let remotes_dir = self.storage.remotes_dir();
        let remote_files = self
            .storage
            .list_files(&remotes_dir, "*.toml")
            .await
            .context("Failed to list remote config files")?;

        let mut instances = Vec::new();
        for remote_file in remote_files {
            // Load remote config from file
            let remote_toml = self
                .storage
                .read_string(&remote_file)
                .await
                .with_context(|| format!("Failed to read remote file: {remote_file:?}"))?;

            let remote = Remote::from_toml(&remote_toml)
                .with_context(|| format!("Failed to parse remote config: {remote_file:?}"))?;

            // Convert RemoteVocabulary to HashMap for plugin
            let mut defaults = HashMap::new();
            if !remote.vocabulary.roles.is_empty() {
                defaults.insert(
                    "roles".to_string(),
                    toml::Value::Array(
                        remote
                            .vocabulary
                            .roles
                            .iter()
                            .map(|s| toml::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }
            if !remote.vocabulary.objectives.is_empty() {
                defaults.insert(
                    "objectives".to_string(),
                    toml::Value::Array(
                        remote
                            .vocabulary
                            .objectives
                            .iter()
                            .map(|s| toml::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }
            if !remote.vocabulary.actions.is_empty() {
                defaults.insert(
                    "actions".to_string(),
                    toml::Value::Array(
                        remote
                            .vocabulary
                            .actions
                            .iter()
                            .map(|s| toml::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }
            if !remote.vocabulary.subjects.is_empty() {
                defaults.insert(
                    "subjects".to_string(),
                    toml::Value::Array(
                        remote
                            .vocabulary
                            .subjects
                            .iter()
                            .map(|s| toml::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }

            let instance = self
                .instantiate_plugin(&remote.plugin, &remote.id, remote.connection, defaults)
                .await
                .with_context(|| {
                    format!("Failed to instantiate plan remote plugin '{}'", remote.id)
                })?;
            instances.push(instance);
        }

        Ok(instances)
    }

    /// Get remote configurations without instantiating plugins
    ///
    /// Returns the raw Remote configuration objects for all configured remotes.
    /// Use this to access remote metadata like connection settings, vocabulary, etc.
    ///
    /// # Returns
    /// Vector of Remote configuration objects
    pub async fn get_remote_configs(&self) -> Result<Vec<Remote>> {
        let remotes_dir = self.storage.remotes_dir();
        let remote_files = self
            .storage
            .list_files(&remotes_dir, "*.toml")
            .await
            .context("Failed to list remote config files")?;

        let mut configs = Vec::new();
        for remote_file in remote_files {
            let remote_toml = self
                .storage
                .read_string(&remote_file)
                .await
                .with_context(|| format!("Failed to read remote file: {remote_file:?}"))?;

            let remote = Remote::from_toml(&remote_toml)
                .with_context(|| format!("Failed to parse remote config: {remote_file:?}"))?;

            configs.push(remote);
        }

        Ok(configs)
    }

    /// Get instantiated audience plugins from remotes directory
    ///
    /// TODO: For now this returns all remotes. In the future we may want to
    /// filter remotes that are specifically configured for timesheet audiences.
    pub async fn audiences(&mut self) -> Result<Vec<Py<PyAny>>> {
        // For now, audiences are the same as plan remotes
        // A plugin can implement both PlanRemote and TimesheetAudience interfaces
        self.plan_remotes().await
    }

    /// Get a specific audience plugin by ID
    ///
    /// This searches through all configured audience plugins and returns the one
    /// matching the given ID, or None if not found.
    pub async fn get_audience_by_id(&mut self, audience_id: &str) -> Result<Option<Py<PyAny>>> {
        let audiences = self.audiences().await?;

        Python::attach(|py| -> PyResult<Option<Py<PyAny>>> {
            for audience in audiences {
                // Get the 'id' attribute from the plugin instance
                let id: String = match audience.getattr(py, "id") {
                    Ok(id_attr) => match id_attr.extract(py) {
                        Ok(id) => id,
                        Err(_) => continue, // Skip if can't extract ID
                    },
                    Err(_) => continue, // Skip if no 'id' attribute
                };

                if id == audience_id {
                    return Ok(Some(audience));
                }
            }
            Ok(None)
        })
        .map_err(|e: PyErr| anyhow::anyhow!("Failed to get audience by ID: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::path::Path;
    use std::sync::Mutex;

    struct MockStorage {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
        root: PathBuf,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                files: Mutex::new(HashMap::new()),
                root: PathBuf::from("/test"),
            }
        }
    }

    #[async_trait]
    impl Storage for MockStorage {
        fn base_dir(&self) -> PathBuf {
            self.root.join(".faff")
        }
        async fn read_string(&self, path: &Path) -> Result<String> {
            let bytes = self.read_bytes(path).await?;
            Ok(String::from_utf8(bytes)?)
        }
        async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found"))
        }
        async fn write_string(&self, path: &Path, data: &str) -> Result<()> {
            self.write_bytes(path, data.as_bytes()).await
        }
        async fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()> {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), data.to_vec());
            Ok(())
        }
        async fn delete(&self, path: &Path) -> Result<()> {
            let mut files = self.files.lock().unwrap();
            if files.remove(path).is_some() {
                Ok(())
            } else {
                anyhow::bail!("File not found: {:?}", path)
            }
        }
        fn exists(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }
        async fn create_dir_all(&self, _path: &Path) -> Result<()> {
            Ok(())
        }
        async fn list_files(&self, _dir: &Path, _pattern: &str) -> Result<Vec<PathBuf>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let storage = Arc::new(MockStorage::new());
        let mut manager = PluginManager::new(storage);

        // Should load successfully even when no files exist
        manager.load_plugins().await.unwrap();
    }
}
