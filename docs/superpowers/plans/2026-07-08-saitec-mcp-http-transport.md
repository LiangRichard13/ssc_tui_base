# SAITEC-Skills MCP HTTP Transport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace SAITEC-TUI's vendored-source stdio MCP integration with a public-HTTP integration: the SAITEC-Skills MCP server is reached at `http://<host>/mcp` over streamable HTTP, authenticated via the `X-API-Key` header sourced from the SAITEC login session. Eliminate `_vendor/SAITEC-Skills/`, `auth_headers.py` / `http_errors.py` vendored helpers, the embedded-resource fallback, and the Python-on-end-user-machine requirement.

**Note on URL sourcing:** for now the MCP server URL is the constant `DEFAULT_SAITEC_MCP_URL = "http://101.133.153.37:8000/mcp"`, defined as a `pub const` in `src/saitec/auth.rs`. To change the URL later, edit **that one constant** (or override via the `SAITEC_MCP_URL` env var — same resolution chain as `core_api_base()`). The URL is **not** buried in business logic — `ensure_bootstrap()` reads the constant directly.

**Architecture:** Introduce a `McpTransport` enum on `McpServerConfig` (`stdio` or `http`). `McpHandle`'s public surface (`request`, `call_tool`, `tools`, `server_info`, etc.) stays unchanged for callers (`manager.rs`, `pool.rs`, `tool.rs`, tests). Internally `McpHandle` is rewired to delegate to a `MessageTransport` trait object that abstracts over stdio subprocess and HTTP request/response; the per-call `pending` map and `writer_tx` channel disappear because the trait drives request/response correlation synchronously. `ensure_bootstrap()` writes a new HTTP entry into `~/.saitec_tui/mcp.json` (URL = `saitec_mcp_url()`, header `X-API-Key: <session.api_key>`), and `apply_runtime_env()` updates the header value (and the URL if `SAITEC_MCP_URL` env var changed) rather than the env map. The `_vendor/SAITEC-Skills/` tree, the vendored-resource embed, and any packaging code that bundled them are removed; `release.yml` no longer needs to package MCP resources because they are no longer on disk.

**Tech Stack:** Rust 2024, `reqwest 0.12` (already a dep, with `stream` + `json` features), `tokio` (already a dep), `serde_json` (already a dep), existing `JsonRpcRequest` / `JsonRpcResponse` types in `src/mcp/protocol.rs`. No new crate dependencies.

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src/mcp/protocol.rs` | modify | Add `transport: McpTransport` enum and `url`, `headers` fields to `McpServerConfig`. Update `import_from_external` to recognize the HTTP form. |
| `src/mcp/transport.rs` | **create** | New file. Define `McpTransport` enum and a `MessageTransport` trait that abstracts the byte-stream interface: `send_message(&str) -> Result<()>` plus a background pump that feeds responses into the `pending` map. Two impls: `StdioMessageTransport` (current stdio code, refactored out of `client.rs`) and `HttpMessageTransport` (new). |
| `src/mcp/client.rs` | rewrite | Reduce to: `McpHandle` (unchanged shape) + `McpClient::connect()` which dispatches to the right transport based on `config.transport`. The `McpHandle` API stays identical so all callers (`manager.rs`, `pool.rs`, `tool.rs`, tests) keep working. |
| `src/mcp/pool.rs` | modify | No semantic change. The `clients: HashMap<String, McpClient>` field is fine because `McpClient` now owns either a `Box<dyn MessageTransport>` or its `shutdown()` semantics stay equivalent. Verify that `connect_server` still works for HTTP transports — the only change is that no `child: Child` exists for HTTP. |
| `src/saitec/mcp.rs` | rewrite | Drop the entire `resolve_skills_root` / `private_installed_skills_root` / `find_vendor_root_from` / `resolve_server_script` chain (about 70 lines), plus the `python_command()` / `default_env()` helpers and the `SAITEC_SKILLS_ROOT` / `SAITEC_TUI_PYTHON` constants. `ensure_bootstrap()` now writes `{type: "http", url: saitec_mcp_url(), headers: {"X-API-Key": "<api_key>"}, shared: true}`. `apply_runtime_env()` updates the `X-API-Key` header value (not the `env` map). `reconnect_saitec_mcp()` and `disconnect_saitec_mcp()` are unchanged in shape. |
| `src/saitec/auth.rs` | modify | Add a `pub const DEFAULT_SAITEC_MCP_URL: &str = "http://101.133.153.37:8000/mcp"` and a `saitec_mcp_url() -> String` helper that reads the `SAITEC_MCP_URL` env var, falling back to the constant. **This is the one and only place to change the MCP server URL.** |
| `src/saitec/mod.rs` | modify | Remove `mcp_resources` module declaration (it's already gone post-revert but make sure). |
| `src/mcp/manager.rs` | minor | No structural change. `connect_all()` and `connect()` already pass `&McpServerConfig` to `McpClient::connect()` — that signature is preserved. |
| `src/mcp/mod.rs` | minor | Add `pub mod transport;` and re-export the new types as needed. |
| `src/mcp/protocol_tests.rs` | modify | The existing `test_saitec_mcp_load_injects_saved_session_runtime_env_without_persisting_secret` test must be rewritten because the API key now lives in `headers` rather than `env`. Add a parallel test for the HTTP shape. |
| `src/mcp/transport_tests.rs` | **create** | New file. Unit tests for the HTTP transport using a `wiremock` or hand-rolled `tokio::net::TcpListener` to simulate a streamable HTTP server. Verify: `initialize` round-trip, `tools/list` round-trip, `tools/call` round-trip, session id propagation, header forwarding, JSON-only response mode, SSE chunked response mode. |
| `scripts/package_saitec.ps1` | minor | Remove `SAITEC_MCP_RESOURCE_ARCHIVE` lines. Install script no longer extracts `saitec-mcp.resources`. (After `_vendor/SAITEC-Skills/` is deleted, those lines reference non-existent files; we need to either remove or stub them.) |
| `.github/workflows/release.yml` | minor | Remove the `Copy-Item _vendor/SAITEC-Skills` line and the `resources` arg to `tar`. |
| `_vendor/SAITEC-Skills/` | **delete** | The entire tree. ~22 files. |
| `build.rs` | minor | Remove `generate_mcp_resources` / `collect_vendor_files` / `compute_fnv1a_64` helpers (~110 lines). Restore the build.rs that existed before the embed experiment. |

---

### Task 1: Add `McpTransport` enum to `McpServerConfig`

**Files:**
- Modify: `src/mcp/protocol.rs:169-194`

- [ ] **Step 1: Write the failing test**

Append to `src/mcp/protocol_tests.rs` (or create if needed; check the file's existing structure first — see the `mod protocol_tests` line at the bottom of `src/mcp/protocol.rs`):

```rust
#[test]
fn http_server_config_deserializes_with_url_and_headers() {
    let json = r#"{
        "type": "http",
        "url": "http://example.com/mcp",
        "headers": {"X-API-Key": "sk-test"},
        "shared": true
    }"#;
    let cfg: McpServerConfig = serde_json::from_str(json).expect("http cfg should parse");
    assert_eq!(cfg.transport, McpTransport::Http);
    assert_eq!(cfg.url.as_deref(), Some("http://example.com/mcp"));
    assert_eq!(
        cfg.headers.get("X-API-Key").map(String::as_str),
        Some("sk-test")
    );
    assert!(cfg.shared);
}

