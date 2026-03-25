//! Core domain models for the Posting importer.
//!
//! This module defines the intermediate representation (IR) that all
//! importer plugins convert their source formats into. This IR is then
//! used by the Posting format writer to generate `.posting.yaml` files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// HTTP methods supported by Posting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// GET request method
    #[default]
    Get,
    /// POST request method
    Post,
    /// PUT request method
    Put,
    /// DELETE request method
    Delete,
    /// PATCH request method
    Patch,
    /// HEAD request method
    Head,
    /// OPTIONS request method
    Options,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Patch => write!(f, "PATCH"),
            HttpMethod::Head => write!(f, "HEAD"),
            HttpMethod::Options => write!(f, "OPTIONS"),
        }
    }
}

impl From<&str> for HttpMethod {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => HttpMethod::Get,
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "DELETE" => HttpMethod::Delete,
            "PATCH" => HttpMethod::Patch,
            "HEAD" => HttpMethod::Head,
            "OPTIONS" => HttpMethod::Options,
            _ => HttpMethod::Get,
        }
    }
}

/// Authentication types supported by Posting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// No authentication
    #[default]
    None,
    /// Basic authentication
    Basic,
    /// Digest authentication
    Digest,
    /// Bearer token authentication
    BearerToken,
}

impl AuthType {
    /// Returns the string representation of the auth type.
    pub fn as_str(&self) -> &str {
        match self {
            AuthType::None => "",
            AuthType::Basic => "basic",
            AuthType::Digest => "digest",
            AuthType::BearerToken => "bearer_token",
        }
    }
}

/// Basic authentication credentials.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BasicAuth {
    /// Username for basic auth
    pub username: String,
    /// Password for basic auth
    pub password: String,
}

/// Digest authentication credentials.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DigestAuth {
    /// Username for digest auth
    pub username: String,
    /// Password for digest auth
    pub password: String,
}

/// Bearer token authentication.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BearerTokenAuth {
    /// The bearer token value
    pub token: String,
}

/// Authentication configuration for a request.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Auth {
    /// The type of authentication being used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<AuthType>,
    /// Basic authentication credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic: Option<BasicAuth>,
    /// Digest authentication credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<DigestAuth>,
    /// Bearer token authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer_token: Option<BearerTokenAuth>,
}

impl Auth {
    /// Creates a new empty authentication configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates basic authentication with username and password.
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            auth_type: Some(AuthType::Basic),
            basic: Some(BasicAuth {
                username: username.into(),
                password: password.into(),
            }),
            ..Default::default()
        }
    }

    /// Creates bearer token authentication.
    pub fn bearer(token: impl Into<String>) -> Self {
        Self {
            auth_type: Some(AuthType::BearerToken),
            bearer_token: Some(BearerTokenAuth {
                token: token.into(),
            }),
            ..Default::default()
        }
    }

    /// Returns true if no authentication is configured.
    pub fn is_empty(&self) -> bool {
        self.auth_type.is_none()
            && self.basic.is_none()
            && self.digest.is_none()
            && self.bearer_token.is_none()
    }
}

fn default_true() -> bool {
    true
}

/// Request header with optional enable flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    /// Header name
    pub name: String,
    /// Header value
    pub value: String,
    /// Whether the header is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Query parameter with optional enable flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParam {
    /// Parameter name
    pub name: String,
    /// Parameter value
    pub value: String,
    /// Whether the parameter is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Path parameter placeholder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathParam {
    /// Parameter name
    pub name: String,
    /// Parameter value
    pub value: String,
}

/// Form data item with optional enable flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormItem {
    /// Form field name
    pub name: String,
    /// Form field value
    pub value: String,
    /// Whether the field is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Request body configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestBody {
    /// Raw body content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Form data fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_data: Option<Vec<FormItem>>,
    /// Content type of the body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

/// Script paths for request lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Scripts {
    /// Setup script to run before request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    /// Script to run on request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_request: Option<String>,
    /// Script to run on response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_response: Option<String>,
}

/// Request options/configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOptions {
    /// Whether to follow HTTP redirects
    #[serde(default = "default_true")]
    pub follow_redirects: bool,
    /// Whether to verify SSL certificates
    #[serde(default = "default_true")]
    pub verify_ssl: bool,
    /// Whether to attach cookies
    #[serde(default = "default_true")]
    pub attach_cookies: bool,
    /// Proxy URL to use
    #[serde(default)]
    pub proxy_url: String,
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: f64,
}

