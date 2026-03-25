//! Postman collection importer plugin.
//!
//! Supports Postman Collection Format v2.0 and v2.1.

use crate::core::models::{
    Auth, AuthType, Collection, Environment, Header, HttpMethod, Request, RequestBody, SourceType,
};
use crate::plugins::{ImporterPlugin, PluginInfo};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const PLUGIN_INFO: PluginInfo = PluginInfo {
    name: "Postman Importer",
    version: "1.0.0",
    source_type: SourceType::Postman,
    file_extensions: &["json"],
    description: "Import Postman collections (v2.0/v2.1)",
};

#[derive(Debug, Deserialize)]
struct PostmanCollection {
    info: PostmanInfo,
    item: Vec<PostmanItem>,
    #[serde(default)]
    variable: Vec<PostmanVariable>,
    #[serde(default)]
    auth: Option<PostmanAuth>,
}

#[derive(Debug, Deserialize)]
struct PostmanInfo {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    schema: String,
}

#[derive(Debug)]
enum PostmanItem {
    Request(PostmanRequest),
    Folder(PostmanFolder),
}

impl<'de> serde::Deserialize<'de> for PostmanItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;

        if value.get("request").is_some() {
            Ok(PostmanItem::Request(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ))
        } else {
            Ok(PostmanItem::Folder(
                serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
struct PostmanFolder {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    item: Vec<PostmanItem>,
    #[serde(default)]
    auth: Option<PostmanAuth>,
    #[serde(default)]
    event: Vec<PostmanEvent>,
}

#[derive(Debug, Deserialize)]
struct PostmanRequest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    request: PostmanRequestDef,
    #[serde(default)]
    response: Vec<()>,
}

#[derive(Debug, Default, Deserialize)]
struct PostmanRequestDef {
    #[serde(default)]
    method: String,
    #[serde(default)]
    header: Vec<PostmanHeader>,
    #[serde(default)]
    url: PostmanUrl,
    #[serde(default)]
    body: Option<PostmanBody>,
    #[serde(default)]
    auth: Option<PostmanAuth>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PostmanUrl {
    String(String),
    Structured(PostmanUrlStructured),
}

impl Default for PostmanUrl {
    fn default() -> Self {
        PostmanUrl::String(String::new())
    }
}

#[derive(Debug, Deserialize)]
struct PostmanUrlStructured {
    raw: String,
    #[serde(default)]
    protocol: Option<String>,
    #[serde(default)]
    host: Vec<String>,
    #[serde(default)]
    path: Vec<String>,
    #[serde(default)]
    query: Vec<PostmanQueryParam>,
}

#[derive(Debug, Deserialize)]
struct PostmanQueryParam {
    key: Option<String>,
    value: Option<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
struct PostmanHeader {
    key: String,
    value: String,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
struct PostmanBody {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    raw: Option<String>,
    #[serde(default)]
    formdata: Vec<PostmanFormData>,
    #[serde(default)]
    urlencoded: Vec<PostmanFormData>,
    #[serde(default)]
    graphql: Option<PostmanGraphql>,
    #[serde(default)]
    binary: Option<()>,
    #[serde(default)]
    options: Option<PostmanBodyOptions>,
}

#[derive(Debug, Default, Deserialize)]
struct PostmanBodyOptions {
    #[serde(default)]
    raw: PostmanBodyRawOptions,
}

#[derive(Debug, Default, Deserialize)]
struct PostmanBodyRawOptions {
    #[serde(default)]
    language: String,
}

#[derive(Debug, Deserialize)]
struct PostmanFormData {
    key: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    type_field: String,
}

#[derive(Debug, Deserialize)]
struct PostmanGraphql {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    variables: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostmanEvent {
    #[serde(default)]
    listen: String,
    #[serde(default)]
    script: Option<PostmanScript>,
}

#[derive(Debug, Deserialize)]
struct PostmanScript {
    #[serde(default)]
    exec: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PostmanVariable {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PostmanAuth {
    NoAuth,
    Empty,
    Basic { basic: PostmanBasicAuth },
    Bearer { bearer: Vec<PostmanToken> },
    Digest { digest: Vec<PostmanKeyValue> },
    ApiKey { apikey: Vec<PostmanKeyValue> },
    Other(HashMap<String, serde_json::Value>),
}

#[derive(Debug, Deserialize)]
struct PostmanBasicAuth {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct PostmanToken {
    value: String,
    #[serde(default)]
    key: String,
}

#[derive(Debug, Deserialize)]
struct PostmanKeyValue {
    key: String,
    value: String,
    #[serde(default)]
    disabled: bool,
}

/// Postman collection importer plugin.
pub struct PostmanImporter;

impl PostmanImporter {
    /// Creates a new PostmanImporter instance.
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

    fn parse_collection(&self, collection: PostmanCollection) -> Collection {
        let mut result = Collection::new(&collection.info.name);

        if let Some(desc) = &collection.info.description {
            if !desc.is_empty() {
                result.readme = Some(desc.clone());
            }
        }

        // Extract Postman collection-level variables as an environment
        if !collection.variable.is_empty() {
            let mut env = Environment::new("default");
            for var in collection.variable {
                if !var.key.is_empty() {
                    env.variables.insert(var.key, var.value);
                }
            }
            if !env.variables.is_empty() {
                result.environments.push(env);
            }
        }

        for item in collection.item {
            self.parse_item(item, &mut result);
        }

        // If no environments defined, collect variables from requests
        if !result.has_environments() {
            let mut fallback_vars: Vec<String> = Vec::new();
            for request in &result.requests {
                for var in &request.variables {
                    if !fallback_vars.contains(var) {
                        fallback_vars.push(var.clone());
                    }
                }
            }
            for var in fallback_vars {
                result.add_fallback_variable(var);
            }
        }

        result
    }

    fn parse_item(&self, item: PostmanItem, collection: &mut Collection) {
        match item {
            PostmanItem::Folder(folder) => {
                let mut subfolder = Collection::new(&folder.name);
                if let Some(desc) = folder.description {
                    if !desc.is_empty() {
                        subfolder.readme = Some(desc);
                    }
                }

                for sub_item in folder.item {
                    self.parse_item(sub_item, &mut subfolder);
                }

                collection.subfolders.push(subfolder);
            }
            PostmanItem::Request(request) => {
                if let Some(parsed) = self.parse_request(request) {
                    collection.requests.push(parsed);
                }
            }
        }
    }

    fn parse_request(&self, request: PostmanRequest) -> Option<Request> {
        let method = HttpMethod::from(request.request.method.as_str());

        let mut vars = Vec::new();
        let url = match request.request.url {
            PostmanUrl::String(s) => {
                let (transformed, extracted) = self.transform_variables(&s);
                vars.extend(extracted);
                transformed
            }
            PostmanUrl::Structured(u) => {
                let (mut transformed_url, extracted) = self.transform_variables(&u.raw);
                vars.extend(extracted);

                if !u.query.is_empty() {
                    let params: Vec<String> = u
                        .query
                        .iter()
                        .filter_map(|q| {
                            if q.disabled {
                                None
                            } else {
                                Some(format!(
                                    "{}={}",
                                    urlencoding::encode(q.key.as_deref().unwrap_or("")),
                                    urlencoding::encode(q.value.as_deref().unwrap_or(""))
                                ))
                            }
                        })
                        .collect();

                    if !params.is_empty() {
                        if transformed_url.contains('?') {
                            transformed_url = format!("{}&{}", transformed_url, params.join("&"));
                        } else {
                            transformed_url = format!("{}?{}", transformed_url, params.join("&"));
                        }
                    }
                }

                transformed_url
            }
        };

        let mut result = Request {
            name: request.name,
            description: request.description.unwrap_or_default(),
            method,
            url,
            variables: vars,
            ..Default::default()
        };

        // Parse headers
        for header in request.request.header {
            result.headers.push(Header {
                name: header.key,
                value: header.value,
                enabled: !header.disabled,
            });
        }

        // Parse body
        if let Some(body) = request.request.body {
            result.body = self.parse_body(body);
        }

        // Parse auth
        if let Some(auth) = request.request.auth {
            result.auth = self.parse_auth(auth);
        }

        // Parse pre-request script
        // Note: Postman scripts would need special handling for Posting's Python scripts

        Some(result)
    }

    fn parse_body(&self, body: PostmanBody) -> Option<RequestBody> {
        match body.mode.as_str() {
            "raw" => {
                let content_type = body
                    .options
                    .as_ref()
                    .and_then(|o| {
                        if o.raw.language == "json" {
                            Some("application/json".to_string())
                        } else if o.raw.language == "xml" {
                            Some("application/xml".to_string())
                        } else if o.raw.language == "text" {
                            Some("text/plain".to_string())
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        body.raw.as_ref().and_then(|raw| {
                            if raw.starts_with('{') || raw.starts_with('[') {
                                Some("application/json".to_string())
                            } else {
                                None
                            }
                        })
                    });

                body.raw.map(|raw| RequestBody {
                    content: Some(raw),
                    content_type,
                    form_data: None,
                })
            }
            "formdata" => {
                if body.formdata.is_empty() {
                    return None;
                }
                Some(RequestBody {
                    content: None,
                    form_data: Some(
                        body.formdata
                            .into_iter()
                            .map(|item| crate::core::models::FormItem {
                                name: item.key,
                                value: item.value.clone(),
                                enabled: !item.disabled,
                            })
                            .collect(),
                    ),
                    content_type: Some("multipart/form-data".to_string()),
                })
            }
            "urlencoded" => {
                if body.urlencoded.is_empty() {
                    return None;
                }
                Some(RequestBody {
                    content: None,
                    form_data: Some(
                        body.urlencoded
                            .into_iter()
                            .map(|item| crate::core::models::FormItem {
                                name: item.key,
                                value: item.value.clone(),
                                enabled: !item.disabled,
                            })
                            .collect(),
                    ),
                    content_type: Some("application/x-www-form-urlencoded".to_string()),
                })
            }
            "graphql" => {
                if let Some(gql) = body.graphql {
                    let mut query = gql.query.unwrap_or_default();
                    if let Some(vars) = gql.variables {
                        if !vars.is_empty() {
                            query = format!("{}\n\nvariables: {}", query, vars);
                        }
                    }
                    Some(RequestBody {
                        content: Some(query),
                        content_type: Some("application/graphql".to_string()),
                        form_data: None,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn parse_auth(&self, auth: PostmanAuth) -> Option<Auth> {
        match auth {
            PostmanAuth::Basic { basic } => Some(Auth::basic(basic.username, basic.password)),
            PostmanAuth::Bearer { bearer } => {
                bearer.first().and_then(|t| Some(Auth::bearer(&t.value)))
            }
            PostmanAuth::Digest { digest } => {
                let username = digest
                    .iter()
                    .find(|kv| kv.key == "username")
                    .map(|kv| kv.value.clone());
                let password = digest
                    .iter()
                    .find(|kv| kv.key == "password")
                    .map(|kv| kv.value.clone());

                if let (Some(u), Some(p)) = (username, password) {
                    Some(Auth {
                        auth_type: Some(AuthType::Digest),
                        digest: Some(crate::core::models::DigestAuth {
                            username: u,
                            password: p,
                        }),
                        ..Default::default()
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl ImporterPlugin for PostmanImporter {
    fn info(&self) -> &PluginInfo {
        &PLUGIN_INFO
    }

    fn import_file(&self, path: &Path) -> Result<Collection> {
        self.validate(path)?;

        let content = fs::read_to_string(path).context("Failed to read Postman collection file")?;

        let collection: PostmanCollection =
            serde_json::from_str(&content).context("Failed to parse Postman collection JSON")?;

        Ok(self.parse_collection(collection))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_postman_collection() {
        let json = r#"{
            "info": {
                "name": "Test Collection",
                "description": "A test collection",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "Get Users",
                    "request": {
                        "method": "GET",
                        "header": [],
                        "url": "https://api.example.com/users"
                    }
                }
            ]
        }"#;

        let collection: PostmanCollection = serde_json::from_str(json).unwrap();
        assert_eq!(collection.info.name, "Test Collection");
        assert_eq!(collection.item.len(), 1);
    }

    #[test]
    fn test_parse_postman_auth() {
        let importer = PostmanImporter::new();

        // Test basic auth
        let basic_auth = PostmanAuth::Basic {
            basic: PostmanBasicAuth {
                username: "user".to_string(),
                password: "pass".to_string(),
            },
        };
        let auth = importer.parse_auth(basic_auth).unwrap();
        assert!(auth.basic.is_some());

        // Test bearer auth
        let bearer_auth = PostmanAuth::Bearer {
            bearer: vec![PostmanToken {
                value: "token123".to_string(),
                key: "".to_string(),
            }],
        };
        let auth = importer.parse_auth(bearer_auth).unwrap();
        assert!(auth.bearer_token.is_some());
    }
}
