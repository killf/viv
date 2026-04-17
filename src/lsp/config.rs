use crate::core::json::JsonValue;
use crate::{Error, Result};
use std::fs;

/// Configuration for a single LSP server.
pub struct LspServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// Configuration for all LSP servers loaded from settings.
pub struct LspConfig {
    /// List of (name, config) pairs, preserving order from JSON.
    pub servers: Vec<(String, LspServerConfig)>,
}

impl LspConfig {
    /// Load LSP config from a file path.
    ///
    /// Returns empty config (no error) if:
    /// - The file does not exist
    /// - The file has no `lspServers` field
    pub fn load(path: &str) -> Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LspConfig { servers: vec![] });
            }
            Err(e) => return Err(Error::Io(e)),
        };
        Self::parse(&contents)
    }

    /// Parse LSP config from a JSON string.
    ///
    /// Returns empty config if there is no `lspServers` field.
    pub fn parse(json_str: &str) -> Result<Self> {
        let root = JsonValue::parse(json_str)?;

        let servers_obj = match root.get("lspServers") {
            Some(v) => v,
            None => return Ok(LspConfig { servers: vec![] }),
        };

        let pairs = servers_obj
            .as_object()
            .ok_or_else(|| Error::Json("lspServers must be an object".to_string()))?;

        let mut servers = Vec::with_capacity(pairs.len());
        for (name, value) in pairs {
            let server = parse_server_config(name, value)?;
            servers.push((name.clone(), server));
        }

        Ok(LspConfig { servers })
    }

    /// Find the name of the LSP server that handles the given file extension.
    ///
    /// Returns `None` if no server is registered for the extension.
    pub fn server_for_extension(&self, ext: &str) -> Option<&str> {
        for (name, config) in &self.servers {
            if config.extensions.iter().any(|e| e == ext) {
                return Some(name.as_str());
            }
        }
        None
    }
}

fn parse_server_config(name: &str, value: &JsonValue) -> Result<LspServerConfig> {
    let command = value
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Json(format!("server '{}': missing 'command'", name)))?
        .to_string();

    let args = match value.get("args") {
        Some(arr_val) => {
            let arr = arr_val.as_array().ok_or_else(|| {
                Error::Json(format!("server '{}': 'args' must be an array", name))
            })?;
            let mut result = Vec::with_capacity(arr.len());
            for item in arr {
                let s = item.as_str().ok_or_else(|| {
                    Error::Json(format!("server '{}': each arg must be a string", name))
                })?;
                result.push(s.to_string());
            }
            result
        }
        None => vec![],
    };

    let extensions = match value.get("extensions") {
        Some(arr_val) => {
            let arr = arr_val.as_array().ok_or_else(|| {
                Error::Json(format!("server '{}': 'extensions' must be an array", name))
            })?;
            let mut result = Vec::with_capacity(arr.len());
            for item in arr {
                let s = item.as_str().ok_or_else(|| {
                    Error::Json(format!(
                        "server '{}': each extension must be a string",
                        name
                    ))
                })?;
                result.push(s.to_string());
            }
            result
        }
        None => vec![],
    };

    let env = match value.get("env") {
        Some(env_val) => {
            let pairs = env_val.as_object().ok_or_else(|| {
                Error::Json(format!("server '{}': 'env' must be an object", name))
            })?;
            let mut result = Vec::with_capacity(pairs.len());
            for (k, v) in pairs {
                let val = v.as_str().ok_or_else(|| {
                    Error::Json(format!(
                        "server '{}': env value for '{}' must be a string",
                        name, k
                    ))
                })?;
                result.push((k.clone(), val.to_string()));
            }
            result
        }
        None => vec![],
    };

    Ok(LspServerConfig {
        command,
        args,
        extensions,
        env,
    })
}
