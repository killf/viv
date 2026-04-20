pub mod client;
pub mod config;
pub mod jsonrpc;
pub mod tools;
pub mod types;

use std::collections::HashMap;
use std::path::Path;

#[cfg(unix)]
use crate::mcp::transport::stdio::{Framing, StdioTransport};
use crate::{Error, Result};
#[cfg(unix)]
use client::LspClient;
use config::LspConfig;

#[cfg(unix)]
type LspStdioClient = LspClient<StdioTransport>;

// ---------------------------------------------------------------------------
// LspManager
// ---------------------------------------------------------------------------

/// Tracks an open document's version and the LSP client that owns it.
struct OpenDocument {
    /// Server name this document is open in.
    #[allow(dead_code)]
    server_name: String,
    /// Monotonically increasing version. Starts at 1 on `didOpen`, incremented on each `didChange`.
    version: i32,
}

/// Manages LSP server lifecycle with lazy loading.
///
/// Each server is keyed by name (from config). Servers are started on demand
/// when a file is first accessed via [`get_or_start`].
pub struct LspManager {
    config: LspConfig,
    /// `None` = not yet started, `Some(client)` = running.
    #[cfg(unix)]
    pub servers: HashMap<String, Option<LspStdioClient>>,
    #[cfg(not(unix))]
    pub servers: HashMap<String, ()>,
    open_documents: HashMap<String, OpenDocument>,
}

impl LspManager {
    /// Create a new manager; all servers start in the unstarted (`None`) state.
    pub fn new(config: LspConfig) -> Self {
        let mut servers = HashMap::new();
        for (name, _) in &config.servers {
            #[cfg(unix)]
            servers.insert(name.clone(), None);
            #[cfg(not(unix))]
            servers.insert(name.clone(), ());
        }
        LspManager {
            config,
            servers,
            open_documents: HashMap::new(),
        }
    }

    /// Returns `true` if no servers are configured.
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    /// Return the server name responsible for the given file, based on its extension.
    ///
    /// Returns `None` if no configured server handles this file type.
    pub fn server_name_for_file(&self, file: &str) -> Option<&str> {
        let ext = extension_with_dot(file)?;
        self.config.server_for_extension(ext)
    }

    /// Return a mutable reference to the started client for the given file.
    ///
    /// If the server has not been started yet it will be started now.
    /// Returns an error if no server is configured for this file type.
    #[cfg(unix)]
    pub async fn get_or_start(&mut self, file: &str) -> Result<&mut LspStdioClient> {
        let name = self
            .server_name_for_file(file)
            .ok_or_else(|| Error::Lsp {
                server: "<none>".to_string(),
                message: format!("no LSP server configured for file: {}", file),
            })?
            .to_string();

        // Start the server if not yet running.
        if self
            .servers
            .get(&name)
            .map(|v| v.is_none())
            .unwrap_or(false)
        {
            self.start_server(&name).await?;
        }

        self.servers
            .get_mut(&name)
            .and_then(|opt| opt.as_mut())
            .ok_or_else(|| Error::Lsp {
                server: name.clone(),
                message: "failed to start LSP server".to_string(),
            })
    }

    /// Stub for non-unix — LSP over stdio is not yet supported.
    #[cfg(not(unix))]
    pub async fn get_or_start(&mut self, _file: &str) -> Result<&mut ()> {
        Err(Error::Lsp {
            server: "<none>".to_string(),
            message: "LSP over stdio is not yet supported on this platform".to_string(),
        })
    }

