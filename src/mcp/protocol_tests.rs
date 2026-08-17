use super::*;

#[test]
fn http_user_supplied_config_in_mcp_json_is_loaded_verbatim() {
    // Stage 1 (chore/ssc-tui-baseline): after McpConfig::load no longer calls into
    // src/saitec/, a user-configured HTTP MCP server in $JCODE_HOME/mcp.json must be
    // parsed verbatim — url and headers preserved as written, no SAITEC-specific
    // rewriting, no in-memory key injection.
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());

    let mcp_path = temp.path().join("mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{
            "servers": {
                "user-http-mcp": {
                    "type": "http",
                    "url": "http://example.com:9000/mcp",
                    "headers": {"X-API-Key": "sk-user-configured"},
                    "shared": true
                }
            }
        }"#,
    )
    .unwrap();

    let config = McpConfig::load();
    let server = config
        .servers
        .get("user-http-mcp")
        .expect("user-supplied HTTP MCP entry must be loaded");
    assert_eq!(server.transport, McpTransport::Http);
    assert_eq!(
        server.url.as_deref(),
        Some("http://example.com:9000/mcp")
    );
    assert_eq!(
        server.headers.get("X-API-Key").map(String::as_str),
        Some("sk-user-configured"),
        "user-configured X-API-Key must be preserved verbatim"
    );
    assert!(server.shared);

    // Re-read the file directly to confirm what was persisted (no SAITEC rewrite).
    let persisted = McpConfig::load_from_file(&mcp_path).unwrap();
    let persisted_server = persisted.servers.get("user-http-mcp").unwrap();
    assert_eq!(
        persisted_server.headers.get("X-API-Key").map(String::as_str),
        Some("sk-user-configured"),
        "user-configured X-API-Key must remain in mcp.json"
    );

    if let Some(value) = prev_home {
        crate::env::set_var("JCODE_HOME", value);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}

#[test]
fn test_json_rpc_request_serialization() {
    let request = JsonRpcRequest::new(1, "tools/list", None);
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
    assert!(json.contains("\"id\":1"));
    assert!(json.contains("\"method\":\"tools/list\""));
}

#[test]
fn test_json_rpc_response_deserialization() {
    let json = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
    let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, Some(1));
    assert!(response.result.is_some());
    assert!(response.error.is_none());
}

#[test]
fn test_json_rpc_error_response() {
    let json = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
    let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
    assert!(response.error.is_some());
    let err = response.error.unwrap();
    assert_eq!(err.code, -32600);
    assert_eq!(err.message, "Invalid Request");
}

#[test]
fn test_mcp_config_deserialization() {
    let json = r#"{
            "servers": {
                "test-server": {
                    "command": "/usr/bin/test-mcp",
                    "args": ["--port", "8080"],
                    "env": {"API_KEY": "secret"}
                }
            }
        }"#;
    let config: McpConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.servers.len(), 1);
    let server = config.servers.get("test-server").unwrap();
    assert_eq!(server.command, "/usr/bin/test-mcp");
    assert_eq!(server.args, vec!["--port", "8080"]);
    assert_eq!(server.env.get("API_KEY"), Some(&"secret".to_string()));
}

#[test]
fn test_mcp_config_empty() {
    let json = r#"{}"#;
    let config: McpConfig = serde_json::from_str(json).unwrap();
    assert!(config.servers.is_empty());
}

#[test]
fn test_tool_def_deserialization() {
    let json = r#"{
            "name": "read_file",
            "description": "Read a file from disk",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }
        }"#;
    let tool: McpToolDef = serde_json::from_str(json).unwrap();
    assert_eq!(tool.name, "read_file");
    assert_eq!(tool.description, Some("Read a file from disk".to_string()));
}

#[test]
fn test_tool_call_result_text() {
    let json = r#"{
            "content": [{"type": "text", "text": "File contents here"}],
            "isError": false
        }"#;
    let result: ToolCallResult = serde_json::from_str(json).unwrap();
    assert!(!result.is_error);
    assert_eq!(result.content.len(), 1);
    match &result.content[0] {
        ContentBlock::Text { text, .. } => assert_eq!(text, "File contents here"),
        _ => panic!("Expected text block"),
    }
}

#[test]
fn test_tool_call_result_error() {
    let json = r#"{
            "content": [{"type": "text", "text": "File not found"}],
            "isError": true
        }"#;
    let result: ToolCallResult = serde_json::from_str(json).unwrap();
    assert!(result.is_error);
}

#[test]
fn test_initialize_result() {
    let json = r#"{
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {"listChanged": true}
            },
            "serverInfo": {
                "name": "test-server",
                "version": "1.0.0"
            }
        }"#;
    let result: InitializeResult = serde_json::from_str(json).unwrap();
    assert_eq!(result.protocol_version, "2024-11-05");
    assert!(result.server_info.is_some());
}

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