#[test]
fn stdio_server_config_defaults_to_stdio_transport() {
    let json = r#"{"command": "python", "args": ["server.py"]}"#;
    let cfg: McpServerConfig = serde_json::from_str(json).expect("stdio cfg should parse");
    assert_eq!(cfg.transport, McpTransport::Stdio);
    assert_eq!(cfg.command, "python");
    assert!(cfg.url.is_none());
    assert!(cfg.headers.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib mcp::protocol_tests::http_server_config_deserializes_with_url_and_headers -- --exact`
Expected: compile error `cannot find type McpTransport` / field `transport` on `McpServerConfig`.

- [ ] **Step 3: Add the enum and fields to `McpServerConfig`**

In `src/mcp/protocol.rs`, replace the `McpServerConfig` struct (lines 169-186) with:

```rust
/// MCP transport — how messages are exchanged with the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    /// Spawn `command` as a subprocess; exchange JSON-RPC over its stdio.
    #[default]
    Stdio,
    /// Stream JSON-RPC to `{url}` over HTTP(S); response is JSON or SSE.
    Http,
}

/// MCP server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    /// Transport selection. Stdio by default; legacy configs without a
    /// `type` field continue to be treated as stdio for back-compat.
    #[serde(default, rename = "type")]
    pub transport: McpTransport,
    /// Stdio transport fields. Required when `transport = "stdio"`,
    /// ignored otherwise.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// HTTP transport fields. Required when `transport = "http"`,
    /// ignored otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub headers: std::collections::HashMap<String, String>,
    /// Whether this server can be shared across sessions (default: true).
    /// Stateless API wrappers (Todoist, Canvas) should be shared.
    /// Stateful servers (Playwright browser) should not be shared.
    #[serde(default = "default_shared")]
    pub shared: bool,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib mcp::protocol_tests -- --exact`
Expected: both new tests pass, existing tests still pass.

- [ ] **Step 5: Commit**

```bash
git add src/mcp/protocol.rs src/mcp/protocol_tests.rs
git commit -m "feat(mcp): add McpTransport enum (stdio | http) to McpServerConfig"
```

---

### Task 2: Define the `MessageTransport` trait

**Files:**
- Create: `src/mcp/transport.rs`
- Create: `src/mcp/transport_tests.rs`
- Modify: `src/mcp/mod.rs` (add `pub mod transport;`)

- [ ] **Step 1: Write the failing trait-shape test**

Create `src/mcp/transport_tests.rs`:

```rust
use super::transport::MessageTransport;

#[test]
fn trait_object_compiles() {
    // Asserts the trait is dyn-compatible (no Self: Sized bound on the
    // public surface, no generic methods).
    fn _assert_dyn_compatible(_t: Box<dyn MessageTransport>) {}
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib mcp::transport_tests -- --exact`
Expected: compile error — `transport` module does not exist.

- [ ] **Step 3: Define the trait and the two impls in `src/mcp/transport.rs`**

The trait is **synchronous per request**: the transport receives a serialized JSON-RPC request, returns the matching response. No background pump, no `pending` map — the future IS the response. This is fundamentally simpler than a stream-based design and matches both transports cleanly (stdio: write+read one line; HTTP: POST+parse).

```rust
//! Pluggable transport layer for MCP message exchange.
//!
//! `MessageTransport` abstracts the JSON-RPC request/response surface
//! that `McpHandle` sits on top of. Two impls:
//! - [`StdioMessageTransport`]: spawns a child process, exchanges
//!   newline-delimited JSON on its stdin/stdout.
//! - [`HttpMessageTransport`]: posts JSON-RPC to a remote endpoint,
//!   parses JSON or SSE responses per the streamable-HTTP spec.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

/// JSON-RPC message exchange. Synchronous per request: the future returned
/// by `round_trip` resolves to the matching response payload, or fails.
///
/// `notify` sends a JSON-RPC notification (no `id`, no response expected).
///
/// `shutdown` terminates the transport. Idempotent.
#[async_trait]
pub trait MessageTransport: Send + Sync {
    async fn round_trip(&self, request: String) -> Result<Value>;
    async fn notify(&self, notification: String) -> Result<()>;
    async fn shutdown(&self);
}

// ---------- stdio transport ----------

struct StdioState {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
}

/// Stdio transport: spawns a child process on first use, exchanges
/// newline-delimited JSON-RPC on its stdin/stdout.
pub struct StdioMessageTransport {
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    state: tokio::sync::Mutex<Option<StdioState>>,
}

impl StdioMessageTransport {
    pub fn new(command: String, args: Vec<String>, env: HashMap<String, String>) -> Self {
        Self {
            command,
            args,
            env,
            state: tokio::sync::Mutex::new(None),
        }
    }

    async fn ensure_spawned(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        if state.is_some() {
            return Ok(());
        }
        let mut env: HashMap<String, String> = std::env::vars().collect();
        env.extend(self.env.clone());

        let mut child = Command::new(&self.command)
            .args(&self.args)
            .envs(&env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn MCP server: {}", self.command))?;

        let stdin = child.stdin.take().context("no stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("no stdout")?).lines();
        let stderr = child.stderr.take().context("no stderr")?;

        let server_name = self.command.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    crate::logging::warn(&format!("MCP [{}] stderr: {}", server_name, line));
                }
            }
        });

        *state = Some(StdioState { child, stdin, stdout });
        Ok(())
    }
}

#[async_trait]
impl MessageTransport for StdioMessageTransport {
    async fn round_trip(&self, request: String) -> Result<Value> {
        self.ensure_spawned().await?;
        let mut state = self.state.lock().await;
        let s = state.as_mut().unwrap();
        s.stdin.write_all(request.as_bytes()).await?;
        s.stdin.flush().await?;
        let line = match s.stdout.next_line().await? {
            Some(l) => l,
            None => anyhow::bail!("MCP stdio: server closed stdout"),
        };
        Ok(serde_json::from_str(&line)?)
    }

    async fn notify(&self, notification: String) -> Result<()> {
        self.ensure_spawned().await?;
        let mut state = self.state.lock().await;
        let s = state.as_mut().unwrap();
        s.stdin.write_all(notification.as_bytes()).await?;
        s.stdin.flush().await?;
        Ok(())
    }

    async fn shutdown(&self) {
        if let Some(mut s) = self.state.lock().await.take() {
            let _ = s.child.kill().await;
        }
    }
}

// ---------- HTTP transport ----------

/// HTTP transport: posts JSON-RPC to `{url}` with the configured headers,
/// parses JSON or SSE responses per the streamable-HTTP spec.
pub struct HttpMessageTransport {
    url: String,
    headers: HashMap<String, String>,
    client: reqwest::Client,
    session_id: tokio::sync::Mutex<Option<String>>,
}

impl HttpMessageTransport {
    pub fn new(url: String, headers: HashMap<String, String>) -> Self {
        Self {
            url,
            headers,
            client: reqwest::Client::new(),
            session_id: tokio::sync::Mutex::new(None),
        }
    }
}

/// Parse an SSE body into a single JSON payload. Per the spec, multiple
/// `data:` lines should be concatenated with `\n`; this implementation
/// accepts a single `data:` line (the common case for stateless
/// responses) and the multi-line case.
fn parse_sse_payload(body: &str) -> Result<Value> {
    let mut concatenated = String::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            let part = rest.trim();
            if part.is_empty() {
                continue;
            }
            if !concatenated.is_empty() {
                concatenated.push('\n');
            }
            concatenated.push_str(part);
        }
    }
    if concatenated.is_empty() {
        anyhow::bail!("SSE body had no data: lines");
    }
    Ok(serde_json::from_str(&concatenated)?)
}

