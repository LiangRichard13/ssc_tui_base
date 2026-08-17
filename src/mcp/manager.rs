//! MCP Manager - manages MCP server connections for a single session.
//!
//! In daemon mode with a shared pool, servers marked `shared: true` (the default)
//! are managed by the pool and reused across sessions. Servers marked `shared: false`
//! (e.g., Playwright with browser state) are spawned per-session.

use super::client::{McpClient, McpHandle};
use super::pool::SharedMcpPool;
use super::protocol::{McpConfig, McpServerConfig, McpToolDef, ToolCallResult};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default, Serialize)]
pub struct McpManagerMemoryProfile {
    pub shared_pool_enabled: bool,
    pub configured_servers: usize,
    pub connected_servers: usize,
    pub pooled_handles: usize,
    pub owned_clients: usize,
    pub available_tools: usize,
    pub configured_json_bytes: usize,
    pub tool_schema_estimate_bytes: usize,
}

/// Manages MCP server connections for a session.
///
/// In daemon mode, shared servers delegate to the SharedMcpPool while
/// non-shared (stateful) servers are owned per-session.
pub struct McpManager {
    pool: Option<Arc<SharedMcpPool>>,
    /// Handles from the shared pool (shared servers)
    pool_handles: RwLock<HashMap<String, McpHandle>>,
    /// Per-session owned clients (non-shared / stateful servers)
    owned_clients: RwLock<HashMap<String, McpClient>>,
    config: McpConfig,
    session_id: String,
}

impl McpManager {
    /// Create a new manager in owned in-process mode (used by tests and local harnesses).
    pub fn new() -> Self {
        Self {
            pool: None,
            pool_handles: RwLock::new(HashMap::new()),
            owned_clients: RwLock::new(HashMap::new()),
            config: McpConfig::load(),
            session_id: "owned".to_string(),
        }
    }

    /// Create a manager backed by a shared pool (daemon mode)
    pub fn with_shared_pool(pool: Arc<SharedMcpPool>, session_id: String) -> Self {
        Self {
            pool: Some(pool),
            pool_handles: RwLock::new(HashMap::new()),
            owned_clients: RwLock::new(HashMap::new()),
            config: McpConfig::load(),
            session_id,
        }
    }

    /// Create manager with specific config (no sharing)
    pub fn with_config(config: McpConfig) -> Self {
        Self {
            pool: None,
            pool_handles: RwLock::new(HashMap::new()),
            owned_clients: RwLock::new(HashMap::new()),
            config,
            session_id: "owned".to_string(),
        }
    }

    /// Whether this manager has a shared pool available
    pub fn is_shared(&self) -> bool {
        self.pool.is_some()
    }

