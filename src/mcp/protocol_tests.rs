use super::*;

#[test]
fn test_saitec_bootstrap_creates_missing_mcp_config() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_skills_root = std::env::var_os("SAITEC_SKILLS_ROOT");
    let temp = tempfile::TempDir::new().unwrap();
    let skills_root = temp.path().join("vendored-skills");
    std::fs::create_dir_all(&skills_root).unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_SKILLS_ROOT", &skills_root);

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp.path().join("external").join(".saitec_tui").join("mcp.json");
    assert!(mcp_path.exists(), "expected bootstrap to create mcp.json");

    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
    if let Some(prev_skills_root) = prev_skills_root {
        crate::env::set_var("SAITEC_SKILLS_ROOT", prev_skills_root);
    } else {
        crate::env::remove_var("SAITEC_SKILLS_ROOT");
    }
}

#[test]
fn test_saitec_bootstrap_preserves_existing_servers_and_saitec_entry() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_skills_root = std::env::var_os("SAITEC_SKILLS_ROOT");
    let temp = tempfile::TempDir::new().unwrap();
    let skills_root = temp.path().join("vendored-skills");
    std::fs::create_dir_all(&skills_root).unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_SKILLS_ROOT", &skills_root);

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
    assert_eq!(saitec.command, "custom-bin");
    assert_eq!(saitec.args, vec!["--custom"]);
    assert_eq!(saitec.env.get("CUSTOM"), Some(&"1".to_string()));

    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
    if let Some(prev_skills_root) = prev_skills_root {
        crate::env::set_var("SAITEC_SKILLS_ROOT", prev_skills_root);
    } else {
        crate::env::remove_var("SAITEC_SKILLS_ROOT");
    }
}

#[test]
fn test_saitec_bootstrap_skips_when_vendored_script_missing() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_skills_root = std::env::var_os("SAITEC_SKILLS_ROOT");
    let temp = tempfile::TempDir::new().unwrap();
    let skills_root = temp.path().join("missing-vendored-skills");
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_SKILLS_ROOT", &skills_root);

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp.path().join("external").join(".saitec_tui").join("mcp.json");
    assert!(
        !mcp_path.exists(),
        "expected bootstrap to skip config creation when the vendored server script is missing"
    );

    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
    if let Some(prev_skills_root) = prev_skills_root {
        crate::env::set_var("SAITEC_SKILLS_ROOT", prev_skills_root);
    } else {
        crate::env::remove_var("SAITEC_SKILLS_ROOT");
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
