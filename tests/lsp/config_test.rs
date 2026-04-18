use viv::lsp::config::{LspConfig, LspServerConfig};

#[test]
fn parse_single_server() {
    let json = r#"{
        "lspServers": {
            "rust": {
                "command": "rust-analyzer",
                "args": [],
                "extensions": [".rs"]
            }
        }
    }"#;
    let config = LspConfig::parse(json).unwrap();
    assert_eq!(config.servers.len(), 1);

    let (name, server) = &config.servers[0];
    assert_eq!(name, "rust");
    assert_eq!(server.command, "rust-analyzer");
    assert!(server.args.is_empty());
    assert_eq!(server.extensions, vec![".rs"]);
    assert!(server.env.is_empty());
}

#[test]
fn parse_multiple_servers() {
    let json = r#"{
        "lspServers": {
            "rust": {
                "command": "rust-analyzer",
                "args": [],
                "extensions": [".rs"]
            },
            "typescript": {
                "command": "typescript-language-server",
                "args": ["--stdio"],
                "extensions": [".ts", ".tsx"],
                "env": {
                    "NODE_ENV": "production"
                }
            }
        }
    }"#;
    let config = LspConfig::parse(json).unwrap();
    assert_eq!(config.servers.len(), 2);

    let find = |name: &str| -> &LspServerConfig {
        let (_, s) = config.servers.iter().find(|(n, _)| n == name).unwrap();
        s
    };

    let rust = find("rust");
    assert_eq!(rust.command, "rust-analyzer");
    assert!(rust.args.is_empty());
    assert_eq!(rust.extensions, vec![".rs"]);
    assert!(rust.env.is_empty());

    let ts = find("typescript");
    assert_eq!(ts.command, "typescript-language-server");
    assert_eq!(ts.args, vec!["--stdio"]);
    assert_eq!(ts.extensions, vec![".ts", ".tsx"]);
    assert_eq!(ts.env.len(), 1);
    assert!(
        ts.env
            .iter()
            .any(|(k, v)| k == "NODE_ENV" && v == "production")
    );
}

#[test]
fn empty_config() {
    let config = LspConfig::parse("{}").unwrap();
    assert!(config.servers.is_empty());
}

#[test]
fn load_nonexistent_file() {
    let config = LspConfig::load("/tmp/viv_test_nonexistent_lsp_8675309/settings.json").unwrap();
    assert!(config.servers.is_empty());
}

#[test]
fn server_for_extension() {
    let json = r#"{
        "lspServers": {
            "rust": {
                "command": "rust-analyzer",
                "args": [],
                "extensions": [".rs"]
            },
            "typescript": {
                "command": "typescript-language-server",
                "args": ["--stdio"],
                "extensions": [".ts", ".tsx"]
            }
        }
    }"#;
    let config = LspConfig::parse(json).unwrap();

    assert_eq!(config.server_for_extension(".rs"), Some("rust"));
    assert_eq!(config.server_for_extension(".ts"), Some("typescript"));
    assert_eq!(config.server_for_extension(".tsx"), Some("typescript"));
    assert_eq!(config.server_for_extension(".py"), None);
}