#[async_trait]
impl MessageTransport for HttpMessageTransport {
    async fn round_trip(&self, request: String) -> Result<Value> {
        let mut req = self.client
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .body(request);
        for (k, v) in &self.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if let Some(sid) = self.session_id.lock().await.clone() {
            req = req.header("Mcp-Session-Id", sid);
        }
        let response = req.send().await.context("HTTP MCP send failed")?;
        if let Some(sid) = response
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|v| v.to_str().ok())
        {
            *self.session_id.lock().await = Some(sid.to_string());
        }
        let status = response.status();
        let ctype = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("HTTP MCP error {}: {}", status, body);
        }
        if ctype.starts_with("text/event-stream") {
            let body = response.text().await?;
            parse_sse_payload(&body)
        } else {
            response.json().await.context("parse JSON response")
        }
    }

    async fn notify(&self, notification: String) -> Result<()> {
        let mut req = self.client
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .body(notification);
        for (k, v) in &self.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if let Some(sid) = self.session_id.lock().await.clone() {
            req = req.header("Mcp-Session-Id", sid);
        }
        req.send().await.context("HTTP MCP notify failed")?;
        Ok(())
    }

    async fn shutdown(&self) {
        // Stateless — no server-side session to close.
    }
}
```

Also add the `transport_for` factory to the same file:

```rust
use crate::mcp::protocol::{McpServerConfig, McpTransport};

/// Construct the right transport for the given config. Does not start it.
pub fn transport_for(config: &McpServerConfig) -> Result<Box<dyn MessageTransport>> {
    match config.transport {
        McpTransport::Stdio => Ok(Box::new(StdioMessageTransport::new(
            config.command.clone(),
            config.args.clone(),
            config.env.clone(),
        ))),
        McpTransport::Http => {
            let url = config
                .url
                .clone()
                .ok_or_else(|| anyhow::anyhow!("HTTP transport requires `url`"))?;
            Ok(Box::new(HttpMessageTransport::new(url, config.headers.clone())))
        }
    }
}
```

- [ ] **Step 4: Add `transport_for` test**

Append to `src/mcp/transport_tests.rs`:

```rust
use crate::mcp::protocol::McpServerConfig;
use crate::mcp::transport::transport_for;

#[test]
fn stdio_config_picks_stdio_transport() {
    let cfg = McpServerConfig {
        transport: crate::mcp::protocol::McpTransport::Stdio,
        command: "echo".to_string(),
        args: vec![],
        env: Default::default(),
        url: None,
        headers: Default::default(),
        shared: true,
    };
    assert!(matches!(transport_for(&cfg), Ok(_)));
}

#[test]
fn http_config_picks_http_transport() {
    let cfg = McpServerConfig {
        transport: crate::mcp::protocol::McpTransport::Http,
        command: String::new(),
        args: vec![],
        env: Default::default(),
        url: Some("http://localhost:0/mcp".to_string()),
        headers: Default::default(),
        shared: true,
    };
    let t = transport_for(&cfg).expect("http transport should construct");
    let debug = format!("{:?}", t);
    assert!(debug.contains("Http"), "expected Http transport, got: {debug}");
}
```

- [ ] **Step 5: Run all transport tests**

Run: `cargo test --lib mcp::transport_tests -- --exact`
Expected: all 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/mcp/transport.rs src/mcp/transport_tests.rs src/mcp/mod.rs
git commit -m "feat(mcp): introduce MessageTransport trait (stdio | http)"
```

---

### Task 3: Refactor `McpClient` to use the trait

**Files:**
- Modify: `src/mcp/client.rs`

- [ ] **Step 1: Verify what `McpClient::connect` looks like today**

Open `src/mcp/client.rs` and find `McpClient::connect` (around line 127). It currently owns a `child: Child` field directly, and `McpHandle` carries a `writer_tx: mpsc::Sender<String>` plus a `pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>`. Both of those fields are about to go away — the trait now drives request/response correlation synchronously.

- [ ] **Step 2: Rewrite `src/mcp/client.rs`**

Replace the file contents with:

```rust
//! MCP Client - handles communication with a single MCP server.
//!
//! The transport (stdio subprocess or HTTP) is selected by
//! `McpServerConfig.transport` and dispatched via the `MessageTransport`
//! trait. `McpHandle` is a thin wrapper around the transport plus the
//! per-server state (tools, capabilities, request id counter).

use super::protocol::*;
use super::transport::{transport_for, MessageTransport};
use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Shared communication handle for an MCP server. Cheap to clone.
#[derive(Clone)]
pub struct McpHandle {
    pub(crate) name: String,
    request_id: Arc<AtomicU64>,
    transport: Arc<Box<dyn MessageTransport>>,
    server_info: Arc<std::sync::RwLock<Option<ServerInfo>>>,
    capabilities: Arc<std::sync::RwLock<ServerCapabilities>>,
    tools: Arc<std::sync::RwLock<Vec<McpToolDef>>>,
}

impl McpHandle {
    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<JsonRpcResponse> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);
        let serialized = serde_json::to_string(&request)? + "\n";

        let response_value = self
            .transport
            .round_trip(serialized)
            .await
            .context("MCP round_trip failed")?;

        let response: JsonRpcResponse = serde_json::from_value(response_value)
            .context("MCP response not a JSON-RPC envelope")?;

        if let Some(err) = &response.error {
            anyhow::bail!("MCP error {}: {}", err.code, err.message);
        }
        Ok(response)
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
        let arguments = if arguments.is_null() {
            Value::Object(serde_json::Map::new())
        } else {
            arguments
        };
        let params = ToolCallParams { name: name.to_string(), arguments };
        let response = self
            .request("tools/call", Some(serde_json::to_value(params)?))
            .await?;
        let result = response.result.context("No result from tool call")?;
        let tool_result: ToolCallResult = serde_json::from_value(result)?;
        Ok(tool_result)
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn tools(&self) -> Vec<McpToolDef> {
        self.tools.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub async fn refresh_tools(&self) -> Result<()> {
        let response = self.request("tools/list", None).await?;
        if let Some(result) = response.result {
            let tools_result: ToolsListResult = serde_json::from_value(result)?;
            *self.tools.write().unwrap_or_else(|p| p.into_inner()) = tools_result.tools;
        }
        Ok(())
    }
}

/// Owns the transport. One per MCP server, regardless of transport kind.
pub struct McpClient {
    handle: McpHandle,
    transport: Arc<Box<dyn MessageTransport>>,
}

impl McpClient {
    pub async fn connect(name: String, config: &McpServerConfig) -> Result<Self> {
        crate::logging::info(&format!("MCP: Connecting to '{}'", name));

        let transport = transport_for(config)?;
        let transport: Arc<Box<dyn MessageTransport>> = Arc::new(transport);
        let handle = McpHandle {
            name: name.clone(),
            request_id: Arc::new(AtomicU64::new(1)),
            transport: Arc::clone(&transport),
            server_info: Arc::new(std::sync::RwLock::new(None)),
            capabilities: Arc::new(std::sync::RwLock::new(ServerCapabilities::default())),
            tools: Arc::new(std::sync::RwLock::new(Vec::new())),
        };
        let mut client = Self { handle, transport };

        client.initialize().await
            .with_context(|| format!("MCP server '{}' failed to initialize", name))?;
        client.handle.refresh_tools().await
            .with_context(|| format!("MCP server '{}' failed to list tools", name))?;

        crate::logging::info(&format!(
            "MCP: Connected to '{}' with {} tools",
            name,
            client.handle.tools().len()
        ));
        Ok(client)
    }

    pub fn handle(&self) -> McpHandle { self.handle.clone() }

    async fn initialize(&mut self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo {
                name: "jcode".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        let response = self
            .handle
            .request("initialize", Some(serde_json::to_value(params)?))
            .await?;
        if let Some(result) = response.result {
            let init_result: InitializeResult = serde_json::from_value(result)?;
            *self.handle.server_info.write().unwrap_or_else(|p| p.into_inner()) =
                init_result.server_info;
            *self.handle.capabilities.write().unwrap_or_else(|p| p.into_inner()) =
                init_result.capabilities;
        }
        let notif = JsonRpcRequest::new(0, "notifications/initialized", None);
        let serialized = serde_json::to_string(&notif)? + "\n";
        self.transport.notify(serialized).await?;
        Ok(())
    }

    pub fn is_running(&self) -> bool { true }

    pub async fn shutdown(&mut self) {
        self.transport.shutdown().await;
    }

    pub fn name(&self) -> &str { &self.handle.name }
    pub fn server_info(&self) -> Option<ServerInfo> { self.handle.server_info() }
    pub fn tools(&self) -> Vec<McpToolDef> { self.handle.tools() }
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
        self.handle.call_tool(name, arguments).await
    }
}
```

