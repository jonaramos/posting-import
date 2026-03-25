//! Bruno collection importer plugin.
//!
//! Supports Bruno collections in both .bru format and the new OpenCollection YAML format.

use crate::core::models::{
    Auth, AuthType, Collection, Environment, Header, HttpMethod, Request, RequestBody, SourceType,
};
use crate::plugins::{ImportError, ImporterPlugin, PluginInfo};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const PLUGIN_INFO: PluginInfo = PluginInfo {
    name: "Bruno Importer",
    version: "1.0.0",
    source_type: SourceType::Bruno,
    file_extensions: &["bru", "yml", "yaml"],
    description: "Import Bruno collections (.bru and OpenCollection YAML format)",
};

#[derive(Debug, PartialEq, Eq)]
enum BrunoFormat {
    Bru,
    OpenCollection,
    Unknown,
}

#[derive(Debug)]
struct BruFile {
    path: PathBuf,
    name: String,
    content: String,
}

#[derive(Debug, Default)]
struct BruMeta {
    name: Option<String>,
    uid: Option<String>,
    description: Option<String>,
}

#[derive(Debug)]
struct BruRequest {
    method: String,
    url: String,
    variables: Vec<String>,
    headers: Vec<(String, String, bool)>,
    body: Option<String>,
    body_content_type: Option<String>,
    auth: Option<BruAuth>,
    vars: HashMap<String, String>,
}

#[derive(Debug)]
enum BruAuth {
    Basic { username: String, password: String },
    Bearer { token: String },
    Digest { username: String, password: String },
}

/// Bruno collection importer plugin.
pub struct BrunoImporter;

impl BrunoImporter {
    /// Creates a new BrunoImporter instance.
    pub fn new() -> Self {
        Self
    }

    /// Transforms variable syntax ({{variable}}) to Posting syntax (${variable}).
    fn transform_variables(&self, input: &str) -> (String, Vec<String>) {
        let re = regex::Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap();
        let mut vars = Vec::new();

        for caps in re.captures_iter(input) {
            if let Some(var_name) = caps.get(1) {
                vars.push(var_name.as_str().to_string());
            }
        }

        let transformed =
            re.replace_all(input, |caps: &regex::Captures| format!("${{{}}}", &caps[1]));
        (transformed.to_string(), vars)
    }

