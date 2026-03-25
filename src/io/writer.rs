//! Posting format writer.
//!
//! This module handles writing the intermediate representation
//! to Posting TUI's YAML format.

use crate::core::models::{Collection, Environment, Request, RequestBody, Scripts};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration for the writer.
#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// Name of the collection
    pub collection_name: String,
    /// Output directory path
    pub output_dir: PathBuf,
    /// Whether to overwrite existing files
    pub overwrite: bool,
    /// Whether to preserve directory structure
    pub preserve_structure: bool,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            collection_name: String::new(),
            output_dir: PathBuf::from("."),
            overwrite: false,
            preserve_structure: true,
        }
    }
}

impl WriterConfig {
    /// Creates a new writer config with the specified output directory.
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            ..Default::default()
        }
    }

    /// Sets the collection name.
    pub fn with_collection_name(mut self, name: impl Into<String>) -> Self {
        self.collection_name = name.into();
        self
    }

    /// Sets the overwrite flag.
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
}

/// Writer for Posting TUI format.
pub struct PostingWriter {
    config: WriterConfig,
}

impl PostingWriter {
    /// Creates a new PostingWriter with the given configuration.
    pub fn new(config: WriterConfig) -> Self {
        Self { config }
    }

    /// Writes a collection to the configured output directory.
    pub fn write_collection(&self, collection: &Collection) -> Result<Vec<PathBuf>> {
        let output_path = self.config.output_dir.join(&collection.name);
        self.write_collection_to(collection, &output_path)
    }

    /// Writes a collection to a specific base path.
    pub fn write_collection_to(
        &self,
        collection: &Collection,
        base_path: &Path,
    ) -> Result<Vec<PathBuf>> {
        fs::create_dir_all(base_path).context("Failed to create output directory")?;

        let mut written_files = Vec::new();

        // Write README if present
        if let Some(readme) = &collection.readme {
            let readme_path = base_path.join("README.md");
            fs::write(&readme_path, readme).context("Failed to write README")?;
            written_files.push(readme_path);
        }

        // Write each request
        for request in &collection.requests {
            let request_path = self.write_request(request, base_path)?;
            written_files.push(request_path);
        }

        // Recursively write subfolders
        for subfolder in &collection.subfolders {
            let subfolder_path = base_path.join(&subfolder.name);
            let subfolder_files = self.write_collection_to(subfolder, &subfolder_path)?;
            written_files.extend(subfolder_files);
        }

        // Write environment files
        if collection.has_environments() {
            // Write .env.{name} files for each environment
            for env in &collection.environments {
                let env_path = base_path.join(env.filename());
                let env_content = self.generate_env_content_with_values(&env.variables);
                fs::write(&env_path, env_content).context("Failed to write environment file")?;
                written_files.push(env_path);
            }
        } else {
            // No environments defined - collect variables from requests and create posting.env
            let vars = self.collect_fallback_variables(collection);
            if !vars.is_empty() {
                let env_path = base_path.join("posting.env");
                let env_content = self.generate_env_content(&vars);
                fs::write(&env_path, env_content).context("Failed to write posting.env file")?;
                written_files.push(env_path);
            }
        }

        Ok(written_files)
    }

    fn collect_fallback_variables(&self, collection: &Collection) -> Vec<String> {
        let mut vars = collection.fallback_variables.clone();
        for request in &collection.requests {
            for var in &request.variables {
                if !vars.contains(var) {
                    vars.push(var.clone());
                }
            }
        }
        vars.sort();
        vars.dedup();
        vars
    }

    fn generate_env_content_with_values(&self, vars: &HashMap<String, String>) -> String {
        let mut content = String::from("# Environment variables for Posting TUI\n");
        content.push_str("# Load with: posting --env filename.env\n\n");

        let mut keys: Vec<_> = vars.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(value) = vars.get(key) {
                content.push_str(&format!("{}={}\n", key, value));
            }
        }

