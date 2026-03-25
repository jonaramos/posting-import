//! posting-import - Import API collections from Postman, Insomnia, and Bruno to Posting TUI format
//!
//! This tool allows you to import API collections from popular API clients
//! and convert them to the Posting TUI's YAML format.
//!
//! # Supported Sources
//!
//! - **Postman**: Collections exported in JSON format (v2.0/v2.1)
//! - **Insomnia**: Collections exported in JSON format (v4/v5)
//! - **Bruno**: Collections in both `.bru` format and OpenCollection YAML format
//!
//! # Usage
//!
//! ```bash
//! # Import a Postman collection
//! posting-import --app postman --source collection.json --target ./output
//!
//! # Import an Insomnia collection
//! posting-import --app insomnia --source insomnia-export.json --target ./output
//!
//! # Import a Bruno collection
//! posting-import --app bruno --source ./my-bruno-collection --target ./output
//!
//! # List supported formats
//! posting-import --list-sources
//! ```
//!
//! # Examples
//!
//! ## Import from Postman
//!
//! ```bash
//! # Export your Postman collection as JSON, then:
//! posting-import -a postman -s postman-collection.json -t ./posting-collections
//! ```
//!
//! ## Import from Insomnia
//!
//! ```bash
//! # Export your Insomnia workspace as JSON, then:
//! posting-import -a insomnia -s insomnia-export.json -t ./posting-collections
//! ```
//!
//! ## Import from Bruno
//!
//! ```bash
//! # Point to your Bruno collection directory or opencollection.yml file:
//! posting-import -a bruno -s ./bruno/petstore -t ./posting-collections
//! ```
//!
//! # Environment Variables
//!
//! - `POSTING_IMPORTER_VERBOSE`: Set verbosity level (1-3)

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cli;
mod core;
mod io;
mod plugins;

use anyhow::Result;
use cli::run as cli_run;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    cli_run()
}
