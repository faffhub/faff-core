use anyhow::{Context, Result};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::models::log::Log;
use crate::models::plan::Plan;
use crate::models::timesheet::Timesheet;
use crate::models::Config;
use crate::storage::Storage;

/// Manages loading and executing Python plugins
#[derive(Clone)]
pub struct PluginManager {
    storage: Arc<dyn Storage>,
    config: Config,
    pub plugins_cache: Arc<Mutex<Option<HashMap<String, Py<PyAny>>>>>,
}

impl PluginManager {
    pub fn new(storage: Arc<dyn Storage>, config: Config) -> Self {
        Self {
            storage,
            config,
            plugins_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the plugin directory path
    fn plugin_dir(&self) -> PathBuf {
        self.storage.root_dir().join(".faff").join("plugins")
    }

    /// Load all available plugins from the plugins directory
    ///
    /// Ensures plugins are loaded into the cache
    pub fn load_plugins(&mut self) -> Result<()> {
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

        // List all .py files in the plugin directory
        let pattern = "*.py";
        let plugin_files = self
            .storage
            .list_files(&plugin_dir, pattern)
            .context("Failed to list plugin files")?;

        Python::attach(|py| -> PyResult<()> {
            // Import the base Plugin classes from faff_core.plugins
            let faff_plugins = py.import("faff_core.plugins")?;
            let plan_source_cls = faff_plugins.getattr("PlanSource")?;
            let audience_cls = faff_plugins.getattr("Audience")?;

            for plugin_file in plugin_files {
                let filename = plugin_file
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| {
                        pyo3::exceptions::PyValueError::new_err("Invalid plugin filename")
                    })?;

                // Skip __init__.py
                if filename == "__init__.py" {
                    continue;
                }

                let module_name = plugin_file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| {
                        pyo3::exceptions::PyValueError::new_err("Invalid module name")
                    })?;

                // Load the module using importlib
                let importlib = py.import("importlib.util")?;
                let spec = importlib.call_method1(
                    "spec_from_file_location",
                    (module_name, plugin_file.to_str()),
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

                    // This is a concrete plugin class
                    plugins.insert(module_name.to_string(), attr_value.into());
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
    pub fn instantiate_plugin(
        &mut self,
        plugin_name: &str,
        instance_name: &str,
        config: HashMap<String, toml::Value>,
        defaults: HashMap<String, toml::Value>,
    ) -> Result<Py<PyAny>> {
        // Ensure plugins are loaded
        self.load_plugins()?;

        // Verify plugin exists
        {
            let cache = self.plugins_cache.lock().unwrap();
            let plugins = cache.as_ref().ok_or_else(|| anyhow::anyhow!("Plugins not loaded"))?;
            if !plugins.contains_key(plugin_name) {
                return Err(anyhow::anyhow!("Plugin '{}' not found", plugin_name));
            }
        } // Lock released here

        // Get paths needed for plugin instantiation (can now access self.storage)
        let root_dir = self.storage.root_dir();
        let state_path = root_dir
            .join(".faff")
            .join("plugin_state")
            .join(instance_name);

        // Ensure state directory exists
        self.storage
            .create_dir_all(&state_path)
            .context("Failed to create plugin state directory")?;

        // Get the plugin class inside Python::attach to avoid borrowing issues
        let plugin_name_owned = plugin_name.to_string();
        let instance_name_owned = instance_name.to_string();
        let state_path_str = state_path.to_str().unwrap().to_string();

        Python::attach(move |py| -> PyResult<Py<PyAny>> {
            // Re-import to get the plugin class (avoids lifetime issues)
            let importlib = py.import("importlib.util")?;
            let root_py = py
                .import("pathlib")?
                .call_method0("Path")?
                .call_method1("__truediv__", (root_dir.to_str().unwrap(),))?;
            let faff_dir = root_py.call_method1("__truediv__", (".faff",))?;
            let plugins_dir = faff_dir.call_method1("__truediv__", ("plugins",))?;
            let plugin_file =
                plugins_dir.call_method1("__truediv__", (format!("{}.py", plugin_name_owned),))?;

            let spec = importlib
                .call_method1("spec_from_file_location", (&plugin_name_owned, plugin_file))?;
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
                    "No concrete plugin class found in {}",
                    plugin_name_owned
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

    /// Get instantiated plan remote plugins based on config
    pub fn plan_remotes(&mut self) -> Result<Vec<Py<PyAny>>> {
        self.load_plugins()?;

        // Clone the config values we need to avoid borrow checker issues
        let plan_remotes = self.config.plan_remote.clone();

        let mut instances = Vec::new();
        for plan_remote in &plan_remotes {
            // Convert PlanDefaults to HashMap<String, toml::Value>
            // FIXME: This is a temporary solution - we should properly serialize PlanDefaults
            let defaults = HashMap::new();

            let instance = self
                .instantiate_plugin(
                    &plan_remote.plugin,
                    &plan_remote.name,
                    plan_remote.config.clone(),
                    defaults,
                )
                .with_context(|| {
                    format!(
                        "Failed to instantiate plan remote plugin '{}'",
                        plan_remote.name
                    )
                })?;
            instances.push(instance);
        }

        Ok(instances)
    }

    /// Get instantiated audience plugins based on config
    pub fn audiences(&mut self) -> Result<Vec<Py<PyAny>>> {
        self.load_plugins()?;

        // Clone the config values we need to avoid borrow checker issues
        let audiences = self.config.timesheet_audience.clone();

        let mut instances = Vec::new();
        for audience in &audiences {
            // TimesheetAudience doesn't have defaults
            let defaults = HashMap::new();

            let instance = self
                .instantiate_plugin(
                    &audience.plugin,
                    &audience.name,
                    audience.config.clone(),
                    defaults,
                )
                .with_context(|| format!("Failed to instantiate audience plugin '{}'", audience.name))?;
            instances.push(instance);
        }

        Ok(instances)
    }

    /// Get a specific audience plugin by ID
    ///
    /// This searches through all configured audience plugins and returns the one
    /// matching the given ID, or None if not found.
    pub fn get_audience_by_id(&mut self, audience_id: &str) -> Result<Option<Py<PyAny>>> {
        let audiences = self.audiences()?;

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

/// A PlanSource plugin instance
pub struct PlanSourcePlugin {
    instance: Py<PyAny>,
}

impl PlanSourcePlugin {
    pub fn new(instance: Py<PyAny>) -> Self {
        Self { instance }
    }

    /// Pull a plan for the given date
    pub fn pull_plan(&self, date: chrono::NaiveDate) -> Result<Plan> {
        Python::attach(|py| -> PyResult<Plan> {
            // Convert date to Python date using type_mapping
            let py_date = crate::type_mapping::date_rust_to_py(py, &date)?;

            // Call the pull_plan method
            let result = self.instance.call_method1(py, "pull_plan", (py_date,))?;

            // The result should be a PyPlan object
            // Extract the inner field which contains the Rust Plan
            use crate::py_models::plan::PyPlan;
            let pyplan: PyRef<PyPlan> = result.extract(py)?;
            let rust_plan = pyplan.inner.clone();

            Ok(rust_plan)
        })
        .map_err(|e: PyErr| anyhow::anyhow!("Failed to pull plan: {}", e))
    }
}

/// An Audience plugin instance
pub struct AudiencePlugin {
    instance: Py<PyAny>,
}

impl AudiencePlugin {
    pub fn new(instance: Py<PyAny>) -> Self {
        Self { instance }
    }

    /// Compile a timesheet for the given log
    pub fn compile_timesheet(&self, log: &Log) -> Result<Timesheet> {
        Python::attach(|py| -> PyResult<Timesheet> {
            // Create a PyLog wrapper around the Rust Log
            use crate::py_models::log::PyLog;
            let pylog = Py::new(py, PyLog { inner: log.clone() })?;

            // Call the compile_time_sheet method
            let result = self
                .instance
                .call_method1(py, "compile_time_sheet", (pylog,))?;

            // The result should be a PyTimesheet object
            use crate::py_models::timesheet::PyTimesheet;
            let pytimesheet: PyRef<PyTimesheet> = result.extract(py)?;
            let rust_timesheet = pytimesheet.inner.clone();

            Ok(rust_timesheet)
        })
        .map_err(|e: PyErr| anyhow::anyhow!("Failed to compile timesheet: {}", e))
    }

    /// Submit a timesheet
    pub fn submit_timesheet(&self, timesheet: &Timesheet) -> Result<()> {
        Python::attach(|py| -> PyResult<()> {
            // Create a PyTimesheet wrapper around the Rust Timesheet
            use crate::py_models::timesheet::PyTimesheet;
            let pytimesheet = Py::new(
                py,
                PyTimesheet {
                    inner: timesheet.clone(),
                },
            )?;

            // Call the submit_timesheet method
            self.instance
                .call_method1(py, "submit_timesheet", (pytimesheet,))?;

            Ok(())
        })
        .map_err(|e: PyErr| anyhow::anyhow!("Failed to submit timesheet: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    impl Storage for MockStorage {
        fn root_dir(&self) -> PathBuf {
            self.root.clone()
        }
        fn log_dir(&self) -> PathBuf {
            self.root.join("logs")
        }
        fn plan_dir(&self) -> PathBuf {
            self.root.join("plans")
        }
        fn identity_dir(&self) -> PathBuf {
            self.root.join("identities")
        }
        fn timesheet_dir(&self) -> PathBuf {
            self.root.join("timesheets")
        }
        fn config_file(&self) -> PathBuf {
            self.root.join("config.toml")
        }
        fn read_string(&self, path: &PathBuf) -> Result<String> {
            let bytes = self.read_bytes(path)?;
            Ok(String::from_utf8(bytes)?)
        }
        fn read_bytes(&self, path: &PathBuf) -> Result<Vec<u8>> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found"))
        }
        fn write_string(&self, path: &PathBuf, data: &str) -> Result<()> {
            self.write_bytes(path, data.as_bytes())
        }
        fn write_bytes(&self, path: &PathBuf, data: &[u8]) -> Result<()> {
            self.files
                .lock()
                .unwrap()
                .insert(path.clone(), data.to_vec());
            Ok(())
        }
        fn delete(&self, path: &PathBuf) -> Result<()> {
            let mut files = self.files.lock().unwrap();
            if files.remove(path).is_some() {
                Ok(())
            } else {
                anyhow::bail!("File not found: {:?}", path)
            }
        }
        fn exists(&self, path: &PathBuf) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }
        fn create_dir_all(&self, _path: &PathBuf) -> Result<()> {
            Ok(())
        }
        fn list_files(&self, _dir: &PathBuf, _pattern: &str) -> Result<Vec<PathBuf>> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_plugin_manager_creation() {
        let storage = Arc::new(MockStorage::new());
        let config = crate::models::Config {
            timezone: chrono_tz::America::New_York,
            plan_remote: vec![],
            timesheet_audience: vec![],
            role: vec![],
        };
        let mut manager = PluginManager::new(storage, config);

        // Should load successfully even when no files exist
        manager.load_plugins().unwrap();
    }
}