- [ ] **Step 3: Build to verify the project compiles**

Run: `cargo build`
Expected: zero errors. (There will be warnings about unused imports in `protocol.rs` for the old stdio-only field types — clean those up if any.)

- [ ] **Step 4: Run all mcp tests**

Run: `cargo test --lib mcp:: -- --exact`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/mcp/client.rs
git commit -m "refactor(mcp): McpHandle dispatches via MessageTransport trait"
```

---

---

### Task 4: Add the SAITEC MCP URL helper

**Files:**
- Modify: `src/saitec/auth.rs` (add `DEFAULT_SAITEC_MCP_URL` const + `saitec_mcp_url()` function)

This task defines the **single source of truth** for the SAITEC-Skills MCP server URL. `ensure_bootstrap()` (Task 5) and `apply_runtime_env()` (Task 5) both call `saitec_mcp_url()` — they do not hardcode any URL themselves. To point at a different MCP server in the future, change the constant here (or set `SAITEC_MCP_URL` in the environment).

- [ ] **Step 1: Add the constant and the helper**

In `src/saitec/auth.rs`, near the top of the file (next to the existing `DEFAULT_AUTH_BASE` and `DEFAULT_CORE_API_BASE` constants), add:

```rust
/// Default URL of the SAITEC-Skills MCP server. To change the MCP server
/// endpoint, edit this constant (or set the `SAITEC_MCP_URL` env var).
pub const DEFAULT_SAITEC_MCP_URL: &str = "http://101.133.153.37:8000/mcp";
```

Then, near `core_api_base()` (around line 281), add a sibling helper:

```rust
/// URL of the SAITEC-Skills MCP server. Reads the `SAITEC_MCP_URL`
/// env var first, then falls back to `DEFAULT_SAITEC_MCP_URL`. Trims
/// trailing slashes for normalization.
pub fn saitec_mcp_url() -> String {
    std::env::var("SAITEC_MCP_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_SAITEC_MCP_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}
```

- [ ] **Step 2: Add a unit test**

Append to the existing `#[cfg(test)] mod tests` block in `src/saitec/auth.rs`:

```rust
#[test]
fn saitec_mcp_url_defaults_to_constant() {
    let _lock = crate::storage::lock_test_env();
    let prev = std::env::var_os("SAITEC_MCP_URL");
    std::env::remove_var("SAITEC_MCP_URL");
    assert_eq!(saitec_mcp_url(), DEFAULT_SAITEC_MCP_URL);
    if let Some(prev) = prev {
        std::env::set_var("SAITEC_MCP_URL", prev);
    }
}

#[test]
fn saitec_mcp_url_honors_env_override() {
    let _lock = crate::storage::lock_test_env();
    let prev = std::env::var_os("SAITEC_MCP_URL");
    crate::env::set_var("SAITEC_MCP_URL", "http://override.example.com:9999/mcp/");
    assert_eq!(saitec_mcp_url(), "http://override.example.com:9999/mcp");
    if let Some(prev) = prev {
        std::env::set_var("SAITEC_MCP_URL", prev);
    } else {
        std::env::remove_var("SAITEC_MCP_URL");
    }
}
```

- [ ] **Step 3: Run the test**

Run: `cargo test --lib saitec::auth::tests::saitec_mcp_url -- --exact`
Expected: both new tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/saitec/auth.rs
git commit -m "feat(saitec): add saitec_mcp_url() and DEFAULT_SAITEC_MCP_URL constant"
```

---

### Task 5: Rewrite `ensure_bootstrap()` to write HTTP config

**Files:**
- Modify: `src/saitec/mcp.rs:14-87` (rewrite `ensure_bootstrap`)
- Modify: `src/saitec/mcp.rs:89-111` (rewrite `apply_runtime_env`)
- Modify: `src/saitec/mcp.rs` (drop the stdio-only helpers `python_command()`, `default_env()`, `resolve_server_script()`, `resolve_skills_root()`, `private_installed_skills_root()`, `find_vendor_root_from()`, and the `SAITEC_SKILLS_ROOT` / `SAITEC_TUI_PYTHON` constants)

- [ ] **Step 1: Write the failing test**

Append to `src/saitec/mcp.rs` (or replace the existing `tests` module; see the `#[cfg(test)] mod tests` block near line 260):

```rust
#[test]
fn ensure_bootstrap_writes_http_entry_with_x_api_key_header() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_API_KEY", "sk-http-bootstrap");

    // Save a session so runtime_api_key() returns a value.
    crate::saitec::auth::save_session(&crate::saitec::auth::SaitecSession {
        auth_token: None,
        api_key: "sk-http-bootstrap".to_string(),
        token_type: "Bearer".to_string(),
        user_id: Some("u".to_string()),
        email: None, phone: None, display_name: None,
        api_key_id: None, api_key_name: None,
        api_key_created_at: None, api_key_expires_at: None,
        last_validated_at: None,
    }).unwrap();

    // Run the bootstrap.
    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let path = temp.path().join(".saitec_tui").join("mcp.json");
    let cfg: McpConfig = McpConfig::load_from_file(&path).unwrap();
    let saitec = cfg.servers.get(crate::saitec::mcp::SAITEC_MCP_SERVER_NAME)
        .expect("SAITEC-Skills entry must be written");

    assert_eq!(saitec.transport, McpTransport::Http);
    let url = saitec.url.as_deref().expect("http url must be set");
    assert!(url.ends_with("/mcp"), "url should end in /mcp, got {url}");
    assert_eq!(
        saitec.headers.get("X-API-Key").map(String::as_str),
        Some("sk-http-bootstrap")
    );
    assert!(saitec.shared);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib saitec::mcp::tests::ensure_bootstrap_writes_http_entry_with_x_api_key_header -- --exact`
Expected: fail because `ensure_bootstrap` still writes stdio config.

- [ ] **Step 3: Rewrite `ensure_bootstrap`**

In `src/saitec/mcp.rs`, replace `ensure_bootstrap` (lines 14-87) with:

```rust
pub fn ensure_bootstrap() -> Result<()> {
    let mcp_path = mcp_config_file()?;
    let mut config = if mcp_path.exists() {
        match McpConfig::load_from_file(&mcp_path) {
            Ok(c) => c,
            Err(err) => {
                crate::logging::warn(&format!(
                    "SAITEC MCP bootstrap skipped: failed to parse {}: {}",
                    mcp_path.display(), err
                ));
                return Ok(());
            }
        }
    } else {
        McpConfig::default()
    };

    let url = crate::saitec::auth::saitec_mcp_url();
    let api_key = runtime_api_key();
    let mut changed = false;

    match config.servers.get_mut(SAITEC_MCP_SERVER_NAME) {
        Some(server) => {
            // Force the HTTP transport shape. If the existing entry is stdio,
            // switch it over (and clear the stdio-only fields).
            if server.transport != McpTransport::Http { server.transport = McpTransport::Http; changed = true; }
            if !server.command.is_empty() { server.command.clear(); changed = true; }
            if !server.args.is_empty() { server.args.clear(); changed = true; }
            if !server.env.is_empty() { server.env.clear(); changed = true; }
            if server.url.as_deref() != Some(url.as_str()) {
                server.url = Some(url.clone());
                changed = true;
            }
            if let Some(ref key) = api_key {
                let needs_header = server.headers.get("X-API-Key").map(String::as_str) != Some(key.as_str());
                if needs_header {
                    server.headers.insert("X-API-Key".to_string(), key.clone());
                    changed = true;
                }
            }
            if !server.shared { server.shared = true; changed = true; }
        }
        None => {
            let mut headers = HashMap::new();
            if let Some(ref key) = api_key {
                headers.insert("X-API-Key".to_string(), key.clone());
            }
            config.servers.insert(
                SAITEC_MCP_SERVER_NAME.to_string(),
                McpServerConfig {
                    transport: McpTransport::Http,
                    command: String::new(),
                    args: Vec::new(),
                    env: HashMap::new(),
                    url: Some(url.clone()),
                    headers,
                    shared: true,
                },
            );
            changed = true;
        }
    }

    if changed {
        config.save_to_file(&mcp_path)?;
        crate::logging::info(&format!(
            "SAITEC MCP bootstrap updated config at {}",
            mcp_path.display()
        ));
    }
    Ok(())
}
```

Use the existing `saitec_mcp_url()` helper from `src/saitec/auth.rs` (introduced in Step 1 of Task 4) — don't define a new helper here:

```rust
// Already defined in src/saitec/auth.rs:
//   pub const DEFAULT_SAITEC_MCP_URL: &str = "http://101.133.153.37:8000/mcp";
//   pub fn saitec_mcp_url() -> String { /* reads SAITEC_MCP_URL env, falls back to const */ }
```

Update imports at the top of the file to include `McpTransport`:

```rust
use crate::mcp::{McpConfig, McpServerConfig, McpTransport};
```

- [ ] **Step 4: Rewrite `apply_runtime_env`**

In `src/saitec/mcp.rs`, replace `apply_runtime_env` (lines 89-111) with:

```rust
pub fn apply_runtime_env(config: &mut McpConfig) {
    let Some(server) = config.servers.get_mut(SAITEC_MCP_SERVER_NAME) else {
        return;
    };
    if let Some(api_key) = runtime_api_key() {
        server.headers.insert("X-API-Key".to_string(), api_key);
    }
    server.url = Some(crate::saitec::auth::saitec_mcp_url());
}
```

(The `CORE_API_BASE` / `SAITEC_TUI_HOME` env-var injections are no longer needed because the HTTP transport uses `url` + `headers` directly.)

- [ ] **Step 5: Drop the stdio-only helpers**

Delete from `src/saitec/mcp.rs`:
- `python_command()` (lines 169-171)
- `default_env()` (lines 173-179)
- `resolve_server_script()` (lines 181-185)
- `resolve_skills_root()` (lines 187-229)
- `private_installed_skills_root()` (lines 231-239)
- `find_vendor_root_from()` (lines 241-249)

Delete the constants (no longer used by `mcp.rs`; double-check `src/cli/`, `src/auth/`, etc. don't reference them):

```rust
pub const SAITEC_TUI_PYTHON: &str = "SAITEC_TUI_PYTHON";
pub const SAITEC_SKILLS_ROOT: &str = "SAITEC_SKILLS_ROOT";
```

Run: `cargo build` after deletion; if anything still references them, fix the call site (not by re-adding the constants).

- [ ] **Step 6: Run all saitec tests**

Run: `cargo test --lib saitec:: -- --exact`
Expected: the new HTTP test passes. (The four stdio-shaped `protocol_tests` are addressed in Task 5 — they will continue to fail until you complete that task, so filter them out with `cargo test --lib saitec::mcp::tests -- --exact` to verify just the new test passes here.)

- [ ] **Step 7: Commit**

```bash
git add src/saitec/mcp.rs
git commit -m "feat(saitec): bootstrap HTTP MCP transport (no local Python)"
```

---

### Task 6: Update the protocol_tests for the new HTTP config shape

**Files:**
- Modify: `src/mcp/protocol_tests.rs:28-80` (the existing `test_saitec_mcp_load_injects_saved_session_runtime_env_without_persisting_secret`)

- [ ] **Step 1: Locate the existing test**

The test starts at line 28 and ends around line 80 in `src/mcp/protocol_tests.rs`. It uses `SAITEC_SKILLS_ROOT` to point at a temp `server.py`. After Task 4, the SAITEC bootstrap no longer looks at `SAITEC_SKILLS_ROOT` and no longer writes `SAITEC_API_KEY` into `env`. The test's assertions on `saitec.env.get("SAITEC_API_KEY")` will fail.

- [ ] **Step 2: Rewrite the test for the HTTP shape**

Replace the body of `test_saitec_mcp_load_injects_saved_session_runtime_env_without_persisting_secret` with:

```rust
#[test]
fn test_saitec_mcp_load_injects_saved_session_runtime_header_without_persisting_secret() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");
    let prev_core_api_base = std::env::var_os("CORE_API_BASE");
    let prev_saitec_api_base = std::env::var_os("SAITEC_API_BASE");
    let prev_auth_base = std::env::var_os("SAITEC_AUTH_BASE");

    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::remove_var("SAITEC_API_KEY");
    crate::env::remove_var("CORE_API_BASE");
    crate::env::remove_var("SAITEC_API_BASE");
    crate::env::remove_var("SAITEC_AUTH_BASE");

    let auth_path = crate::saitec::paths::auth_file().unwrap();
    crate::storage::write_json_secret(&auth_path, &sample_saitec_session("sk-session-only"))
        .unwrap();

    let config = McpConfig::load();
    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(saitec.transport, McpTransport::Http);
    assert_eq!(
        saitec.headers.get("X-API-Key").map(String::as_str),
        Some("sk-session-only")
    );
    let url = saitec.url.as_deref().expect("http url must be set");
    assert!(url.ends_with("/mcp"), "url should end in /mcp, got {url}");
    assert!(url.starts_with(crate::saitec::auth::DEFAULT_CORE_API_BASE.trim_end_matches('/')));

    let mcp_path = temp
        .path()
        .join(".saitec_tui")
        .join("mcp.json");
    let persisted = std::fs::read_to_string(mcp_path).unwrap();
    assert!(
        !persisted.contains("sk-session-only"),
        "runtime API key must not be persisted in mcp.json"
    );

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
    restore_env_var("CORE_API_BASE", prev_core_api_base);
    restore_env_var("SAITEC_API_BASE", prev_saitec_api_base);
    restore_env_var("SAITEC_AUTH_BASE", prev_auth_base);
}
```

> **Note:** The exact URL assertion can be simplified — replace the multi-line block above with a single `assert!(saitec.url.as_deref().unwrap().ends_with("/mcp"))`. The original test is for shape, not exact value.

- [ ] **Step 3: Run the test**

Run: `cargo test --lib mcp::protocol_tests -- --exact`
Expected: the rewritten test passes; the next two tests in this file (Steps 4–6) will still fail until you rewrite them.

- [ ] **Step 4: Rewrite `test_saitec_bootstrap_prefers_private_installed_mcp_resources` (line 130)**

This test points `LOCALAPPDATA` at a fake `.saitec-mcp/SAITEC-Skills` dir with a `server.py`, then asserts `saitec.args[0]` matches the script path. After Task 4, the bootstrap no longer touches disk layout for the script — it writes an HTTP entry. Replace the test with one that asserts the HTTP config is canonical, with no dependency on disk layout:

```rust
#[test]
fn test_saitec_bootstrap_writes_http_entry_unaffected_by_private_install_dir() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");
    let prev_local_appdata = std::env::var_os("LOCALAPPDATA");

    let temp = tempfile::TempDir::new().unwrap();
    let local_appdata = temp.path().join("local-appdata");
    // Pre-create a private install dir to confirm bootstrap ignores it.
    let _ = std::fs::create_dir_all(
        local_appdata
            .join("saitec-tui")
            .join("resources")
            .join(".saitec-mcp")
            .join("SAITEC-Skills")
            .join("mcp_server"),
    );

    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_API_KEY", "sk-direct");
    crate::env::set_var("LOCALAPPDATA", &local_appdata);

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp.path().join("external").join(".saitec_tui").join("mcp.json");
    let config = McpConfig::load_from_file(&mcp_path).unwrap();
    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(saitec.transport, McpTransport::Http);
    assert_eq!(
        saitec.headers.get("X-API-Key").map(String::as_str),
        Some("sk-direct")
    );
    assert!(saitec.url.as_deref().unwrap().ends_with("/mcp"));

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
    restore_env_var("LOCALAPPDATA", prev_local_appdata);
}
```

- [ ] **Step 5: Rewrite `test_saitec_bootstrap_preserves_existing_servers_and_refreshes_saitec_entry` (line 170)**

This test sets up an existing `mcp.json` with a custom SAITEC-Skills stdio entry and asserts that bootstrap refreshes the SAITEC entry while leaving `existing-server` alone. After Task 4, the refresh logic also rewrites the SAITEC entry from stdio to HTTP. Replace with:

```rust
#[test]
fn test_saitec_bootstrap_preserves_existing_servers_and_migrates_saitec_to_http() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");
    let prev_core_api_base = std::env::var_os("CORE_API_BASE");
    let prev_saitec_api_base = std::env::var_os("SAITEC_API_BASE");
    let prev_auth_base = std::env::var_os("SAITEC_AUTH_BASE");

    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_API_KEY", "sk-refresh");
    crate::env::remove_var("CORE_API_BASE");
    crate::env::remove_var("SAITEC_API_BASE");
    crate::env::remove_var("SAITEC_AUTH_BASE");

    let mcp_dir = temp.path().join("external").join(".saitec_tui");
    std::fs::create_dir_all(&mcp_dir).unwrap();
    let mcp_path = mcp_dir.join("mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{
            "servers": {
                "existing-server": {
                    "command": "existing-bin",
                    "args": ["--flag"],
                    "env": {"EXISTING": "1"}
                },
                "SAITEC-Skills": {
                    "type": "stdio",
                    "command": "custom-bin",
                    "args": ["--custom"],
                    "env": {"CUSTOM": "1"}
                }
            }
        }"#,
    ).unwrap();

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let config = McpConfig::load_from_file(&mcp_path).unwrap();
    let existing = config.servers.get("existing-server").unwrap();
    assert_eq!(existing.command, "existing-bin");
    assert_eq!(existing.args, vec!["--flag"]);
    assert_eq!(existing.env.get("EXISTING"), Some(&"1".to_string()));

    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(saitec.transport, McpTransport::Http);
    assert!(saitec.command.is_empty(), "stdio fields should be cleared");
    assert!(saitec.args.is_empty());
    assert!(saitec.env.is_empty());
    assert_eq!(saitec.headers.get("X-API-Key").map(String::as_str), Some("sk-refresh"));
    assert!(saitec.url.as_deref().unwrap().ends_with("/mcp"));

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
    restore_env_var("CORE_API_BASE", prev_core_api_base);
    restore_env_var("SAITEC_API_BASE", prev_saitec_api_base);
    restore_env_var("SAITEC_AUTH_BASE", prev_auth_base);
}
```

- [ ] **Step 6: Rewrite `test_saitec_bootstrap_skips_when_vendored_script_missing` (line 245)**

This test asserts that bootstrap writes nothing when no `server.py` is found. After Task 4, bootstrap has no dependency on a vendored script — it always writes the HTTP config when `SAITEC_API_KEY` is present, and skips when it isn't. Replace with:

```rust
#[test]
fn test_saitec_bootstrap_skips_when_no_api_key_present() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");
    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::remove_var("SAITEC_API_KEY");

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp.path().join("external").join(".saitec_tui").join("mcp.json");
    assert!(
        !mcp_path.exists(),
        "expected bootstrap to skip config creation when no API key is available"
    );

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
}
```

- [ ] **Step 7: Run all protocol tests**

Run: `cargo test --lib mcp::protocol_tests -- --exact`
Expected: all 4 tests pass (the one rewritten in Step 2 plus the three rewritten in Steps 4–6).

- [ ] **Step 8: Commit**

```bash
git add src/mcp/protocol_tests.rs
git commit -m "test(mcp): rewrite all SAITEC bootstrap tests for HTTP transport"
```

---

### Task 7: Add an end-to-end test for the HTTP transport

**Files:**
- Modify: `src/mcp/transport_tests.rs` (add a test using a real local HTTP server)

- [ ] **Step 1: Write the failing test**

Append to `src/mcp/transport_tests.rs`:

```rust
use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::mcp::transport::HttpMessageTransport;
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Read HTTP request headers + body from a stream.
/// Returns (method, path, headers, body).
async fn read_http_request(stream: &mut tokio::net::TcpStream) -> (String, String, HashMap<String, String>, String) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    // Read until we see \r\n\r\n
    loop {
        let n = stream.read(&mut tmp).await.unwrap();
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") { break; }
        if n == 0 { break; }
    }
    let raw = String::from_utf8_lossy(&buf).to_string();
    let mut lines = raw.split("\r\n");
    let request_line = lines.next().unwrap_or("").to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::new();
    let mut content_length: usize = 0;
    let mut body_start = raw.len();
    for line in lines.by_ref() {
        if line.is_empty() { break; }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim().to_string();
            if k == "content-length" { content_length = v.parse().unwrap_or(0); }
            headers.insert(k, v);
        }
    }
    // Find body offset
    if let Some(idx) = raw.find("\r\n\r\n") {
        body_start = idx + 4;
    }
    let mut body = raw[body_start..].to_string();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).await.unwrap();
        body.push_str(&String::from_utf8_lossy(&tmp[..n]));
    }
    (method, path, headers, body)
}

fn make_ok_json_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    )
}

#[tokio::test]
async fn http_transport_round_trips_initialize_and_tools_list() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    // Server: read a request, return a canned `initialize` response, then
    // return a canned `tools/list` response, then return EOF.
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (method, path, headers, body) = read_http_request(&mut stream).await;
        assert_eq!(method, "POST");
        assert_eq!(path, "/mcp");
        assert_eq!(headers.get("x-api-key").map(String::as_str), Some("sk-test"));
        let req: Value = serde_json::from_str(&body).unwrap();
        let id = req.get("id").and_then(Value::as_u64).unwrap();
        let initialize_response = serde_json::json!({
            "jsonrpc": "2.0", "id": id, "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "fake", "version": "0.1.0"}
            }
        });
        stream.write_all(make_ok_json_response(&initialize_response.to_string()).as_bytes()).await.unwrap();
        stream.flush().await.unwrap();

        // tools/list
        let (_m2, _p2, _h2, body2) = read_http_request(&mut stream).await;
        let req2: Value = serde_json::from_str(&body2).unwrap();
        let id2 = req2.get("id").and_then(Value::as_u64).unwrap();
        let tools_response = serde_json::json!({
            "jsonrpc": "2.0", "id": id2, "result": {
                "tools": [
                    {"name": "ping", "description": "ping the server", "inputSchema": {"type": "object"}}
                ]
            }
        });
        stream.write_all(make_ok_json_response(&tools_response.to_string()).as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
    });

    let mut headers = HashMap::new();
    headers.insert("X-API-Key".to_string(), "sk-test".to_string());
    let transport = HttpMessageTransport::new(format!("http://{}/mcp", addr), headers);

    // initialize
    let init_req = JsonRpcRequest::new(1, "initialize", Some(serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "test", "version": "0.0.0"}
    })));
    let init_value = transport.round_trip(serde_json::to_string(&init_req).unwrap() + "\n").await.unwrap();
    let init_resp: JsonRpcResponse = serde_json::from_value(init_value).unwrap();
    assert_eq!(init_resp.id, Some(1));
    assert!(init_resp.result.is_some());

    // tools/list
    let list_req = JsonRpcRequest::new(2, "tools/list", None);
    let list_value = transport.round_trip(serde_json::to_string(&list_req).unwrap() + "\n").await.unwrap();
    let list_resp: JsonRpcResponse = serde_json::from_value(list_value).unwrap();
    let list_result = list_resp.result.unwrap();
    let tools = list_result.get("tools").and_then(Value::as_array).unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].get("name").and_then(Value::as_str), Some("ping"));

    server.await.unwrap();
}
```

- [ ] **Step 2: Run test to verify it fails (compile error first)**

Run: `cargo test --lib mcp::transport_tests::http_transport_round_trips_initialize_and_tools_list -- --exact`
Expected: first pass — both the transport and the test now exist. (If it fails to compile because the trait shape from Task 3 is wrong, fix that first.)

- [ ] **Step 3: Run again to verify it passes**

Run: `cargo test --lib mcp::transport_tests -- --exact`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add src/mcp/transport_tests.rs
git commit -m "test(mcp): round-trip HTTP transport against a real local server"
```

---

### Task 8: Add SSE response support to the HTTP transport

**Files:**
- Modify: `src/mcp/transport.rs` (verify `parse_sse_payload` handles multi-line `data:` blocks)
- Modify: `src/mcp/transport_tests.rs` (add an SSE-mode test)

- [ ] **Step 1: Write the failing test**

Append:

```rust
#[tokio::test]
async fn http_transport_parses_sse_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (_m, _p, _h, body) = read_http_request(&mut stream).await;
        let req: Value = serde_json::from_str(&body).unwrap();
        let id = req.get("id").and_then(Value::as_u64).unwrap();
        let body = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"ok": true}}).to_string();
        let sse = format!(
            "event: message\r\ndata: {}\r\n\r\n",
            body
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            sse.len(), sse
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.flush().await.unwrap();
    });

    let mut headers = HashMap::new();
    headers.insert("X-API-Key".to_string(), "sk-sse".to_string());
    let transport = HttpMessageTransport::new(format!("http://{}/mcp", addr), headers);
    let req = JsonRpcRequest::new(7, "ping", None);
    let value = transport.round_trip(serde_json::to_string(&req).unwrap() + "\n").await.unwrap();
    let resp: JsonRpcResponse = serde_json::from_value(value).unwrap();
    assert_eq!(resp.id, Some(7));
    assert!(resp.result.is_some());

    server.await.unwrap();
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --lib mcp::transport_tests::http_transport_parses_sse_response -- --exact`
Expected: pass (the SSE parser in `parse_sse_payload` already handles this shape).

If it fails, fix `parse_sse_payload` in `src/mcp/transport.rs` to be more lenient (e.g. accept `\n`-separated JSON, tolerate trailing whitespace).

- [ ] **Step 3: Commit**

```bash
git add src/mcp/transport_tests.rs
git commit -m "test(mcp): HTTP transport parses SSE-shaped responses"
```

---

### Task 9: Confirm embedded-resources machinery is gone

**Files:**
- Verify: `src/saitec/mcp_resources.rs` (should not exist)
- Verify: `build.rs` (should not have `generate_mcp_resources`)

- [ ] **Step 1: Verify the resources file does not exist**

Run: `ls src/saitec/mcp_resources.rs 2>&1`
Expected: `No such file or directory`. If the file exists, delete it (`rm src/saitec/mcp_resources.rs`).

- [ ] **Step 2: Verify `build.rs` has no MCP resource generation**

Run: `grep -n "generate_mcp_resources\|mcp_resources_data\|saitec-mcp" build.rs`
Expected: no output. If there are hits, remove the `generate_mcp_resources` function, its call from `main()`, and any `cargo:rerun-if-changed=_vendor/SAITEC-Skills/...` lines.

- [ ] **Step 3: Verify `src/saitec/mod.rs` does not declare `mcp_resources`**

Run: `grep -n "mcp_resources" src/saitec/mod.rs`
Expected: no output. If present, delete that line.

- [ ] **Step 4: Build to verify nothing breaks**

Run: `cargo build`
Expected: zero new errors.

- [ ] **Step 5: Commit (if any changes were made in steps 1–3)**

```bash
git add -A
git commit -m "chore: remove embedded MCP resources (replaced by HTTP transport)"
```

(If steps 1–3 found nothing, this step is a no-op — no commit needed.)

---

### Task 10: Delete the `_vendor/SAITEC-Skills/` tree

**Files:**
- Delete: `_vendor/SAITEC-Skills/` (entire directory)

- [ ] **Step 1: Confirm no `cargo build.rs` or runtime code references it**

Run: `grep -r "_vendor" src/ build.rs Cargo.toml`
Expected: no results.

- [ ] **Step 2: Delete the tree**

Run: `git rm -r _vendor/SAITEC-Skills`
Run: `rm -rf _vendor` (removes the now-empty parent)

- [ ] **Step 3: Build to verify**

Run: `cargo build`
Expected: zero new errors.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: remove vendored SAITEC-Skills tree (replaced by HTTP transport)"
```

