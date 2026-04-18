use viv::mcp::config::{McpConfig, ServerConfig};

#[test]
fn parse_stdio_server() {
    let json = r#"{
        "mcpServers": {
            "my-server": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server"],
                "env": {
                    "API_KEY": "secret123",
                    "DEBUG": "true"
                }
            }
        }
    }"#;
    let config = McpConfig::parse(json).unwrap();
    assert_eq!(config.servers.len(), 1);

    let (name, server) = &config.servers[0];
    assert_eq!(name, "my-server");
    match server {
        ServerConfig::Stdio { command, args, env } => {
            assert_eq!(command, "npx");
            assert_eq!(args, &["-y", "@modelcontextprotocol/server"]);
            assert_eq!(env.len(), 2);
            // env is order-preserving from JSON object
            assert!(env.iter().any(|(k, v)| k == "API_KEY" && v == "secret123"));
            assert!(env.iter().any(|(k, v)| k == "DEBUG" && v == "true"));
        }
        _ => panic!("expected Stdio variant"),
    }
}

#[test]
fn parse_multiple_types() {
    let json = r#"{
        "mcpServers": {
            "local": {
                "type": "stdio",
                "command": "node",
                "args": ["server.js"]
            },
            "remote-sse": {
                "type": "sse",
                "url": "https://example.com/sse"
            },
            "remote-http": {
                "type": "http",
                "url": "https://example.com/mcp"
            },
            "remote-ws": {
                "type": "websocket",
                "url": "wss://example.com/ws"
            }
        }
    }"#;
    let config = McpConfig::parse(json).unwrap();
    assert_eq!(config.servers.len(), 4);

    // Find each server by name
    let find = |name: &str| config.servers.iter().find(|(n, _)| n == name).unwrap();

    let (_, local) = find("local");
    match local {
        ServerConfig::Stdio { command, args, env } => {
            assert_eq!(command, "node");
            assert_eq!(args, &["server.js"]);
            assert!(env.is_empty());
        }
        _ => panic!("expected Stdio"),
    }

    let (_, sse) = find("remote-sse");
    match sse {
        ServerConfig::Sse { url } => assert_eq!(url, "https://example.com/sse"),
        _ => panic!("expected Sse"),
    }

    let (_, http) = find("remote-http");
    match http {
        ServerConfig::Http { url } => assert_eq!(url, "https://example.com/mcp"),
        _ => panic!("expected Http"),
    }

    let (_, ws) = find("remote-ws");
    match ws {
        ServerConfig::WebSocket { url } => assert_eq!(url, "wss://example.com/ws"),
        _ => panic!("expected WebSocket"),
    }
}

#[test]
fn empty_config() {
    let config = McpConfig::parse("{}").unwrap();
    assert!(config.servers.is_empty());
}

fn nonexistent_path(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("viv_test_nonexistent_{}", name))
        .join("settings.json")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn load_nonexistent_file() {
    let config = McpConfig::load(&nonexistent_path("8675309")).unwrap();
    assert!(config.servers.is_empty());
}