    /// Connect to all configured servers.
    /// Shared servers go to the pool, non-shared are spawned per-session.
    #[expect(
        clippy::collapsible_if,
        reason = "MCP connect flow keeps shared-pool and owned-server paths explicit"
    )]
    pub async fn connect_all(&self) -> Result<(usize, Vec<(String, String)>)> {
        let mut total_successes = 0;
        let mut total_failures = Vec::new();

        // Split servers into shared vs owned
        let (shared_servers, owned_servers): (Vec<_>, Vec<_>) = self
            .config
            .servers
            .iter()
            .partition(|(_, config)| config.shared && self.pool.is_some());

        // Connect shared servers via pool
        if let Some(pool) = &self.pool {
            if !shared_servers.is_empty() {
                let (successes, failures) = pool.connect_all().await;
                total_successes += successes;
                total_failures.extend(failures);

                // Acquire handles for shared servers only
                let all_handles = pool.acquire_handles(&self.session_id).await;
                let shared_names: std::collections::HashSet<&String> =
                    shared_servers.iter().map(|(name, _)| *name).collect();
                let mut pool_handles = self.pool_handles.write().await;
                for (name, handle) in all_handles {
                    if shared_names.contains(&name) {
                        pool_handles.insert(name, handle);
                    }
                }

                // If pool already had servers connected, count those as successes
                if total_successes == 0 && !pool_handles.is_empty() {
                    total_successes = pool_handles.len();
                }
            }
        }

        // Connect non-shared servers per-session
        if !owned_servers.is_empty() {
            let mut spawn_handles = Vec::new();

            for (name, config) in owned_servers {
                let name = name.clone();
                let config = config.clone();
                let handle = tokio::spawn(async move {
                    let result = McpClient::connect(name.clone(), &config).await;
                    (name, result)
                });
                spawn_handles.push(handle);
            }

            for handle in spawn_handles {
                match handle.await {
                    Ok((name, Ok(client))) => {
                        let mut clients = self.owned_clients.write().await;
                        clients.insert(name, client);
                        total_successes += 1;
                    }
                    Ok((name, Err(e))) => {
                        let error_msg = format!("{:#}", e);
                        crate::logging::error(&format!(
                            "Failed to connect to MCP server '{}': {}",
                            name, error_msg
                        ));
                        total_failures.push((name, error_msg));
                    }
                    Err(e) => {
                        crate::logging::error(&format!("MCP connection task panicked: {}", e));
                    }
                }
            }
        }

        Ok((total_successes, total_failures))
    }

    /// Connect to a specific server
    #[expect(
        clippy::collapsible_if,
        reason = "MCP connect flow keeps shared-pool and owned-server paths explicit"
    )]
    pub async fn connect(&self, name: &str, config: &McpServerConfig) -> Result<()> {
        if config.shared {
            if let Some(pool) = &self.pool {
                pool.connect_server(name, config).await?;
                if let Some(handle) = pool.get_handle(name).await {
                    self.pool_handles
                        .write()
                        .await
                        .insert(name.to_string(), handle);
                }
                return Ok(());
            }
        }

        // Owned (non-shared or no pool available)
        let client = McpClient::connect(name.to_string(), config)
            .await
            .with_context(|| format!("Failed to connect to MCP server '{}'", name))?;

        self.owned_clients
            .write()
            .await
            .insert(name.to_string(), client);
        Ok(())
    }

    /// Disconnect from a server
    pub async fn disconnect(&self, name: &str) -> Result<()> {
        // Check if it's a pool handle
        {
            let mut handles = self.pool_handles.write().await;
            if handles.remove(name).is_some() {
                if let Some(pool) = &self.pool {
                    // Truly tear down the pooled client (remove handle, shutdown
                    // transport) so a later connect re-fetches tools/list instead
                    // of reusing a stale handle with cached tool definitions.
                    pool.disconnect_server(name).await;
                }
                return Ok(());
            }
        }

        // Otherwise it's owned
        let mut clients = self.owned_clients.write().await;
        if let Some(mut client) = clients.remove(name) {
            client.shutdown().await;
        }
        Ok(())
    }

    /// Disconnect from all servers
    pub async fn disconnect_all(&self) {
        // Release pool handles
        {
            let mut handles = self.pool_handles.write().await;
            let names: Vec<String> = handles.keys().cloned().collect();
            handles.clear();
            if let Some(pool) = &self.pool {
                pool.release_handles(&self.session_id, &names).await;
            }
        }

        // Shutdown owned clients
        {
            let mut clients = self.owned_clients.write().await;
            for (_, mut client) in clients.drain() {
                client.shutdown().await;
            }
        }
    }

    /// Get list of connected server names
    pub async fn connected_servers(&self) -> Vec<String> {
        let mut names: Vec<String> = self.pool_handles.read().await.keys().cloned().collect();
        names.extend(self.owned_clients.read().await.keys().cloned());
        names
    }

    /// Get all available tools from all connected servers
    pub async fn all_tools(&self) -> Vec<(String, McpToolDef)> {
        let mut tools = Vec::new();

        // Pool handles
        for (server_name, handle) in self.pool_handles.read().await.iter() {
            for tool in handle.tools() {
                tools.push((server_name.clone(), tool));
            }
        }

        // Owned clients
        for (server_name, client) in self.owned_clients.read().await.iter() {
            for tool in client.tools() {
                tools.push((server_name.clone(), tool));
            }
        }

        tools
    }

    /// Call a tool on a specific server
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult> {
        // Try pool handles first
        {
            let handles = self.pool_handles.read().await;
            if let Some(handle) = handles.get(server) {
                return handle.call_tool(tool, arguments).await;
            }
        }

        // Try owned clients
        {
            let clients = self.owned_clients.read().await;
            if let Some(client) = clients.get(server) {
                return client.call_tool(tool, arguments).await;
            }
        }

        anyhow::bail!("MCP server '{}' not connected", server)
    }

    /// Reload config and reconnect to servers
    pub async fn reload(&mut self) -> Result<(usize, Vec<(String, String)>)> {
        // Disconnect all (releases pool handles, shuts down owned)
        self.disconnect_all().await;

        // Reload config
        self.config = McpConfig::load();

        // If we have a pool, reload it too (reconnects shared servers)
        if let Some(pool) = &self.pool {
            pool.reload().await;
        }

        // Reconnect everything
        self.connect_all().await
    }

    /// Get config
    pub fn config(&self) -> &McpConfig {
        &self.config
    }

    /// Re-acquire a handle from the shared pool without re-connecting.
    /// Used after a pool-level reconnect (e.g., after auth reconnect).
    pub async fn reacquire_pool_handle(&self, name: &str) -> bool {
        if let Some(pool) = &self.pool {
            if let Some(handle) = pool.get_handle(name).await {
                self.pool_handles
                    .write()
                    .await
                    .insert(name.to_string(), handle);
                return true;
            }
        }
        false
    }

    pub fn debug_memory_profile(&self) -> McpManagerMemoryProfile {
        let pooled_handles = self
            .pool_handles
            .try_read()
            .map(|handles| handles.len())
            .unwrap_or(0);
        let owned_clients = self
            .owned_clients
            .try_read()
            .map(|clients| clients.len())
            .unwrap_or(0);

        let mut available_tools = 0usize;
        let mut tool_schema_estimate_bytes = 0usize;

        if let Ok(handles) = self.pool_handles.try_read() {
            for handle in handles.values() {
                for tool in handle.tools() {
                    available_tools += 1;
                    tool_schema_estimate_bytes += estimate_tool_bytes(&tool);
                }
            }
        }

        if let Ok(clients) = self.owned_clients.try_read() {
            for client in clients.values() {
                for tool in client.tools() {
                    available_tools += 1;
                    tool_schema_estimate_bytes += estimate_tool_bytes(&tool);
                }
            }
        }

        McpManagerMemoryProfile {
            shared_pool_enabled: self.pool.is_some(),
            configured_servers: self.config.servers.len(),
            connected_servers: pooled_handles + owned_clients,
            pooled_handles,
            owned_clients,
            available_tools,
            configured_json_bytes: crate::process_memory::estimate_json_bytes(&self.config),
            tool_schema_estimate_bytes,
        }
    }

    /// Check if any servers are connected
    pub async fn has_connections(&self) -> bool {
        !self.pool_handles.read().await.is_empty() || !self.owned_clients.read().await.is_empty()
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

fn estimate_tool_bytes(tool: &McpToolDef) -> usize {
    tool.name.len()
        + tool
            .description
            .as_ref()
            .map(|value| value.len())
            .unwrap_or(0)
        + crate::process_memory::estimate_json_bytes(&tool.input_schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::{McpServerConfig, McpTransport};
    use serde_json::Value;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn read_http_request(
        stream: &mut tokio::net::TcpStream,
    ) -> (String, String, HashMap<String, String>, String) {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        loop {
            let n = stream.read(&mut tmp).await.unwrap();
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if n == 0 {
                break;
            }
        }
        let raw = String::from_utf8_lossy(&buf).to_string();
        let mut lines = raw.split("\r\n");
        let request_line = lines.next().unwrap_or("").to_string();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        let mut headers = HashMap::new();
        let mut content_length: usize = 0;
        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim().to_string();
                if k == "content-length" {
                    content_length = v.parse().unwrap_or(0);
                }
                headers.insert(k, v);
            }
        }
        let mut body = String::new();
        if let Some(idx) = raw.find("\r\n\r\n") {
            body = raw[idx + 4..].to_string();
        }
        while body.len() < content_length {
            let n = stream.read(&mut tmp).await.unwrap();
            body.push_str(&String::from_utf8_lossy(&tmp[..n]));
        }
        (method, path, headers, body)
    }

    fn ok_json_response(id: u64, body: &str) -> String {
        let resp = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{body}}}");
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            resp.len(),
            resp
        )
    }

    /// A shared-server disconnect must actually remove the handle from the
    /// pool, so a subsequent connect rebuilds the client (and re-fetches
    /// tools/list) instead of reusing a stale handle.
    #[tokio::test]
    async fn shared_disconnect_removes_pool_handle() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/mcp");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Requests: initialize, notifications/initialized, tools/list.
            for _ in 0..3 {
                let (_m, _p, _h, body) = read_http_request(&mut stream).await;
                let req: Value = serde_json::from_str(&body).unwrap();
                let method = req.get("method").and_then(Value::as_str).unwrap_or("");
                let id = req.get("id").and_then(Value::as_u64);
                let resp = match method {
                    "initialize" => ok_json_response(
                        id.unwrap(),
                        r#"{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"fake","version":"0.1.0"}}"#,
                    ),
                    "tools/list" => ok_json_response(
                        id.unwrap(),
                        r#"{"tools":[{"name":"ping","description":"test tool","inputSchema":{"type":"object"}}]}"#,
                    ),
                    _ => ok_json_response(0, "{}"),
                };
                stream.write_all(resp.as_bytes()).await.unwrap();
                stream.flush().await.unwrap();
            }
        });

        let pool = Arc::new(SharedMcpPool::new(McpConfig::default()));
        let mgr = McpManager::with_shared_pool(Arc::clone(&pool), "test-session".to_string());

        let config = McpServerConfig {
            transport: McpTransport::Http,
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            url: Some(url.clone()),
            headers: HashMap::new(),
            shared: true,
        };

        mgr.connect("fake", &config).await.expect("connect");
        assert!(pool.get_handle("fake").await.is_some());

        mgr.disconnect("fake").await.expect("disconnect");
        assert!(
            pool.get_handle("fake").await.is_none(),
            "shared disconnect must remove the handle from the pool so a later connect rebuilds it"
        );

        server.await.unwrap();
    }

    /// After a shared disconnect, a subsequent connect must re-fetch tools/list
    /// from the server — not reuse a stale handle with cached tool definitions.
    /// The mock server returns only `ping` on the first connection, then adds
    /// `pong` on the second (simulating the server-side MCP update).
    #[tokio::test]
    async fn shared_reconnect_refetches_updated_tools() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/mcp");

        let server = tokio::spawn(async move {
            // Two connection rounds: round 0 serves [ping], round 1 (after the
            // server-side update) serves [ping, pong].
            for round in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                // Requests: initialize, notifications/initialized, tools/list.
                for _ in 0..3 {
                    let (_m, _p, _h, body) = read_http_request(&mut stream).await;
                    let req: Value = serde_json::from_str(&body).unwrap();
                    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
                    let id = req.get("id").and_then(Value::as_u64);
                    let tools_body = if round == 0 {
                        r#"{"tools":[{"name":"ping","description":"test tool","inputSchema":{"type":"object"}}]}"#
                    } else {
                        r#"{"tools":[{"name":"ping","description":"test tool","inputSchema":{"type":"object"}},{"name":"pong","description":"new tool","inputSchema":{"type":"object"}}]}"#
                    };
                    let resp = match method {
                        "initialize" => ok_json_response(
                            id.unwrap(),
                            r#"{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"fake","version":"0.1.0"}}"#,
                        ),
                        "tools/list" => ok_json_response(id.unwrap(), tools_body),
                        _ => ok_json_response(0, "{}"),
                    };
                    stream.write_all(resp.as_bytes()).await.unwrap();
                    stream.flush().await.unwrap();
                }
            }
        });

        let pool = Arc::new(SharedMcpPool::new(McpConfig::default()));
        let mgr = McpManager::with_shared_pool(Arc::clone(&pool), "test-session".to_string());

        let config = McpServerConfig {
            transport: McpTransport::Http,
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            url: Some(url.clone()),
            headers: HashMap::new(),
            shared: true,
        };

        // First connection: only `ping` exists.
        mgr.connect("fake", &config).await.expect("first connect");
        let handle = pool.get_handle("fake").await.unwrap();
        let first: Vec<String> = handle.tools().iter().map(|t| t.name.clone()).collect();
        assert_eq!(first, vec!["ping"]);

        // Server-side update + reconnect: `pong` must appear.
        mgr.disconnect("fake").await.expect("disconnect");
        mgr.connect("fake", &config).await.expect("second connect");

        let handle = pool.get_handle("fake").await.unwrap();
        let second: Vec<String> = handle.tools().iter().map(|t| t.name.clone()).collect();
        assert!(
            second.contains(&"pong".to_string()),
            "reconnect must re-fetch tools/list after server update, got: {:?}",
            second
        );

        server.await.unwrap();
    }
}