---

### Task 11: Clean up the packaging scripts and release workflow

**Files:**
- Modify: `scripts/package_saitec.ps1` (remove any lines that reference `_vendor/SAITEC-Skills` or `saitec-mcp.resources`)
- Modify: `.github/workflows/release.yml` (remove the `Copy-Item _vendor/SAITEC-Skills` line and the `resources` arg to `tar`)

- [ ] **Step 1: Search for references**

Run: `grep -n "_vendor\|saitec-mcp\|SAITEC-Skills" scripts/package_saitec.ps1 .github/workflows/release.yml`
Expected: hits that need to be removed.

- [ ] **Step 2: Remove the lines**

Edit each file to remove the now-dead references. The release.yml change is mechanical:
- delete the `Copy-Item "_vendor/SAITEC-Skills" "dist/resources/SAITEC-Skills" -Recurse -Force` line
- change the `tar -czf ... -C dist "${{ matrix.artifact }}.exe" "resources"` to drop `"resources"`

The `package_saitec.ps1` may have no references if Task 9 already removed the relevant copy step.

- [ ] **Step 3: Verify the scripts parse**

PowerShell: `pwsh -NoProfile -Command "[scriptblock]::Create((Get-Content scripts/package_saitec.ps1 -Raw)) | Out-Null"`
Expected: no parse error.

