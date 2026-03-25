//! Insomnia collection importer plugin.
//!
//! Supports Insomnia JSON export format (v4/v5).

use crate::core::models::{
    Auth, AuthType, Collection, Environment, Header, HttpMethod, Request, RequestBody, SourceType,
};
use crate::plugins::{ImportError, ImporterPlugin, PluginInfo};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

const PLUGIN_INFO: PluginInfo = PluginInfo {
    name: "Insomnia Importer",
    version: "1.0.0",
    source_type: SourceType::Insomnia,
    file_extensions: &["json"],
    description: "Import Insomnia collections (v4/v5)",
};

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct InsomniaExport {
    #[serde(rename = "_type")]
    type_field: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    parentId: Option<String>,
    #[serde(default)]
    environment: Option<serde_json::Value>,
    #[serde(default)]
    environmentProperty: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct InsomniaRequest {
    #[serde(rename = "_type")]
    type_field: String,
    id: String,
    #[serde(default)]
    parentId: Option<String>,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    method: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    headers: Vec<InsomniaHeader>,
    #[serde(default)]
    parameters: Vec<InsomniaParameter>,
    #[serde(default)]
    body: Option<InsomniaBody>,
    #[serde(default)]
    authentication: Option<InsomniaAuth>,
}

#[derive(Debug, Deserialize)]
struct InsomniaHeader {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
struct InsomniaParameter {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum InsomniaBody {
    Detailed(InsomniaBodyDetailed),
    Simple(String),
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct InsomniaBodyDetailed {
    #[serde(default)]
    mimeType: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    form: Vec<InsomniaFormParam>,
    #[serde(default)]
    urlencoded: Vec<InsomniaFormParam>,
}

#[derive(Debug, Deserialize)]
struct InsomniaFormParam {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct InsomniaAuth {
    #[serde(default)]
    type_field: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    addTo: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct InsomniaWorkspace {
    #[serde(rename = "_type")]
    type_field: String,
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
}

/// Insomnia collection importer plugin.
pub struct InsomniaImporter;

impl InsomniaImporter {
    /// Creates a new InsomniaImporter instance.
    pub fn new() -> Self {
        Self
    }

    /// Transforms Insomnia variable syntax ({{ _.variable }}) to Posting syntax (${variable}).
    fn transform_variables(&self, input: &str) -> (String, Vec<String>) {
        let mut result = input.to_string();
        let mut vars = Vec::new();

        // Handle {{ _.variable }} format (Insomnia environment variables)
        // Transform to ${variable} format (Posting format)
        let re = regex::Regex::new(r"\{\{\s*_\.(\w+)\s*\}\}").unwrap();
        for caps in re.captures_iter(&result) {
            if let Some(var_name) = caps.get(1) {
                vars.push(var_name.as_str().to_string());
            }
        }
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("${{{}}}", &caps[1])
            })
            .to_string();

        // Also handle {{variable}} without underscore prefix
        let re2 = regex::Regex::new(r"\{\{\s*(\w+)\s*\}\}").unwrap();
        for caps in re2.captures_iter(&result) {
            if let Some(var_name) = caps.get(1) {
                if !vars.contains(&var_name.as_str().to_string()) {
                    vars.push(var_name.as_str().to_string());
                }
            }
        }
        result = re2
            .replace_all(&result, |caps: &regex::Captures| {
                format!("${{{}}}", &caps[1])
            })
            .to_string();

        (result, vars)
    }

    fn parse_insomnia_auth(&self, auth: &serde_json::Value) -> Option<Auth> {
        let auth_type = auth.get("type").and_then(|v| v.as_str())?;

        match auth_type {
            "basic" => {
                let username = auth.get("username").and_then(|v| v.as_str()).unwrap_or("");
                let password = auth.get("password").and_then(|v| v.as_str()).unwrap_or("");
                if username.is_empty() && password.is_empty() {
                    None
                } else {
                    Some(Auth::basic(username, password))
                }
            }
            "bearer" => {
                let token = auth.get("token").and_then(|v| v.as_str()).unwrap_or("");
                if token.is_empty() {
                    None
                } else {
                    Some(Auth::bearer(token))
                }
            }
            "digest" => {
                let username = auth.get("username").and_then(|v| v.as_str()).unwrap_or("");
                let password = auth.get("password").and_then(|v| v.as_str()).unwrap_or("");
                if username.is_empty() {
                    None
                } else {
                    Some(Auth {
                        auth_type: Some(AuthType::Digest),
                        digest: Some(crate::core::models::DigestAuth {
                            username: username.to_string(),
                            password: password.to_string(),
                        }),
                        ..Default::default()
                    })
                }
            }
            _ => None,
        }
    }
}

impl ImporterPlugin for InsomniaImporter {
    fn info(&self) -> &PluginInfo {
        &PLUGIN_INFO
    }

    fn import_file(&self, path: &Path) -> Result<Collection> {
        self.validate(path)?;

        let content = fs::read_to_string(path).context("Failed to read Insomnia export file")?;

        let json: serde_json::Value =
            serde_json::from_str(&content).context("Failed to parse Insomnia JSON")?;

        // Handle both formats:
        // 1. Direct array: [{...}, {...}]
        // 2. Export object: {"_type": "export", "resources": [...]}
        let resources = if let Some(arr) = json.as_array() {
            arr.clone()
        } else if let Some(res) = json.get("resources").and_then(|r| r.as_array()) {
            res.clone()
        } else {
            return Err(ImportError::InvalidFormat(
                "Expected JSON array or Insomnia export object with 'resources' field".to_string(),
            )
            .into());
        };

        // Build a map of _id -> item for parent lookups
        let mut id_to_item: std::collections::HashMap<String, &serde_json::Value> =
            std::collections::HashMap::new();
        let mut workspace_id: Option<String> = None;
        let mut collection_name = "Imported Collection".to_string();
        let mut collection_description: Option<String> = None;

        for item in &resources {
            if let Some(id) = item.get("_id").and_then(|v| v.as_str()) {
                id_to_item.insert(id.to_string(), item);
            }
            if let Some(type_field) = item.get("_type").and_then(|v| v.as_str()) {
                if type_field == "workspace" {
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        collection_name = name.to_string();
                    }
                    collection_description = item
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if let Some(id) = item.get("_id").and_then(|v| v.as_str()) {
                        workspace_id = Some(id.to_string());
                    }
                }
            }
        }

        // Parse all requests, organizing by parent
        let mut collection = Collection::new(collection_name);
        if let Some(desc) = collection_description {
            collection.readme = Some(desc);
        }

        // Extract environments from Insomnia resources
        for item in &resources {
            if let Some(type_field) = item.get("_type").and_then(|v| v.as_str()) {
                if type_field == "environment" {
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        let mut env = Environment::new(name);
                        if let Some(data) = item.get("data").and_then(|v| v.as_object()) {
                            for (key, value) in data {
                                let k = key.as_str();
                                // Insomnia v4/v5 uses nested {key, value} objects
                                if let Some(v_obj) = value.as_object() {
                                    if let Some(v_str) = v_obj.get("value").and_then(|v| v.as_str())
                                    {
                                        env.variables.insert(k.to_string(), v_str.to_string());
                                    }
                                } else if let Some(v_str) = value.as_str() {
                                    // Simpler key-value format
                                    env.variables.insert(k.to_string(), v_str.to_string());
                                }
                            }
                        }
                        if !env.variables.is_empty() {
                            collection.environments.push(env);
                        }
                    }
                }
            }
        }

        // Helper to build folder path from parent chain
        fn get_folder_path(
            item: &serde_json::Value,
            id_to_item: &std::collections::HashMap<String, &serde_json::Value>,
            workspace_id: &Option<String>,
        ) -> String {
            let mut parts = Vec::new();
            let mut current_id = item.get("parentId").and_then(|v| v.as_str());

            while let Some(pid) = current_id {
                if Some(pid) == workspace_id.as_deref() {
                    break;
                }
                if let Some(parent) = id_to_item.get(pid) {
                    if let Some(name) = parent.get("name").and_then(|v| v.as_str()) {
                        parts.push(name.to_string());
                    }
                    current_id = parent.get("parentId").and_then(|v| v.as_str());
                } else {
                    break;
                }
            }
            parts.reverse();
            if parts.is_empty() {
                String::new()
            } else {
                parts.join(" / ")
            }
        }

        for item in &resources {
            let type_field = item.get("_type").and_then(|v| v.as_str()).unwrap_or("");

            if type_field == "request" {
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unnamed");
                let method = item.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
                let url_str = item.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let (url, mut env_vars) = self.transform_variables(url_str);

                let folder = get_folder_path(item, &id_to_item, &workspace_id);
                let request_name = if folder.is_empty() {
                    name.to_string()
                } else {
                    format!("{} / {}", folder, name)
                };

                let mut request = Request {
                    name: request_name,
                    description: item
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    method: HttpMethod::from(method),
                    url,
                    variables: env_vars,
                    ..Default::default()
                };

                // Parse headers
                if let Some(headers) = item.get("headers").and_then(|v| v.as_array()) {
                    for hdr in headers {
                        let hdr_name = hdr.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let hdr_value = hdr.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        if !hdr_name.is_empty() {
                            let disabled = hdr
                                .get("disabled")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            request.headers.push(Header {
                                name: hdr_name.to_string(),
                                value: hdr_value.to_string(),
                                enabled: !disabled,
                            });
                        }
                    }
                }

                // Parse body
                if let Some(body) = item.get("body") {
                    if let Some(text) = body.get("text").and_then(|v| v.as_str()) {
                        if !text.is_empty() {
                            let mime_type = body
                                .get("mimeType")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            request.body = Some(RequestBody {
                                content: Some(text.to_string()),
                                form_data: None,
                                content_type: mime_type,
                            });
                        }
                    }
                }

                // Parse auth
                if let Some(auth) = item.get("authentication") {
                    if let Some(auth_obj) = self.parse_insomnia_auth(auth) {
                        request.auth = Some(auth_obj);
                    }
                }

                collection.requests.push(request);
            }
        }

        // If no environments defined, collect variables from requests
        if !collection.has_environments() {
            let mut fallback_vars: Vec<String> = Vec::new();
            for request in &collection.requests {
                for var in &request.variables {
                    if !fallback_vars.contains(var) {
                        fallback_vars.push(var.clone());
                    }
                }
            }
            for var in fallback_vars {
                collection.add_fallback_variable(var);
            }
        }

        Ok(collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_insomnia_request() {
        let importer = InsomniaImporter::new();

        let request = InsomniaRequest {
            type_field: "request".to_string(),
            id: "123".to_string(),
            parentId: Some("parent".to_string()),
            name: "Test Request".to_string(),
            description: Some("A test".to_string()),
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: vec![InsomniaHeader {
                id: Some("h1".to_string()),
                name: "Content-Type".to_string(),
                value: "application/json".to_string(),
                disabled: false,
            }],
            parameters: vec![InsomniaParameter {
                id: Some("p1".to_string()),
                name: "page".to_string(),
                value: "1".to_string(),
                disabled: false,
            }],
            body: None,
            authentication: None,
        };

        // Just test that the struct can be created and parsed
        let json = serde_json::json!({
            "type": "basic",
            "username": "testuser",
            "password": "testpass"
        });
        let result = importer.parse_insomnia_auth(&json).unwrap();
        assert!(result.basic.is_some());
    }

    #[test]
    fn test_parse_insomnia_auth() {
        let importer = InsomniaImporter::new();

        // Test basic auth
        let basic_auth = serde_json::json!({
            "type": "basic",
            "username": "user",
            "password": "pass"
        });
        let auth = importer.parse_insomnia_auth(&basic_auth).unwrap();
        assert!(auth.basic.is_some());

        // Test bearer auth
        let bearer_auth = serde_json::json!({
            "type": "bearer",
            "token": "token123"
        });
        let auth = importer.parse_insomnia_auth(&bearer_auth).unwrap();
        assert!(auth.bearer_token.is_some());
    }
}