fn default_timeout() -> f64 {
    5.0
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            follow_redirects: true,
            verify_ssl: true,
            attach_cookies: true,
            proxy_url: String::new(),
            timeout: 5.0,
        }
    }
}

/// A single HTTP request in the intermediate representation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Request {
    /// Request name
    pub name: String,
    /// Request description
    #[serde(default)]
    pub description: String,
    /// HTTP method
    #[serde(default)]
    pub method: HttpMethod,
    /// Request URL
    #[serde(default)]
    pub url: String,
    /// Request body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<RequestBody>,
    /// Authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    /// Request headers
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Header>,
    /// Query parameters
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<QueryParam>,
    /// Path parameters
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_params: Vec<PathParam>,
    /// Request scripts
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<Scripts>,
    /// Request options
    #[serde(default)]
    pub options: RequestOptions,
    /// Variables used in this request (for fallback posting.env)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<String>,
}

impl Request {
    /// Creates a new request with name, method, and URL.
    pub fn new(name: impl Into<String>, method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            method,
            url: url.into(),
            body: None,
            auth: None,
            headers: Vec::new(),
            params: Vec::new(),
            path_params: Vec::new(),
            scripts: None,
            options: RequestOptions::default(),
            variables: Vec::new(),
        }
    }

    /// Adds a variable to the request.
    pub fn add_variable(&mut self, var_name: String) {
        if !self.variables.contains(&var_name) {
            self.variables.push(var_name);
        }
    }

    /// Sets the request body.
    pub fn with_body(mut self, body: RequestBody) -> Self {
        self.body = Some(body);
        self
    }

    /// Sets the request authentication.
    pub fn with_auth(mut self, auth: Auth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Adds a header to the request.
    pub fn add_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push(Header {
            name: name.into(),
            value: value.into(),
            enabled: true,
        });
        self
    }

    /// Adds a query parameter to the request.
    pub fn add_param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.push(QueryParam {
            name: name.into(),
            value: value.into(),
            enabled: true,
        });
        self
    }

    /// Adds a path parameter to the request.
    pub fn add_path_param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.path_params.push(PathParam {
            name: name.into(),
            value: value.into(),
        });
        self
    }
}

/// Environment variables for a collection.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Environment {
    /// Environment name
    pub name: String,
    /// Environment variables
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

impl Environment {
    /// Creates a new empty environment.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            variables: HashMap::new(),
        }
    }

    /// Adds a variable to the environment.
    pub fn add_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables.insert(key.into(), value.into());
        self
    }

    /// Returns a sanitized filename for the environment (e.g., "dev.env", "qa.env").
    pub fn filename(&self) -> String {
        let sanitized = self
            .name
            .to_lowercase()
            .chars()
            .map(|c| match c {
                'a'..='z' | '0'..='9' | '-' | '_' => c,
                ' ' | '.' => '_',
                _ => '_',
            })
            .collect::<String>();
        format!("{}.env", sanitized.trim_matches('_'))
    }

    /// Returns all variable names in this environment.
    pub fn variable_names(&self) -> Vec<&String> {
        self.variables.keys().collect()
    }
}

/// A folder/collection containing requests.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Collection {
    /// Collection name
    pub name: String,
    /// Requests in this collection
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requests: Vec<Request>,
    /// Nested subfolders
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subfolders: Vec<Collection>,
    /// README content
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    /// Environments defined in the collection
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environments: Vec<Environment>,
    /// Variables to include in posting.env when no environments are defined
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallback_variables: Vec<String>,
}

