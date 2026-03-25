//! Plugin trait definitions for importer plugins.
//!
//! This module defines the `ImporterPlugin` trait that all source-specific
//! importers must implement. Each plugin is responsible for:
//! - Detecting and parsing its source format
//! - Converting to the intermediate representation
//! - Providing metadata about the plugin

pub mod bruno;
pub mod insomnia;
pub mod postman;

use crate::core::models::{Collection, SourceType};
use anyhow::Result;
use std::path::Path;

/// Errors that can occur during import operations.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// File was not found
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// The file format is invalid
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// Failed to parse the file
    #[error("Parse error: {0}")]
    ParseError(String),

    /// An IO error occurred
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// The version is not supported
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(String),
}

/// Metadata about an importer plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin name
    pub name: &'static str,
    /// Plugin version
    pub version: &'static str,
    /// Source type this plugin handles
    pub source_type: SourceType,
    /// Supported file extensions
    pub file_extensions: &'static [&'static str],
    /// Plugin description
    pub description: &'static str,
}

/// Trait for source-specific importer plugins.
///
/// Implement this trait to add support for a new source format.
/// Each plugin handles the parsing of a specific format and converts
/// it to the common intermediate representation.
pub trait ImporterPlugin: Send + Sync {
    /// Returns metadata about this plugin.
    fn info(&self) -> &PluginInfo;

    /// Import a collection from a single file.
    ///
    /// # Arguments
    /// * `path` - Path to the source file (e.g., `collection.json`)
    ///
    /// # Returns
    /// A `Result` containing the parsed `Collection` or an error.
    fn import_file(&self, path: &Path) -> Result<Collection>;

    /// Import collections from a directory.
    ///
    /// Some sources (like Bruno) store collections as directories
    /// with individual request files.
    ///
    /// # Arguments
    /// * `path` - Path to the directory containing the source files
    ///
    /// # Returns
    /// A `Result` containing a vector of `Collection` objects or an error.
    fn import_directory(&self, path: &Path) -> Result<Vec<Collection>> {
        let _ = path;
        Ok(vec![])
    }

    /// Check if this plugin can handle the given path.
    ///
    /// This method can be used for auto-detection of source format.
    fn can_handle(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            self.info()
                .file_extensions
                .iter()
                .any(|&e| e.eq_ignore_ascii_case(&ext.to_string_lossy()))
        } else {
            false
        }
    }

    /// Validate that the source file/directory is in a supported format.
    fn validate(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(ImportError::FileNotFound(path.display().to_string()).into());
        }
        Ok(())
    }
}

/// Registry for managing available importer plugins.
///
/// This provides a centralized way to access all installed plugins
/// and select the appropriate one for a given source.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn ImporterPlugin>>,
}

impl PluginRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a new plugin.
    pub fn register<P: ImporterPlugin + 'static>(&mut self, plugin: P) {
        self.plugins.push(Box::new(plugin));
    }

    /// Get a plugin by source type.
    pub fn get(&self, source_type: SourceType) -> Option<&dyn ImporterPlugin> {
        self.plugins
            .iter()
            .find(|p| p.info().source_type == source_type)
            .map(|p| p.as_ref())
    }

    /// Find a plugin that can handle the given path.
    #[allow(dead_code)]
    pub fn find_handler(&self, path: &Path) -> Option<&dyn ImporterPlugin> {
        self.plugins
            .iter()
            .find(|p| p.can_handle(path))
            .map(|p| p.as_ref())
    }

    /// Get all registered plugins.
    pub fn plugins(&self) -> &[Box<dyn ImporterPlugin>] {
        &self.plugins
    }

    /// List all supported source types.
    #[allow(dead_code)]
    pub fn supported_types(&self) -> Vec<SourceType> {
        self.plugins.iter().map(|p| p.info().source_type).collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a registry with all built-in plugins pre-registered.
pub fn default_registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    registry.register(postman::PostmanImporter::new());
    registry.register(insomnia::InsomniaImporter::new());
    registry.register(bruno::BrunoImporter::new());
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPlugin {
        source: SourceType,
        info: PluginInfo,
    }

    impl MockPlugin {
        fn new(source: SourceType) -> Self {
            Self {
                source,
                info: PluginInfo {
                    name: "Mock",
                    version: "1.0.0",
                    source_type: source,
                    file_extensions: &["mock"],
                    description: "Mock plugin for testing",
                },
            }
        }
    }

    impl ImporterPlugin for MockPlugin {
        fn info(&self) -> &PluginInfo {
            &self.info
        }

        fn import_file(&self, _path: &Path) -> Result<Collection> {
            Ok(Collection::new("Mock Collection"))
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = PluginRegistry::new();
        registry.register(MockPlugin::new(SourceType::Postman));
        registry.register(MockPlugin::new(SourceType::Insomnia));

        assert!(registry.get(SourceType::Postman).is_some());
        assert!(registry.get(SourceType::Insomnia).is_some());
        assert!(registry.get(SourceType::Bruno).is_none());
    }

    #[test]
    fn test_registry_supported_types() {
        let mut registry = PluginRegistry::new();
        registry.register(MockPlugin::new(SourceType::Postman));
        registry.register(MockPlugin::new(SourceType::Bruno));

        let types = registry.supported_types();
        assert!(types.contains(&SourceType::Postman));
        assert!(types.contains(&SourceType::Bruno));
        assert!(!types.contains(&SourceType::Insomnia));
    }
}
