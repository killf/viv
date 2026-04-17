use crate::core::json::JsonValue;
use crate::{Error, Result};
use std::fs;

/// Configuration for all MCP servers loaded from settings.
pub struct McpConfig {
    /// List of (name, config) pairs, preserving order from JSON.
    pub servers: Vec<(String, ServerConfig)>,
}

/// Configuration for a single MCP server.
pub enum ServerConfig {
    Stdio {
        command: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
    Sse {
        url: String,
    },
    Http {
        url: String,
    },
    WebSocket {
        url: String,
    },
}

impl McpConfig {
    /// Load MCP config from a file path.
    ///
    /// Returns empty config (no error) if:
    /// - The file does not exist
    /// - The file has no `mcpServers` field
    pub fn load(path: &str) -> Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(McpConfig { servers: vec![] });
            }
            Err(e) => return Err(Error::Io(e)),
        };
        Self::parse(&contents)
    }

    /// Parse MCP config from a JSON string.
    ///
    /// Returns empty config if there is no `mcpServers` field.
    pub fn parse(json_str: &str) -> Result<Self> {
        let root = JsonValue::parse(json_str)?;

        let servers_obj = match root.get("mcpServers") {
            Some(v) => v,
            None => return Ok(McpConfig { servers: vec![] }),
        };

        let pairs = servers_obj
            .as_object()
            .ok_or_else(|| Error::Json("mcpServers must be an object".to_string()))?;

        let mut servers = Vec::with_capacity(pairs.len());
        for (name, value) in pairs {
            let server = parse_server_config(name, value)?;
            servers.push((name.clone(), server));
        }

        Ok(McpConfig { servers })
    }
}

fn parse_server_config(name: &str, value: &JsonValue) -> Result<ServerConfig> {
    let type_str = value.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
        Error::Json(format!(
            "server '{}': missing or invalid 'type' field",
            name
        ))
    })?;

    match type_str {
        "stdio" => {
            let command = value
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::Json(format!("server '{}': missing 'command' for stdio", name))
                })?
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

            Ok(ServerConfig::Stdio { command, args, env })
        }
        "sse" => {
            let url = parse_url_field(name, value, "sse")?;
            Ok(ServerConfig::Sse { url })
        }
        "http" => {
            let url = parse_url_field(name, value, "http")?;
            Ok(ServerConfig::Http { url })
        }
        "websocket" => {
            let url = parse_url_field(name, value, "websocket")?;
            Ok(ServerConfig::WebSocket { url })
        }
        unknown => Err(Error::Json(format!(
            "server '{}': unknown type '{}'",
            name, unknown
        ))),
    }
}

fn parse_url_field(name: &str, value: &JsonValue, type_name: &str) -> Result<String> {
    value
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            Error::Json(format!(
                "server '{}': missing 'url' for {}",
                name, type_name
            ))
        })
}