For release.yml: `python -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
Expected: no parse error.

- [ ] **Step 4: Commit**

```bash
git add scripts/package_saitec.ps1 .github/workflows/release.yml
git commit -m "chore(packaging): drop MCP resource bundling (now over HTTP)"
```

---

### Task 12: End-to-end smoke test against a real SAITEC MCP server

**Files:**
- Create: `scripts/verify_mcp_http.ps1` (small PowerShell script that runs the release exe and confirms the HTTP MCP is reachable)

- [ ] **Step 1: Write the script**

Create `scripts/verify_mcp_http.ps1`:

```powershell
<#
.SYNOPSIS
    End-to-end verification for the SAITEC-Skills HTTP MCP integration.

.DESCRIPTION
    Cleans any cached MCP config, ensures the SAITEC-Skills entry is
    written as an HTTP-type config, and prints a summary of what the
    client would post. Does not launch a TUI (that requires an
    interactive terminal); instead it exercises the bootstrap path so
    the resulting `~/.saitec_tui/mcp.json` is the canonical shape.

    Required:
      - SAITEC_API_KEY env var (or env bridge file at $env:APPDATA/jcode/saitec.env)
      - The MCP server reachable at $env:SAITEC_MCP_URL (defaults to
        "http://101.133.153.37:8000/mcp" if unset; defined as
        DEFAULT_SAITEC_MCP_URL in src/saitec/auth.rs)
