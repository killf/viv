use viv::lsp::config::LspConfig;
use viv::lsp::{LspManager, path_to_uri};

// ---------------------------------------------------------------------------
// path_to_uri
// ---------------------------------------------------------------------------

#[test]
fn path_to_uri_absolute() {
    assert_eq!(path_to_uri("/home/user/project/src/main.rs"), "file:///home/user/project/src/main.rs");
}

#[test]
fn path_to_uri_already_has_scheme() {
    assert_eq!(path_to_uri("file:///home/user/main.rs"), "file:///home/user/main.rs");
}

#[test]
fn path_to_uri_relative() {
    let result = path_to_uri("src/main.rs");
    // relative paths should be resolved to absolute with file:// prefix
    assert!(result.starts_with("file://"), "expected file:// prefix, got: {}", result);
    assert!(result.contains("src/main.rs"), "expected path in uri, got: {}", result);
}

// ---------------------------------------------------------------------------
// LspManager construction
// ---------------------------------------------------------------------------

#[test]
fn empty_config_creates_empty_manager() {
    let config = LspConfig::parse("{}").unwrap();
    let manager = LspManager::new(config);
    assert!(manager.is_empty());
}

#[test]
fn non_empty_config_creates_non_empty_manager() {
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
    let manager = LspManager::new(config);
    assert!(!manager.is_empty());
}

// ---------------------------------------------------------------------------
// server_name_for_file
// ---------------------------------------------------------------------------

#[test]
fn server_for_file_resolves_by_extension() {
    let json = r#"{
        "lspServers": {
            "rust": {
                "command": "rust-analyzer",
                "args": [],
                "extensions": [".rs"]
            },
            "python": {
                "command": "pylsp",
                "args": [],
                "extensions": [".py"]
            }
        }
    }"#;
    let config = LspConfig::parse(json).unwrap();
    let manager = LspManager::new(config);

    assert_eq!(manager.server_name_for_file("src/main.rs"), Some("rust"));
    assert_eq!(manager.server_name_for_file("script.py"), Some("python"));
    assert_eq!(manager.server_name_for_file("style.css"), None);
}

#[test]
fn server_for_file_no_extension() {
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
    let manager = LspManager::new(config);

    assert_eq!(manager.server_name_for_file("Makefile"), None);
}

#[test]
fn server_for_file_absolute_path() {
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
    let manager = LspManager::new(config);

    assert_eq!(manager.server_name_for_file("/workspace/src/lib.rs"), Some("rust"));
}