    /// Spawn a new LSP server process and perform the initialize handshake.
    #[cfg(unix)]
    pub async fn start_server(&mut self, name: &str) -> Result<()> {
        let server_cfg = self
            .config
            .servers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, cfg)| cfg)
            .ok_or_else(|| Error::Lsp {
                server: name.to_string(),
                message: format!("no configuration for server '{}'", name),
            })?;

        let args: Vec<&str> = server_cfg.args.iter().map(|s| s.as_str()).collect();
        let env: HashMap<String, String> = server_cfg.env.iter().cloned().collect();

        let transport = StdioTransport::spawn_with_framing(
            &server_cfg.command,
            &args,
            &env,
            Framing::ContentLength,
        )?;

        let mut client = LspClient::new(transport, name);

        let cwd = std::env::current_dir()
            .map_err(Error::Io)?
            .to_string_lossy()
            .into_owned();

        client.initialize(&cwd).await?;

        if let Some(slot) = self.servers.get_mut(name) {
            *slot = Some(client);
        }

        Ok(())
    }

    /// Ensure a file is open in the appropriate LSP server.
    ///
    /// Reads the file from disk, sends `textDocument/didOpen`, and records it
    /// so subsequent calls are no-ops.
    #[cfg(unix)]
    pub async fn ensure_file_open(&mut self, file: &str) -> Result<()> {
        let uri = path_to_uri(file);
        if self.open_documents.contains_key(&uri) {
            return Ok(());
        }

        let content = std::fs::read_to_string(file).map_err(Error::Io)?;
        let lang = language_id_from_path(file).to_string();

        let client = self.get_or_start(file).await?;
        client.did_open(&uri, &lang, &content).await?;

        // Record the open document with version=1.
        let server_name = self
            .server_name_for_file(file)
            .ok_or_else(|| Error::Lsp {
                server: "<none>".to_string(),
                message: format!("no configured LSP server for {}", file),
            })?
            .to_string();
        self.open_documents.insert(
            uri,
            OpenDocument {
                server_name,
                version: 1,
            },
        );
        Ok(())
    }

    /// Stub for non-unix — LSP over stdio is not yet supported.
    #[cfg(not(unix))]
    pub async fn ensure_file_open(&mut self, _file: &str) -> Result<()> {
        Err(Error::Lsp {
            server: "<none>".to_string(),
            message: "LSP over stdio is not yet supported on this platform".to_string(),
        })
    }

    /// Gracefully shut down all started servers.
    pub async fn shutdown_all(&mut self) {
        #[cfg(unix)]
        for (name, slot) in self.servers.iter_mut() {
            if let Some(client) = slot.as_mut() {
                if let Err(e) = client.shutdown().await {
                    eprintln!("[lsp] failed to shutdown server '{}': {}", name, e);
                }
            }
            *slot = None;
        }
        self.open_documents.clear();
    }

    /// Notify the appropriate LSP server that a file has changed.
    ///
    /// Reads the current file content from disk, increments the version, and sends
    /// `textDocument/didChange`. If the file has not been opened via `ensure_file_open`
    /// this is a no-op.
    #[cfg(unix)]
    pub async fn notify_did_change(&mut self, file: &str) -> Result<()> {
        let uri = path_to_uri(file);

        // Read updated content from disk before taking any mutable borrows.
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[lsp] failed to read changed file '{}': {}", file, e);
                return Ok(());
            }
        };

        // Increment version and extract all values we need while holding the
        // single mutable borrow of open_documents.  This avoids the borrow
        // checker conflict between get_mut and get_or_start (both need &mut self).
        let (version, uri_clone) = {
            let entry = match self.open_documents.get_mut(&uri) {
                Some(e) => e,
                None => return Ok(()), // file not open, nothing to do
            };
            entry.version += 1;
            (entry.version, uri.clone())
        }; // borrow ends here; self is available for get_or_start

        // Get or start the server.
        let client = self.get_or_start(file).await?;

        client
            .notify_did_change(&uri_clone, &content, version)
            .await?;
        Ok(())
    }

    /// Stub for non-unix — LSP over stdio is not yet supported.
    #[cfg(not(unix))]
    pub async fn notify_did_change(&mut self, _file: &str) -> Result<()> {
        Ok(()) // no-op on non-unix
    }
}

// ---------------------------------------------------------------------------
// Public helper functions
// ---------------------------------------------------------------------------

/// Convert a relative or absolute path to a `file://` URI.
pub fn path_to_uri(path: &str) -> String {
    if path.starts_with("file://") {
        return path.to_string();
    }

    if path.starts_with('/') {
        return format!("file://{}", path);
    }

    // Relative path — resolve against cwd.
    let absolute = std::env::current_dir()
        .map(|d| d.join(path).to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());

    format!("file://{}", absolute)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Return the extension of `path` including the leading dot (e.g. `".rs"`), or
/// `None` when the path has no extension.
fn extension_with_dot(path: &str) -> Option<&str> {
    let p = Path::new(path);
    let ext = p.extension()?.to_str()?;
    // Walk back in the original `path` string to find the dot.
    let dot_pos = path.rfind(&format!(".{}", ext))?;
    Some(&path[dot_pos..dot_pos + 1 + ext.len()])
}

/// Map a file path to an LSP language identifier.
fn language_id_from_path(path: &str) -> &str {
    let p = Path::new(path);
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "java" => "java",
        "rb" => "ruby",
        "sh" | "bash" => "shellscript",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "md" => "markdown",
        "html" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "lua" => "lua",
        "zig" => "zig",
        _ => "plaintext",
    }
}
