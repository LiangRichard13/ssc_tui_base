use super::*;

fn restore_env_var(key: &str, value: Option<std::ffi::OsString>) {
    if let Some(value) = value {
        crate::env::set_var(key, value);
    } else {
        crate::env::remove_var(key);
    }
}

fn sample_saitec_session(api_key: &str) -> crate::saitec::auth::SaitecSession {
    crate::saitec::auth::SaitecSession {
        auth_token: Some("jwt-test-token".to_string()),
        api_key: api_key.to_string(),
        token_type: "Bearer".to_string(),
        user_id: Some("user-123".to_string()),
        email: Some("user@example.com".to_string()),
        phone: None,
        display_name: Some("Test User".to_string()),
        api_key_id: Some("key-123".to_string()),
        api_key_name: Some("SAITEC-TUI-test".to_string()),
        api_key_created_at: None,
        api_key_expires_at: None,
        last_validated_at: None,
    }
}

#[test]
fn test_saitec_mcp_load_injects_saved_session_runtime_header_without_persisting_secret() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");

    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::remove_var("SAITEC_API_KEY");

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

    let mcp_path = temp.path().join("mcp.json");
    let persisted = std::fs::read_to_string(mcp_path).unwrap();
    assert!(
        !persisted.contains("sk-session-only"),
        "runtime API key must not be persisted in mcp.json"
    );

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
}

#[test]
fn test_saitec_bootstrap_creates_http_entry_when_no_mcp_config() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");

    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_API_KEY", "sk-bootstrap");

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp.path().join("mcp.json");
    assert!(mcp_path.exists(), "expected bootstrap to create mcp.json");
    let config = McpConfig::load_from_file(&mcp_path).unwrap();
    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(saitec.transport, McpTransport::Http);
    assert!(
        saitec.headers.get("X-API-Key").is_none(),
        "API key must not be persisted in mcp.json"
    );
    assert!(saitec.url.as_deref().unwrap().ends_with("/mcp"));

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
}

#[test]
fn test_saitec_bootstrap_writes_http_entry_unaffected_by_disk_layout() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");

    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_API_KEY", "sk-direct");

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp.path().join("mcp.json");
    let config = McpConfig::load_from_file(&mcp_path).unwrap();
    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(saitec.transport, McpTransport::Http);
    assert!(
        saitec.headers.get("X-API-Key").is_none(),
        "API key must not be persisted in mcp.json"
    );
    assert!(saitec.url.as_deref().unwrap().ends_with("/mcp"));

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
}

#[test]
fn test_saitec_bootstrap_migrates_existing_stdio_entry_to_http() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");

    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_API_KEY", "sk-refresh");

    let mcp_dir = temp.path();
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
    )
    .unwrap();

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
    assert!(
        saitec.headers.get("X-API-Key").is_none(),
        "API key must not be persisted by ensure_bootstrap"
    );
    assert!(saitec.url.as_deref().unwrap().ends_with("/mcp"));

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
}

#[test]
fn test_saitec_bootstrap_writes_http_entry_even_without_api_key() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");

    let temp = tempfile::TempDir::new().unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::remove_var("SAITEC_API_KEY");

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp.path().join("mcp.json");
    let config = McpConfig::load_from_file(&mcp_path).unwrap();
    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(saitec.transport, McpTransport::Http);
    assert!(saitec.headers.get("X-API-Key").is_none());
    assert!(saitec.url.as_deref().unwrap().ends_with("/mcp"));

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
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