impl Collection {
    /// Creates a new empty collection.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            requests: Vec::new(),
            subfolders: Vec::new(),
            readme: None,
            environments: Vec::new(),
            fallback_variables: Vec::new(),
        }
    }

    /// Sets the collection README.
    pub fn with_readme(mut self, readme: impl Into<String>) -> Self {
        self.readme = Some(readme.into());
        self
    }

    /// Adds a request to the collection.
    pub fn add_request(mut self, request: Request) -> Self {
        self.requests.push(request);
        self
    }

    /// Adds a subfolder to the collection.
    pub fn add_subfolder(mut self, folder: Collection) -> Self {
        self.subfolders.push(folder);
        self
    }

    /// Adds an environment to the collection.
    pub fn add_environment(&mut self, env: Environment) {
        self.environments.push(env);
    }

    /// Adds a fallback variable (for posting.env when no environments exist).
    pub fn add_fallback_variable(&mut self, var_name: String) {
        if !self.fallback_variables.contains(&var_name) {
            self.fallback_variables.push(var_name);
        }
    }

    /// Returns whether the collection has environments defined.
    pub fn has_environments(&self) -> bool {
        !self.environments.is_empty()
    }

    /// Count total requests including nested folders.
    pub fn total_requests(&self) -> usize {
        let mut count = self.requests.len();
        for subfolder in &self.subfolders {
            count += subfolder.total_requests();
        }
        count
    }
}

/// Source type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    /// Postman collection format
    Postman,
    /// Insomnia collection format
    Insomnia,
    /// Bruno collection format
    Bruno,
}

impl SourceType {
    /// Returns the string representation of the source type.
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceType::Postman => "postman",
            SourceType::Insomnia => "insomnia",
            SourceType::Bruno => "bruno",
        }
    }

    /// Parses a source type from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "postman" => Some(SourceType::Postman),
            "insomnia" => Some(SourceType::Insomnia),
            "bruno" => Some(SourceType::Bruno),
            _ => None,
        }
    }
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_method_from_str() {
        assert_eq!(HttpMethod::from("GET"), HttpMethod::Get);
        assert_eq!(HttpMethod::from("post"), HttpMethod::Post);
        assert_eq!(HttpMethod::from("PUT"), HttpMethod::Put);
        assert_eq!(HttpMethod::from("delete"), HttpMethod::Delete);
        assert_eq!(HttpMethod::from("PATCH"), HttpMethod::Patch);
        assert_eq!(HttpMethod::from("unknown"), HttpMethod::Get);
    }

    #[test]
    fn test_auth_basic() {
        let auth = Auth::basic("user", "pass");
        assert_eq!(auth.auth_type, Some(AuthType::Basic));
        assert!(auth.basic.is_some());
        assert_eq!(auth.basic.unwrap().username, "user");
    }

    #[test]
    fn test_auth_bearer() {
        let auth = Auth::bearer("token123");
        assert_eq!(auth.auth_type, Some(AuthType::BearerToken));
        assert!(auth.bearer_token.is_some());
    }

    #[test]
    fn test_auth_is_empty() {
        let empty_auth = Auth::new();
        assert!(empty_auth.is_empty());

        let basic_auth = Auth::basic("user", "pass");
        assert!(!basic_auth.is_empty());
    }

    #[test]
    fn test_request_builder() {
        let request = Request::new(
            "Test Request",
            HttpMethod::Get,
            "https://api.example.com/users",
        )
        .add_header("Accept", "application/json")
        .add_param("page", "1")
        .add_path_param("id", "123");

        assert_eq!(request.name, "Test Request");
        assert_eq!(request.method, HttpMethod::Get);
        assert_eq!(request.headers.len(), 1);
        assert_eq!(request.params.len(), 1);
        assert_eq!(request.path_params.len(), 1);
    }

    #[test]
    fn test_collection_total_requests() {
        let mut collection = Collection::new("Root");
        collection
            .requests
            .push(Request::new("req1", HttpMethod::Get, "url1"));

        let mut subfolder = Collection::new("Subfolder");
        subfolder
            .requests
            .push(Request::new("req2", HttpMethod::Post, "url2"));
        subfolder
            .requests
            .push(Request::new("req3", HttpMethod::Put, "url3"));

        collection.subfolders.push(subfolder);

        assert_eq!(collection.total_requests(), 3);
    }

    #[test]
    fn test_source_type_from_str() {
        assert_eq!(SourceType::from_str("postman"), Some(SourceType::Postman));
        assert_eq!(SourceType::from_str("Postman"), Some(SourceType::Postman));
        assert_eq!(SourceType::from_str("insomnia"), Some(SourceType::Insomnia));
        assert_eq!(SourceType::from_str("bruno"), Some(SourceType::Bruno));
        assert_eq!(SourceType::from_str("unknown"), None);
    }
}