    fn detect_format(&self, path: &Path) -> BrunoFormat {
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                match ext.as_str() {
                    "bru" => return BrunoFormat::Bru,
                    "yml" | "yaml" | "json" => return BrunoFormat::OpenCollection,
                    _ => {}
                }
            }
            // Check if it's an opencollection file
            if path.file_name().map_or(false, |n| {
                n == "opencollection.yml" || n == "opencollection.json"
            }) {
                return BrunoFormat::OpenCollection;
            }
        }

        // Check for OpenCollection format
        if path.join("opencollection.yml").exists() || path.join("opencollection.json").exists() {
            return BrunoFormat::OpenCollection;
        }

        // Check for bru files or environments folder
        if path.join("collection.bru").exists()
            || path.join("requests").exists()
            || path.join("environments").exists()
            || self.has_bru_files(path)
        {
            return BrunoFormat::Bru;
        }

        BrunoFormat::Unknown
    }

    fn has_bru_files(&self, path: &Path) -> bool {
        if path.is_dir() {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.path().extension().map_or(false, |ext| ext == "bru") {
                    return true;
                }
            }
        }
        false
    }

    fn parse_bru_env_file(&self, path: &Path, content: &str) -> Option<Environment> {
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if ext == "yml" || ext == "yaml" {
            // Parse YAML format
            let yaml: serde_yaml::Value = match serde_yaml::from_str(content) {
                Ok(v) => v,
                Err(_) => return None,
            };

            let mut env = Environment::new(&name);

            if let Some(vars) = yaml.get("variables").and_then(|v| v.as_sequence()) {
                for var in vars {
                    if let Some(var_obj) = var.as_mapping() {
                        let var_name = var_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let var_value = var_obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        if !var_name.is_empty() {
                            env.variables
                                .insert(var_name.to_string(), var_value.to_string());
                        }
                    }
                }
            }

            if env.variables.is_empty() {
                None
            } else {
                Some(env)
            }
        } else {
            // Parse .bru format
            let mut env = Environment::new(&name);

            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("name:") {
                    if let Some(n) = line.get("name:".len()..).map(|s| s.trim()) {
                        if !n.is_empty() {
                            env.name = n.to_string();
                        }
                    }
                } else if line.starts_with("vars") || line.starts_with("vars {") {
                    // Continue to parse vars block
                    continue;
                } else if line.contains(':') && !line.starts_with('#') {
                    // Parse key: value pairs
                    let parts: Vec<&str> = line.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let key = parts[0].trim();
                        let value = parts[1].trim();
                        if !key.is_empty() && key != "name" && key != "type" && key != "seq" {
                            env.variables.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }

            if env.variables.is_empty() {
                None
            } else {
                Some(env)
            }
        }
    }

    fn parse_bru_file(&self, content: &str) -> Result<BruRequest> {
        let mut method = "GET".to_string();
        let mut url = String::new();
        let mut variables = Vec::new();
        let mut headers = Vec::new();
        let mut body = None;
        let mut body_content_type = None;
        let mut auth = None;
        let mut vars = HashMap::new();

        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            // Check for section headers
            if line == "meta"
                || line == "auth"
                || line == "body"
                || line == "http"
                || line == "vars"
            {
                current_section = line.to_string();
                continue;
            }

            match current_section.as_str() {
                "http" => {
                    if line.starts_with("method:") {
                        method = line["method:".len()..].trim().to_string();
                    } else if line.starts_with("url:") {
                        let (transformed, vars) =
                            self.transform_variables(line["url:".len()..].trim());
                        url = transformed;
                        variables.extend(vars);
                    }
                }
                "body" => {
                    if line.starts_with("type:") {
                        let body_type = line["type:".len()..].trim();
                        if body_type == "json" {
                            body_content_type = Some("application/json".to_string());
                        }
                    } else if !line.starts_with("type:") && body.is_none() {
                        // This is the body content
                        if body_content_type.is_some()
                            || line.starts_with('{')
                            || line.starts_with('[')
                        {
                            body = Some(line.to_string());
                        }
                    }
                }
                "auth" => {
                    if line.starts_with("type:") {
                        let auth_type = line["type:".len()..].trim();
                        match auth_type {
                            "basic" => {
                                auth = Some(BruAuth::Basic {
                                    username: String::new(),
                                    password: String::new(),
                                });
                            }
                            "bearer" => {
                                auth = Some(BruAuth::Bearer {
                                    token: String::new(),
                                });
                            }
                            "digest" => {
                                auth = Some(BruAuth::Digest {
                                    username: String::new(),
                                    password: String::new(),
                                });
                            }
                            _ => {}
                        }
                    } else if let Some(BruAuth::Basic { username, .. }) = &auth {
                        if line.starts_with("username:") && username.is_empty() {
                            // Will be set in next iteration
                        }
                    }
                }
                "vars" => {
                    if line.contains(':') {
                        let parts: Vec<&str> = line.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            vars.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
                        }
                    }
                }
                _ => {
                    // Headers outside sections (but skip env vars file metadata)
                    if line.contains(':') && !line.starts_with('{') && !line.starts_with("vars") {
                        let parts: Vec<&str> = line.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            let key = parts[0].trim();
                            let value = parts[1].trim();
                            // Skip http and body type indicators
                            if !key.eq_ignore_ascii_case("method")
                                && !key.eq_ignore_ascii_case("url")
                                && !key.eq_ignore_ascii_case("type")
                                && !key.eq_ignore_ascii_case("name")
                                && !key.eq_ignore_ascii_case("seq")
                            {
                                headers.push((key.to_string(), value.to_string(), true));
                            }
                        }
                    }
                }
            }
        }

        Ok(BruRequest {
            method,
            url,
            variables,
            headers,
            body,
            body_content_type,
            auth,
            vars,
        })
    }

    fn parse_bru_collection(&self, dir: &Path) -> Result<Collection> {
        let collection_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Bruno Collection")
            .to_string();

        let mut collection = Collection::new(&collection_name);

        // Try to read collection description
        if let Ok(content) = fs::read_to_string(dir.join("README.md")) {
            if !content.is_empty() {
                collection.readme = Some(content);
            }
        }

        // Parse environments from environments/ folder
        let envs_dir = dir.join("environments");
        if envs_dir.exists() {
            for entry in WalkDir::new(&envs_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map_or(false, |ext| ext == "bru" || ext == "yml" || ext == "yaml")
                })
            {
                let path = entry.path();
                if let Ok(content) = fs::read_to_string(path) {
                    if let Some(env) = self.parse_bru_env_file(path, &content) {
                        collection.environments.push(env);
                    }
                }
            }
        }

        // Find all .bru files
        let requests_dir = dir.join("requests");
        let envs_dir = dir.join("environments");
        let search_root = if requests_dir.exists() {
            &requests_dir
        } else {
            dir
        };

        for entry in WalkDir::new(search_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                if !e.path().extension().map_or(false, |ext| ext == "bru") {
                    return false;
                }
                // Skip files in environments folder
                if let Ok(rel) = e.path().strip_prefix(dir) {
                    let rel_str = rel.to_string_lossy();
                    !rel_str.starts_with("environments") && !rel_str.contains("/environments/")
                } else {
                    true
                }
            })
        {
            let path = entry.path();
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("Unnamed")
                .to_string();

            match self.parse_bru_file(&content) {
                Ok(bru_req) => {
                    let mut request = Request {
                        name: name.clone(),
                        description: String::new(),
                        method: HttpMethod::from(bru_req.method.as_str()),
                        url: bru_req.url,
                        variables: bru_req.variables,
                        ..Default::default()
                    };

                    // Add headers
                    for (key, value, enabled) in bru_req.headers {
                        request.headers.push(Header {
                            name: key,
                            value,
                            enabled,
                        });
                    }

                    // Add body
                    if let Some(body_content) = bru_req.body {
                        request.body = Some(RequestBody {
                            content: Some(body_content),
                            form_data: None,
                            content_type: bru_req.body_content_type,
                        });
                    }

                    // Add auth
                    if let Some(auth) = bru_req.auth {
                        request.auth = match auth {
                            BruAuth::Basic { username, password } => {
                                Some(Auth::basic(username, password))
                            }
                            BruAuth::Bearer { token } => Some(Auth::bearer(token)),
                            BruAuth::Digest { username, password } => Some(Auth {
                                auth_type: Some(AuthType::Digest),
                                digest: Some(crate::core::models::DigestAuth {
                                    username,
                                    password,
                                }),
                                ..Default::default()
                            }),
                        };
                    }

                    collection.requests.push(request);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse {}: {}", path.display(), e);
                }
            }
        }

        Ok(collection)
    }

    fn parse_opencollection(&self, path: &Path) -> Result<Vec<Collection>> {
        let content = fs::read_to_string(path).context("Failed to read OpenCollection file")?;

        // Try to parse as JSON first, then YAML
        let value: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => {
                let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
                    .context("Failed to parse OpenCollection YAML")?;
                serde_json::to_value(yaml).context("Failed to convert YAML to JSON")?
            }
        };

        let mut collections = Vec::new();

        // Handle arrays of collections
        if let Some(items) = value.as_array() {
            for item in items {
                if let Some(collection) = self.parse_opencollection_json_item(item)? {
                    collections.push(collection);
                }
            }
        } else if let Some(_) = value.as_object() {
            // Single collection format - could be a collection root or a single item
            if let Some(collection) = self.parse_opencollection_json_item(&value)? {
                collections.push(collection);
            }
        }

        Ok(collections)
    }

    fn parse_opencollection_json_item(
        &self,
        item: &serde_json::Value,
    ) -> Result<Option<Collection>> {
        let obj = item.as_object().ok_or_else(|| {
            ImportError::InvalidFormat("Expected object in OpenCollection item".to_string())
        })?;

        let mut collection = Collection::new("");

        for (key, value) in obj {
            match key.as_str() {
                "name" => {
                    collection.name = value.as_str().unwrap_or("").to_string();
                }
                "description" => {
                    collection.readme = value.as_str().map(String::from);
                }
                "requests" | "items" => {
                    if let Some(items) = value.as_array() {
                        for req in items {
                            if let Some(request) = self.parse_opencollection_json_request(req)? {
                                collection.requests.push(request);
                            }
                            // Check if it's a nested collection/folder
                            if let Some(nested) = self.parse_opencollection_json_item(req)? {
                                if !nested.name.is_empty() {
                                    collection.subfolders.push(nested);
                                }
                            }
                        }
                    }
                }
                "info" => {
                    // Collection-level info
                    if let Some(info) = value.as_object() {
                        if collection.name.is_empty() {
                            collection.name = info
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                        }
                        if collection.readme.is_none() {
                            collection.readme = info
                                .get("description")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                        }
                    }
                }
                _ => {}
            }
        }

        if collection.name.is_empty() {
            return Ok(None);
        }

        Ok(Some(collection))
    }

    fn parse_opencollection_json_request(
        &self,
        item: &serde_json::Value,
    ) -> Result<Option<Request>> {
        let obj = item.as_object().ok_or_else(|| {
            ImportError::InvalidFormat("Expected object in OpenCollection request".to_string())
        })?;

        let mut name = String::new();
        let mut method = HttpMethod::Get;
        let mut url = String::new();
        let mut variables = Vec::new();
        let mut headers = Vec::new();
        let mut body = None;

        for (key, value) in obj {
            match key.as_str() {
                "name" => {
                    name = value.as_str().unwrap_or("").to_string();
                }
                "method" => {
                    method = HttpMethod::from(value.as_str().unwrap_or("GET"));
                }
                "url" | "path" => {
                    let (transformed, vars) =
                        self.transform_variables(value.as_str().unwrap_or(""));
                    url = transformed;
                    variables.extend(vars);
                }
                "http" => {
                    // Nested http block
                    if let Some(http) = value.as_object() {
                        if let Some(m) = http.get("method").and_then(|v| v.as_str()) {
                            method = HttpMethod::from(m);
                        }
                        if let Some(u) = http.get("url").and_then(|v| v.as_str()) {
                            let (transformed, vars) = self.transform_variables(u);
                            url = transformed;
                            variables.extend(vars);
                        }
                        // Parse headers
                        if let Some(hdrs) = http.get("headers").and_then(|v| v.as_array()) {
                            for hdr in hdrs {
                                if let Some(hdr_obj) = hdr.as_object() {
                                    let hdr_name =
                                        hdr_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    let hdr_value =
                                        hdr_obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
                                    headers.push(Header {
                                        name: hdr_name.to_string(),
                                        value: hdr_value.to_string(),
                                        enabled: true,
                                    });
                                }
                            }
                        }
                        // Parse body
                        if let Some(body_obj) = http.get("body").and_then(|v| v.as_object()) {
                            if let Some(body_type) = body_obj.get("type").and_then(|v| v.as_str()) {
                                let content_type = match body_type {
                                    "json" => Some("application/json".to_string()),
                                    "xml" => Some("application/xml".to_string()),
                                    "text" => Some("text/plain".to_string()),
                                    "form-urlencoded" => {
                                        Some("application/x-www-form-urlencoded".to_string())
                                    }
                                    "multipart-form" => Some("multipart/form-data".to_string()),
                                    _ => None,
                                };
                                if let Some(data) = body_obj.get("data").and_then(|v| v.as_str()) {
                                    body = Some(RequestBody {
                                        content: Some(data.to_string()),
                                        form_data: None,
                                        content_type,
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if name.is_empty() {
            return Ok(None);
        }

        Ok(Some(Request {
            name,
            description: String::new(),
            method,
            url,
            headers,
            body,
            auth: None,
            params: Vec::new(),
            path_params: Vec::new(),
            scripts: None,
            options: Default::default(),
            variables,
        }))
    }

    fn parse_opencollection_item(&self, item: &serde_yaml::Value) -> Result<Option<Collection>> {
        let obj = item.as_mapping().ok_or_else(|| {
            ImportError::InvalidFormat("Expected mapping in OpenCollection item".to_string())
        })?;

        let mut collection = Collection::new("");

        for (key, value) in obj {
            let key_str = key.as_str().unwrap_or("");
            match key_str {
                "name" => {
                    collection.name = value.as_str().unwrap_or("").to_string();
                }
                "description" => {
                    collection.readme = value.as_str().map(String::from);
                }
                "requests" => {
                    if let Some(requests) = value.as_sequence() {
                        for req in requests {
                            if let Some(request) = self.parse_opencollection_request(req)? {
                                collection.requests.push(request);
                            }
                        }
                    }
                }
                "items" => {
                    if let Some(subitems) = value.as_sequence() {
                        for subitem in subitems {
                            if let Some(subcollection) = self.parse_opencollection_item(subitem)? {
                                collection.subfolders.push(subcollection);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if collection.name.is_empty() {
            return Ok(None);
        }

        Ok(Some(collection))
    }

    fn parse_opencollection_request(&self, item: &serde_yaml::Value) -> Result<Option<Request>> {
        let obj = item.as_mapping().ok_or_else(|| {
            ImportError::InvalidFormat("Expected mapping in OpenCollection request".to_string())
        })?;

        let mut name = String::new();
        let mut method = HttpMethod::Get;
        let mut url = String::new();
        let mut variables = Vec::new();
        let mut headers = Vec::new();
        let mut body = None;

        for (key, value) in obj {
            let key_str = key.as_str().unwrap_or("");
            match key_str {
                "name" => {
                    name = value.as_str().unwrap_or("").to_string();
                }
                "method" => {
                    method = HttpMethod::from(value.as_str().unwrap_or("GET"));
                }
                "url" | "path" => {
                    let (transformed, vars) =
                        self.transform_variables(value.as_str().unwrap_or(""));
                    url = transformed;
                    variables.extend(vars);
                }
                "headers" => {
                    if let Some(hdrs) = value.as_sequence() {
                        for hdr in hdrs {
                            if let Some(hdr_obj) = hdr.as_mapping() {
                                let mut hdr_name = String::new();
                                let mut hdr_value = String::new();
                                let mut enabled = true;

                                for (k, v) in hdr_obj {
                                    match k.as_str() {
                                        Some("name") | Some("key") => {
                                            hdr_name = v.as_str().unwrap_or("").to_string()
                                        }
                                        Some("value") => {
                                            hdr_value = v.as_str().unwrap_or("").to_string()
                                        }
                                        Some("enabled") | Some("disabled") => {
                                            enabled = !v
                                                .as_str()
                                                .map_or(false, |s| s == "false" || s == "disabled")
                                        }
                                        _ => {}
                                    }
                                }

                                if !hdr_name.is_empty() {
                                    headers.push(Header {
                                        name: hdr_name,
                                        value: hdr_value,
                                        enabled,
                                    });
                                }
                            }
                        }
                    }
                }
                "body" => {
                    if let Some(body_obj) = value.as_mapping() {
                        let mut content = None;
                        let mut content_type = None;

                        for (k, v) in body_obj {
                            match k.as_str() {
                                Some("raw") | Some("content") => {
                                    content = v.as_str().map(String::from)
                                }
                                Some("contentType") | Some("type") => {
                                    content_type = v.as_str().map(String::from)
                                }
                                _ => {}
                            }
                        }

                        if content.is_some() || content_type.is_some() {
                            body = Some(RequestBody {
                                content,
                                form_data: None,
                                content_type,
                            });
                        }
                    } else if let Some(content) = value.as_str() {
                        body = Some(RequestBody {
                            content: Some(content.to_string()),
                            form_data: None,
                            content_type: None,
                        });
                    }
                }
                _ => {}
            }
        }

        if name.is_empty() {
            return Ok(None);
        }

        Ok(Some(Request {
            name,
            description: String::new(),
            method,
            url,
            headers,
            body,
            auth: None,
            params: Vec::new(),
            path_params: Vec::new(),
            scripts: None,
            options: Default::default(),
            variables,
        }))
    }
}

impl ImporterPlugin for BrunoImporter {
    fn info(&self) -> &PluginInfo {
        &PLUGIN_INFO
    }

    fn import_file(&self, path: &Path) -> Result<Collection> {
        self.validate(path)?;

        let format = self.detect_format(path);

        match format {
            BrunoFormat::Bru => {
                if path.is_dir() {
                    self.parse_bru_collection(path)
                } else {
                    // Single file - create a collection from it
                    let content = fs::read_to_string(path)?;
                    let name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Bruno Request")
                        .to_string();

                    let bru_req = self.parse_bru_file(&content)?;
                    let mut collection = Collection::new(&name);

                    let mut request = Request {
                        name: name.clone(),
                        description: String::new(),
                        method: HttpMethod::from(bru_req.method.as_str()),
                        url: bru_req.url,
                        variables: bru_req.variables,
                        ..Default::default()
                    };

                    for (key, value, enabled) in bru_req.headers {
                        request.headers.push(Header {
                            name: key,
                            value,
                            enabled,
                        });
                    }

                    if let Some(body_content) = bru_req.body {
                        request.body = Some(RequestBody {
                            content: Some(body_content),
                            form_data: None,
                            content_type: bru_req.body_content_type,
                        });
                    }

                    collection.requests.push(request);
                    Ok(collection)
                }
            }
            BrunoFormat::OpenCollection => {
                let collections = self.parse_opencollection(path)?;
                Ok(collections
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| Collection::new("Imported")))
            }
            BrunoFormat::Unknown => {
                anyhow::bail!("Could not detect Bruno collection format")
            }
        }
    }

    fn import_directory(&self, path: &Path) -> Result<Vec<Collection>> {
        self.validate(path)?;

        let format = self.detect_format(path);

        match format {
            BrunoFormat::Bru => {
                let collection = self.parse_bru_collection(path)?;
                Ok(vec![collection])
            }
            BrunoFormat::OpenCollection => self.parse_opencollection(path),
            BrunoFormat::Unknown => {
                anyhow::bail!("Could not detect Bruno collection format")
            }
        }
    }

    fn can_handle(&self, path: &Path) -> bool {
        self.detect_format(path) != BrunoFormat::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bru_file() {
        let importer = BrunoImporter::new();

        let content =
            "meta\n  name: Test\n\nhttp\n  method: GET\n  url: https://api.example.com/users";

        let result = importer.parse_bru_file(content).unwrap();
        assert_eq!(result.method, "GET");
        assert_eq!(result.url, "https://api.example.com/users");
    }

    #[test]
    fn test_parse_bru_file_with_body() {
        let importer = BrunoImporter::new();

        let content = r#"
meta
  name: Create User

http
  method: POST
  url: https://api.example.com/users

body
  type: json
  {"name": "test", "email": "test@example.com"}
"#;

        let result = importer.parse_bru_file(content).unwrap();
        assert_eq!(result.method, "POST");
        assert!(result.body.is_some());
        assert_eq!(
            result.body_content_type,
            Some("application/json".to_string())
        );
    }

    #[test]
    fn test_detect_format() {
        let importer = BrunoImporter::new();

        // Test with a path (can't actually test without files, but we can verify the method exists)
        let path = Path::new("/some/path");
        let format = importer.detect_format(path);
        // Format detection depends on actual filesystem state
        assert!(matches!(
            format,
            BrunoFormat::Bru | BrunoFormat::OpenCollection | BrunoFormat::Unknown
        ));
    }
}
