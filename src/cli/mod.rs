//! CLI module for posting-import.
//!
//! This module provides the command-line interface using clap.

use crate::core::models::SourceType;
use crate::io::{PostingWriter, WriterConfig};
use crate::plugins::{default_registry, PluginRegistry};
use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

/// Command-line interface configuration.
#[derive(Parser, Debug)]
#[command(
    name = "posting-import",
    about = "Import API collections from Postman, Insomnia, and Bruno to Posting TUI format",
    long_about = None,
    author = "Your Name <your.email@example.com>",
    version
)]
pub struct Cli {
    /// Source application type (postman, insomnia, or bruno)
    #[arg(short, long, value_enum, required = true)]
    pub app: SourceApp,

    /// Path to the source collection file or directory
    #[arg(short, long, required = true)]
    pub source: PathBuf,

    /// Output directory for the Posting collection
    #[arg(short, long, default_value = ".")]
    pub target: PathBuf,

    /// Overwrite existing files
    #[arg(short = 'w', long, default_value = "false")]
    pub overwrite: bool,

    /// Verbose output (repeat for more verbosity)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// List supported source formats and exit
    #[arg(long)]
    pub list_sources: bool,

    /// Don't write output (useful with --list-sources)
    #[arg(short = 'n', long, default_value = "false")]
    pub dry_run: bool,

    /// Output format for collection info
    #[arg(long, value_enum, default_value = "text")]
    pub format: OutputFormat,

    /// Collection name (overrides detected name)
    #[arg(short = 'c', long)]
    pub name: Option<String>,
}

/// Source application type.
#[derive(Debug, Clone, ValueEnum)]
pub enum SourceApp {
    /// Postman collection format
    Postman,
    /// Insomnia collection format
    Insomnia,
    /// Bruno collection format
    Bruno,
}

impl std::fmt::Display for SourceApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceApp::Postman => write!(f, "postman"),
            SourceApp::Insomnia => write!(f, "insomnia"),
            SourceApp::Bruno => write!(f, "bruno"),
        }
    }
}

impl From<SourceApp> for SourceType {
    fn from(app: SourceApp) -> Self {
        match app {
            SourceApp::Postman => SourceType::Postman,
            SourceApp::Insomnia => SourceType::Insomnia,
            SourceApp::Bruno => SourceType::Bruno,
        }
    }
}

/// Output format for displaying information.
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// Plain text output
    #[default]
    Text,
    /// JSON output
    Json,
    /// YAML output
    Yaml,
}

/// Runs the CLI application.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    setup_logging(cli.verbose)?;

    // Create plugin registry
    let registry = default_registry();

    // Handle --list-sources
    if cli.list_sources {
        list_sources(&registry, &cli.format);
        return Ok(());
    }

    // Validate paths
    if !cli.source.exists() {
        anyhow::bail!("Source path does not exist: {}", cli.source.display());
    }

    // Get the appropriate plugin
    let source_type: SourceType = cli.app.clone().into();
    let plugin = registry
        .get(source_type)
        .with_context(|| format!("No plugin found for {}", cli.app))?;

    info!(
        "Importing {} collection from: {}",
        cli.app,
        cli.source.display()
    );

    // Import the collection
    let collection = if cli.source.is_dir() {
        let collections = plugin.import_directory(&cli.source)?;
        if collections.len() == 1 {
            collections.into_iter().next().unwrap()
        } else if collections.is_empty() {
            anyhow::bail!("No collections found in directory");
        } else {
            // Merge multiple collections into one
            let mut merged = crate::core::models::Collection::new(
                cli.name.as_deref().unwrap_or(
                    cli.source
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Merged"),
                ),
            );
            for col in collections {
                merged.subfolders.push(col);
            }
            merged
        }
    } else {
        let mut collection = plugin.import_file(&cli.source)?;

        // Apply custom name if provided
        if let Some(name) = &cli.name {
            collection.name = name.clone();
        }

        collection
    };

    info!(
        "Found collection: {} ({} requests)",
        collection.name,
        collection.total_requests()
    );

    // Write to Posting format
    if cli.dry_run {
        info!("Dry run - not writing files");
        print_collection_info(&collection, &cli.format);
    } else {
        let writer_config = WriterConfig {
            collection_name: collection.name.clone(),
            output_dir: cli.target.clone(),
            overwrite: cli.overwrite,
            preserve_structure: true,
        };

        let writer = PostingWriter::new(writer_config);
        let written_files = writer.write_collection(&collection)?;

        info!(
            "Successfully wrote {} files to: {}",
            written_files.len(),
            cli.target.display()
        );

        for file in &written_files {
            println!("  Created: {}", file.display());
        }
    }

    Ok(())
}

fn setup_logging(verbosity: u8) -> Result<()> {
    let filter = match verbosity {
        0 => EnvFilter::new("warning"),
        1 => EnvFilter::new("info"),
        2 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    };

    fmt()
        .with_env_filter(filter)
        .with_target(verbosity >= 2)
        .with_thread_ids(verbosity >= 3)
        .init();

    Ok(())
}

fn list_sources(registry: &PluginRegistry, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            let sources: Vec<_> = registry
                .plugins()
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "name": p.info().name,
                        "type": p.info().source_type.as_str(),
                        "extensions": p.info().file_extensions,
                        "description": p.info().description,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&sources).unwrap());
        }
        OutputFormat::Yaml => {
            for plugin in registry.plugins() {
                println!(
                    "- name: {}
  type: {}
  extensions: {:?}",
                    plugin.info().name,
                    plugin.info().source_type.as_str(),
                    plugin.info().file_extensions
                );
            }
        }
        OutputFormat::Text => {
            println!("Supported source formats:\n");
            for plugin in registry.plugins() {
                println!(
                    "  {} ({})",
                    plugin.info().name,
                    plugin.info().source_type.as_str()
                );
                println!(
                    "    Supported extensions: {}\n",
                    plugin.info().file_extensions.join(", ")
                );
            }
        }
    }
}

fn print_collection_info(collection: &crate::core::models::Collection, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            let info = serde_json::json!({
                "name": collection.name,
                "requests": collection.total_requests(),
                "folders": collection.subfolders.len(),
            });
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
        OutputFormat::Yaml => {
            println!(
                "name: {}\nrequests: {}\nfolders: {}",
                collection.name,
                collection.total_requests(),
                collection.subfolders.len()
            );
        }
        OutputFormat::Text => {
            println!("\nCollection: {}", collection.name);
            println!("Requests: {}", collection.total_requests());
            println!("Folders: {}", collection.subfolders.len());

            if !collection.requests.is_empty() {
                println!("\nRequests:");
                for req in &collection.requests {
                    println!("  [{}] {}", req.method, req.name);
                }
            }

            if !collection.subfolders.is_empty() {
                println!("\nFolders:");
                for folder in &collection.subfolders {
                    println!("  {}/ ({} requests)", folder.name, folder.total_requests());
                }
            }
        }
    }
}