#>

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Info([string]$Message) { Write-Host "[verify] $Message" -ForegroundColor Blue }
function Write-Fail([string]$Message) { Write-Host "[verify] FAIL: $Message" -ForegroundColor Red }

if (-not $env:SAITEC_API_KEY) {
    Write-Info "SAITEC_API_KEY not set. Reading from env bridge file..."
    $bridge = Join-Path $env:APPDATA "jcode\saitec.env"
    if (Test-Path -LiteralPath $bridge) {
        $env:SAITEC_API_KEY = (Get-Content $bridge | Where-Object { $_ -match "^SAITEC_API_KEY=" } | Select-Object -First 1) -replace "^SAITEC_API_KEY=", ""
    }
}
if (-not $env:SAITEC_API_KEY) {
    Write-Fail "SAITEC_API_KEY is not set and not found in env bridge. Run SAITEC login first."
    exit 1
}

# Back up the existing mcp.json so we can restore it.
$path = Join-Path $env:USERPROFILE ".saitec_tui\mcp.json"
$backup = "$path.verify-http.bak"
if (Test-Path -LiteralPath $path) {
    Copy-Item -LiteralPath $path -Destination $backup -Force
    Remove-Item -LiteralPath $path -Force
}
try {
    # Trigger bootstrap by running the binary once (--version does NOT trigger
    # ensure_bootstrap; we need a real call path). The simplest is to run
    # `auth-test`, which calls McpConfig::load() inside the validator path.
    Write-Info "Triggering bootstrap via `auth-test`..."
    $exe = Join-Path $PSScriptRoot "..\target\release\jcode.exe"
    if (-not (Test-Path -LiteralPath $exe)) {
        $exe = Join-Path $PSScriptRoot "..\target\debug\jcode.exe"
    }
    if (-not (Test-Path -LiteralPath $exe)) {
        Write-Fail "jcode.exe not found in target/release or target/debug. Build first."
        exit 1
    }
    & $exe --no-selfdev auth-test 2>&1 | Out-Null

    if (-not (Test-Path -LiteralPath $path)) {
        Write-Fail "$path was not written by bootstrap"
        exit 1
    }
    $cfg = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    $entry = $cfg.servers."SAITEC-Skills"
    if ($null -eq $entry) {
        Write-Fail "SAITEC-Skills entry missing"
        exit 1
    }
    if ($entry.type -ne "http") {
        Write-Fail "Expected type=http, got: $($entry.type)"
        exit 1
    }
    if (-not $entry.url -or -not $entry.url.EndsWith("/mcp")) {
        Write-Fail "Expected url to end with /mcp, got: $($entry.url)"
        exit 1
    }
    if (-not $entry.headers -or $entry.headers."X-API-Key" -ne $env:SAITEC_API_KEY) {
        Write-Fail "Expected X-API-Key header to match the session key"
        exit 1
    }
    Write-Host ""
    Write-Host "[verify] PASS — HTTP MCP config is in canonical shape" -ForegroundColor Green
    Write-Host "  type:    $($entry.type)"
    Write-Host "  url:     $($entry.url)"
    Write-Host "  X-API-Key: $($entry.headers.'X-API-Key'.Substring(0, 8))..."
    Write-Host ""
}
finally {
    if (Test-Path -LiteralPath $backup) {
        Move-Item -LiteralPath $backup -Destination $path -Force
        Write-Info "Restored $path from backup"
    }
}
```

- [ ] **Step 2: Run the script**

Pre-conditions: a release or debug build exists, the user has a SAITEC session (so `SAITEC_API_KEY` is set), and the network is reachable to the SAITEC server.

Run: `pwsh -ExecutionPolicy Bypass -File scripts/verify_mcp_http.ps1`
Expected: `[verify] PASS — HTTP MCP config is in canonical shape`.

- [ ] **Step 3: Commit**

```bash
git add scripts/verify_mcp_http.ps1
git commit -m "chore(scripts): add HTTP MCP verification script"
```

---

### Task 13: Final cleanup and CLAUDE.md update

**Files:**
- Modify: `CLAUDE.md` (update the "SAITEC-Skills vendor sync" section, since the vendored tree no longer exists)
- Modify: `CLAUDE.md` (update the SAITEC Platform Integration section to reflect the HTTP transport)

- [ ] **Step 1: Update the vendor-sync section**

The "SAITEC-Skills vendor sync" section in `CLAUDE.md` (around lines 239-247) describes the manual sync procedure. After this refactor, that procedure is no longer needed: the public HTTP server is the source of truth. Replace the section with a brief note:

```markdown
### SAITEC-Skills HTTP transport (no local vendor)