        content
    }

    fn write_request(&self, request: &Request, base_path: &Path) -> Result<PathBuf> {
        let safe_name = sanitize_filename(&request.name);
        let file_path = base_path.join(format!("{}.posting.yaml", safe_name));

        if file_path.exists() && !self.config.overwrite {
            anyhow::bail!(
                "File already exists: {} (use --overwrite to replace)",
                file_path.display()
            );
        }

        let yaml_content = self.serialize_request(request)?;
        fs::write(&file_path, yaml_content).context("Failed to write request file")?;

        Ok(file_path)
    }

    fn serialize_request(&self, request: &Request) -> Result<String> {
        let mut lines = Vec::new();

        // Header comment
        lines.push(format!(
            "# Posting request: {} ({})\n",
            request.name,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Name
        lines.push(format!("name: {}", escape_yaml_string(&request.name)));

        // Method and URL
        lines.push(format!("method: {}", request.method));
        lines.push(format!("url: {}", escape_yaml_string(&request.url)));

        // Description
        if !request.description.is_empty() {
            lines.push(format!(
                "description: {}",
                escape_yaml_string(&request.description)
            ));
        }

        // Body
        if let Some(body) = &request.body {
            self.serialize_body(body, &mut lines);
        }

        // Auth
        if let Some(auth) = &request.auth {
            self.serialize_auth(auth, &mut lines);
        }

        // Headers
        if !request.headers.is_empty() {
            lines.push("headers:".to_string());
            for header in &request.headers {
                let enabled_mark = if header.enabled { "" } else { " # disabled" };
                lines.push(format!(
                    "  - name: {}\n    value: {}{}",
                    escape_yaml_string(&header.name),
                    escape_yaml_string(&header.value),
                    enabled_mark
                ));
            }
        }

        // Query params
        if !request.params.is_empty() {
            lines.push("params:".to_string());
            for param in &request.params {
                let enabled_mark = if param.enabled { "" } else { " # disabled" };
                lines.push(format!(
                    "  - name: {}\n    value: {}{}",
                    escape_yaml_string(&param.name),
                    escape_yaml_string(&param.value),
                    enabled_mark
                ));
            }
        }

        // Path params
        if !request.path_params.is_empty() {
            lines.push("path_params:".to_string());
            for param in &request.path_params {
                lines.push(format!(
                    "  - name: {}\n    value: {}",
                    escape_yaml_string(&param.name),
                    escape_yaml_string(&param.value)
                ));
            }
        }

        // Scripts
        if let Some(scripts) = &request.scripts {
            self.serialize_scripts(scripts, &mut lines);
        }

        // Options
        self.serialize_options(&request.options, &mut lines);

        Ok(lines.join("\n"))
    }

    fn serialize_body(&self, body: &RequestBody, lines: &mut Vec<String>) {
        if let Some(content) = &body.content {
            if !content.is_empty() {
                if body
                    .content_type
                    .as_ref()
                    .map_or(false, |ct| ct.contains("json"))
                {
                    // Try to format as YAML for readability
                    if let Ok(formatted) = serde_json::from_str::<serde_json::Value>(content) {
                        if let Ok(pretty) = serde_yaml::to_string(&formatted) {
                            lines.push("body:".to_string());
                            for line in pretty.lines() {
                                lines.push(format!("  {}", line));
                            }
                            return;
                        }
                    }
                }
                lines.push("body:".to_string());
                lines.push("  content: |".to_string());
                for line in content.lines() {
                    lines.push(format!("    {}", line));
                }
            }
        }

        if let Some(form_data) = &body.form_data {
            if !form_data.is_empty() {
                lines.push("body:".to_string());
                lines.push("  form_data:".to_string());
                for item in form_data {
                    let enabled_mark = if item.enabled { "" } else { " # disabled" };
                    lines.push(format!(
                        "    - name: {}\n      value: {}{}",
                        escape_yaml_string(&item.name),
                        escape_yaml_string(&item.value),
                        enabled_mark
                    ));
                }
            }
        }
    }

    fn serialize_auth(&self, auth: &crate::core::models::Auth, lines: &mut Vec<String>) {
        let Some(auth_type) = &auth.auth_type else {
            return;
        };

        lines.push("auth:".to_string());
        lines.push(format!("  type: {}", auth_type.as_str()));

        if let Some(basic) = &auth.basic {
            lines.push("  basic:".to_string());
            lines.push(format!(
                "    username: {}",
                escape_yaml_string(&basic.username)
            ));
            lines.push(format!(
                "    password: {}",
                escape_yaml_string(&basic.password)
            ));
        }

        if let Some(digest) = &auth.digest {
            lines.push("  digest:".to_string());
            lines.push(format!(
                "    username: {}",
                escape_yaml_string(&digest.username)
            ));
            lines.push(format!(
                "    password: {}",
                escape_yaml_string(&digest.password)
            ));
        }

        if let Some(bearer) = &auth.bearer_token {
            lines.push("  bearer_token:".to_string());
            lines.push(format!("    token: {}", escape_yaml_string(&bearer.token)));
        }
    }

    fn serialize_scripts(&self, scripts: &Scripts, lines: &mut Vec<String>) {
        lines.push("scripts:".to_string());

        if let Some(setup) = &scripts.setup {
            lines.push(format!("  setup: {}", escape_yaml_string(setup)));
        }
        if let Some(on_request) = &scripts.on_request {
            lines.push(format!("  on_request: {}", escape_yaml_string(on_request)));
        }
        if let Some(on_response) = &scripts.on_response {
            lines.push(format!(
                "  on_response: {}",
                escape_yaml_string(on_response)
            ));
        }
    }

    fn serialize_options(
        &self,
        options: &crate::core::models::RequestOptions,
        lines: &mut Vec<String>,
    ) {
        let default = crate::core::models::RequestOptions::default();

        if options.follow_redirects != default.follow_redirects {
            lines.push(format!("  follow_redirects: {}", options.follow_redirects));
        }
        if options.verify_ssl != default.verify_ssl {
            lines.push(format!("  verify_ssl: {}", options.verify_ssl));
        }
        if options.attach_cookies != default.attach_cookies {
            lines.push(format!("  attach_cookies: {}", options.attach_cookies));
        }
        if !options.proxy_url.is_empty() {
            lines.push(format!(
                "  proxy_url: {}",
                escape_yaml_string(&options.proxy_url)
            ));
        }
        if (options.timeout - default.timeout).abs() > f64::EPSILON {
            lines.push(format!("  timeout: {}", options.timeout));
        }
    }

    fn generate_env_content(&self, vars: &[String]) -> String {
        let mut content = String::from("# Environment variables for Posting TUI\n");
        content.push_str("# Copy this file to posting.env or load with --env flag\n\n");

        for var in vars {
            content.push_str(&format!("{}=", var));
            content.push('\n');
        }

        content
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

fn escape_yaml_string(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }

    // Check if string needs quoting
    let needs_quoting = s.starts_with('-')
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s.chars().any(|c| {
            c == ':'
                || c == '#'
                || c == '\n'
                || c == '\''
                || c == '"'
                || c == '['
                || c == ']'
                || c == '{'
                || c == '}'
                || c == ','
                || c == '&'
                || c == '*'
                || c == '!'
                || c == '|'
                || c == '>'
                || c == '%'
                || c == '@'
                || c == '`'
        });

    if needs_quoting {
        if s.contains('\'') {
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
        } else {
            format!("'{}'", s)
        }
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{Auth, HttpMethod};

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("normal-name"), "normal-name");
        assert_eq!(sanitize_filename("name/with/slashes"), "name_with_slashes");
        assert_eq!(sanitize_filename("name:with:colons"), "name_with_colons");
        assert_eq!(
            sanitize_filename("name*with*asterisks"),
            "name_with_asterisks"
        );
    }

    #[test]
    fn test_escape_yaml_string() {
        assert_eq!(escape_yaml_string("simple"), "simple");
        assert_eq!(escape_yaml_string(""), "''");
        assert_eq!(escape_yaml_string("with:colon"), "'with:colon'");
        assert_eq!(escape_yaml_string("with # hash"), "'with # hash'");
    }

    #[test]
    fn test_write_request() {
        let config = WriterConfig {
            collection_name: "Test".to_string(),
            output_dir: std::env::temp_dir(),
            overwrite: true,
            preserve_structure: true,
        };
        let writer = PostingWriter::new(config);

        let request = Request::new(
            "Test Request",
            HttpMethod::Get,
            "https://api.example.com/users",
        )
        .add_header("Accept", "application/json")
        .add_param("page", "1");

        let yaml = writer.serialize_request(&request).unwrap();
        assert!(yaml.contains("name: Test Request"));
        assert!(yaml.contains("method: GET"));
        assert!(yaml.contains("url:"));
        assert!(yaml.contains("https://api.example.com/users"));
        assert!(yaml.contains("headers:"));
        assert!(yaml.contains("Accept"));
    }

    #[test]
    fn test_write_request_with_auth() {
        let config = WriterConfig {
            collection_name: "Test".to_string(),
            output_dir: std::env::temp_dir(),
            overwrite: true,
            preserve_structure: true,
        };
        let writer = PostingWriter::new(config);

        let request = Request::new(
            "Auth Request",
            HttpMethod::Post,
            "https://api.example.com/login",
        )
        .with_auth(Auth::bearer("token123"));

        let yaml = writer.serialize_request(&request).unwrap();
        assert!(yaml.contains("auth:"));
        assert!(yaml.contains("bearer_token"));
        assert!(yaml.contains("token123"));
    }
}
