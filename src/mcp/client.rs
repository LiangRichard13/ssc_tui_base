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
        let params = ToolCallParams {
            name: name.to_string(),
            arguments,
        };
        let response = self
            .request("tools/call", Some(serde_json::to_value(params)?))
            .await?;
        let result = response.result.context("No result from tool call")?;
        let tool_result: ToolCallResult = serde_json::from_value(result)?;
        Ok(tool_result)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn server_info(&self) -> Option<ServerInfo> {
        self.server_info
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn tools(&self) -> Vec<McpToolDef> {
        self.tools
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Refresh the list of available tools
    pub async fn refresh_tools(&self) -> Result<()> {
        let response = self.request("tools/list", None).await?;
        if let Some(result) = response.result {
            let tools_result: ToolsListResult = serde_json::from_value(result)?;
            *self
                .tools
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = tools_result.tools;
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
    /// Connect to an MCP server
    pub async fn connect(name: String, config: &McpServerConfig) -> Result<Self> {
        crate::logging::info(&format!("MCP: Connecting to '{}'", name));

        // Use `transport_for` to dispatch to stdio or HTTP transport
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

        client
            .initialize()
            .await
            .with_context(|| format!("MCP server '{}' failed to initialize", name))?;

        client
            .handle
            .refresh_tools()
            .await
            .with_context(|| format!("MCP server '{}' failed to list tools", name))?;

        crate::logging::info(&format!(
            "MCP: Connected to '{}' with {} tools",
            name,
            client.handle.tools().len()
        ));

        Ok(client)
    }

    /// Get a shareable handle to this client
    pub fn handle(&self) -> McpHandle {
        self.handle.clone()
    }

    /// Initialize the MCP connection
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
            *self
                .handle
                .server_info
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = init_result.server_info;
            *self
                .handle
                .capabilities
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = init_result.capabilities;
        }

        // Send initialized notification.
        // JSON-RPC 2.0: a notification has NO "id" field. Including id: 0 (which
        // `JsonRpcRequest::new(0, ...)` would produce) is rejected by strict
        // servers (e.g. Pydantic discriminated-union validation in
        // SAITEC-Skills) as "Input should be 'ping'"/"Input should be
        // 'initialize'". Build the wire payload directly so the id field is
        // absent.
        let notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": serde_json::Value::Null,
        });
        let msg = serde_json::to_string(&notif)? + "\n";
        self.transport.notify(msg).await?;

        Ok(())
    }

    /// Check if server is still running
    pub fn is_running(&self) -> bool {
        true
    }

    /// Shutdown the server
    pub async fn shutdown(&mut self) {
        self.transport.shutdown().await;
    }

    // === Legacy compatibility methods that delegate to handle ===

    pub fn name(&self) -> &str {
        &self.handle.name
    }

    pub fn server_info(&self) -> Option<ServerInfo> {
        self.handle.server_info()
    }

    pub fn tools(&self) -> Vec<McpToolDef> {
        self.handle.tools()
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
        self.handle.call_tool(name, arguments).await
    }

    pub async fn refresh_tools(&self) -> Result<()> {
        self.handle.refresh_tools().await
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // The transport's drop or shutdown handles cleanup.
        // For stdio: kills the child process.
        // For HTTP: no-op.
    }
}