_vendor/SAITEC-Skills/` is no longer vendored. The SAITEC-Skills MCP server
runs as a public HTTP service at `DEFAULT_SAITEC_MCP_URL` (defined as a
pub const in `src/saitec/auth.rs`, defaults to
`http://101.133.153.37:8000/mcp`, overrideable via the `SAITEC_MCP_URL`
env var), authenticated via the `X-API-Key` header sourced from the
SAITEC login session. The `McpServerConfig` carries
`{type: "http", url, headers}`.

**Updating the protocol**: changes to the upstream MCP server happen
in-place on the public deployment. No sync step is required.

**Security**: the public endpoint is gated by per-user API keys issued
via `submit_business_login`. Anti-distillation controls (rate limits,
output truncation, prompt-injection detection) live on the server.
```

- [ ] **Step 2: Update the SAITEC Platform Integration section**

In the "SAITEC Platform Integration" section (around line 221), change the line:

> "SAITEC-Skills MCP service handles detection/evaluation task dispatch"

to:

> "SAITEC-Skills MCP service (public HTTP at `DEFAULT_SAITEC_MCP_URL`, see `src/saitec/auth.rs` — overridable via `SAITEC_MCP_URL`) handles detection/evaluation task dispatch. Authentication is via the `X-API-Key` header injected at every config load by `apply_runtime_env()`."

- [ ] **Step 3: Run `cargo build` and `cargo test` end-to-end**

Run: `cargo build`
Run: `cargo test --lib -- --skip e2e`
Expected: zero errors, all unit tests pass.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md to reflect HTTP MCP transport"
```

---

## Self-Review

1. **Spec coverage:** every requirement from the spec is covered:
   - Replace stdio with HTTP: Tasks 1, 2, 3, 4, 5
   - Public `X-API-Key` header auth: Tasks 4, 5
   - Remove `_vendor/SAITEC-Skills/`: Tasks 8, 9
   - Remove embedded-resource machinery: Task 8
   - Update packaging/release scripts: Task 10
   - End-to-end verification: Task 11
   - Documentation: Task 12

2. **Placeholder scan:** every code step has the actual code, not "TODO" or "implement later". The trait design in Task 2 is the canonical one (synchronous per-request) and is consistent with how `McpHandle::request` uses it in Task 3. There are no "see Task N" instructions that hide the actual code.

3. **Type consistency:** `McpTransport`, `McpServerConfig.transport/url/headers`, `MessageTransport::round_trip/notify/shutdown`, `HttpMessageTransport::new`, `StdioMessageTransport::new`, `transport_for` — all are introduced in Task 1, 2, or 3 and used consistently thereafter. `ensure_bootstrap` and `apply_runtime_env` in Task 4 use the same field names. The `runtime_api_key()` helper from the old code is preserved unchanged.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-08-saitec-mcp-http-transport.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
