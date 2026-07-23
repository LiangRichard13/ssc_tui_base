//! Pluggable transport layer for MCP message exchange.
//!
//! `MessageTransport` abstracts the JSON-RPC request/response surface
//! that `McpHandle` sits on top of. Two impls:
//! - [`StdioMessageTransport`]: spawns a child process, exchanges
//!   newline-delimited JSON on its stdin/stdout.
//! - [`HttpMessageTransport`]: posts JSON-RPC to a remote endpoint,
//!   parses JSON or SSE responses per the streamable-HTTP spec.

use crate::mcp::protocol::{McpServerConfig, McpTransport};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

/// JSON-RPC message exchange. Synchronous per request: the future returned
/// by `round_trip` resolves to the matching response payload, or fails.
///
/// `notify` sends a JSON-RPC notification (no `id`, no response expected).
///
/// `shutdown` terminates the transport. Idempotent.
#[async_trait]
pub trait MessageTransport: Send + Sync + std::fmt::Debug {
    async fn round_trip(&self, request: String) -> Result<Value>;
    async fn notify(&self, notification: String) -> Result<()>;
    async fn shutdown(&self);
}

#[derive(Debug)]
struct StdioState {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
}

/// Stdio transport: spawns a child process on first use, exchanges
/// newline-delimited JSON-RPC on its stdin/stdout.
#[derive(Debug)]
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

        *state = Some(StdioState {
            child,
            stdin,
            stdout,
        });
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

/// HTTP transport: posts JSON-RPC to `{url}` with the configured headers,
/// parses JSON or SSE responses per the streamable-HTTP spec.
#[derive(Debug)]
pub struct HttpMessageTransport {
    url: String,
    headers: HashMap<String, String>,
    client: reqwest::Client,
    session_id: tokio::sync::Mutex<Option<String>>,
    closed: AtomicBool,
}

impl HttpMessageTransport {
    pub fn new(url: String, headers: HashMap<String, String>) -> Self {
        Self {
            url,
            headers,
            client: reqwest::Client::new(),
            session_id: tokio::sync::Mutex::new(None),
            closed: AtomicBool::new(false),
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
        if self.closed.load(Ordering::SeqCst) {
            anyhow::bail!("MCP HTTP transport is closed");
        }
        let mut req = self
            .client
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
        if self.closed.load(Ordering::SeqCst) {
            anyhow::bail!("MCP HTTP transport is closed");
        }
        let mut req = self
            .client
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
        self.closed.store(true, Ordering::SeqCst);
        self.session_id.lock().await.take();
    }
}

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
            Ok(Box::new(HttpMessageTransport::new(
                url,
                config.headers.clone(),
            )))
        }
    }
}
