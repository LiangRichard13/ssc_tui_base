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
fn test_saitec_mcp_load_injects_saved_session_runtime_env_without_persisting_secret() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_skills_root = std::env::var_os("SAITEC_SKILLS_ROOT");
    let prev_api_key = std::env::var_os("SAITEC_API_KEY");
    let prev_core_api_base = std::env::var_os("CORE_API_BASE");
    let prev_saitec_api_base = std::env::var_os("SAITEC_API_BASE");
    let prev_auth_base = std::env::var_os("SAITEC_AUTH_BASE");

    let temp = tempfile::TempDir::new().unwrap();
    let skills_root = temp.path().join("vendored-skills");
    let server_dir = skills_root.join("mcp_server");
    std::fs::create_dir_all(&server_dir).unwrap();
    std::fs::write(server_dir.join("server.py"), "print('saitec')\n").unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_SKILLS_ROOT", &skills_root);
    crate::env::remove_var("SAITEC_API_KEY");
    crate::env::remove_var("CORE_API_BASE");
    crate::env::remove_var("SAITEC_API_BASE");
    crate::env::remove_var("SAITEC_AUTH_BASE");

    let auth_path = crate::saitec::paths::auth_file().unwrap();
    crate::storage::write_json_secret(&auth_path, &sample_saitec_session("sk-session-only"))
        .unwrap();

    let config = McpConfig::load();
    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(
        saitec.env.get("SAITEC_API_KEY").map(String::as_str),
        Some("sk-session-only")
    );
    assert_eq!(
        saitec.env.get("CORE_API_BASE").map(String::as_str),
        Some(crate::saitec::auth::DEFAULT_CORE_API_BASE)
    );

    let mcp_path = temp
        .path()
        .join("external")
        .join(".saitec_tui")
        .join("mcp.json");
    let persisted = std::fs::read_to_string(mcp_path).unwrap();
    assert!(
        !persisted.contains("sk-session-only"),
        "runtime API key must not be persisted in mcp.json"
    );

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_SKILLS_ROOT", prev_skills_root);
    restore_env_var("SAITEC_API_KEY", prev_api_key);
    restore_env_var("CORE_API_BASE", prev_core_api_base);
    restore_env_var("SAITEC_API_BASE", prev_saitec_api_base);
    restore_env_var("SAITEC_AUTH_BASE", prev_auth_base);
}

#[test]
fn test_saitec_bootstrap_creates_missing_mcp_config() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_skills_root = std::env::var_os("SAITEC_SKILLS_ROOT");
    let temp = tempfile::TempDir::new().unwrap();
    let skills_root = temp.path().join("vendored-skills");
    let server_dir = skills_root.join("mcp_server");
    std::fs::create_dir_all(&server_dir).unwrap();
    std::fs::write(server_dir.join("server.py"), "print('saitec')\n").unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_SKILLS_ROOT", &skills_root);

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp
        .path()
        .join("external")
        .join(".saitec_tui")
        .join("mcp.json");
    assert!(mcp_path.exists(), "expected bootstrap to create mcp.json");
    let config = McpConfig::load_from_file(&mcp_path).unwrap();
    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(saitec.command, "python");
    assert_eq!(
        saitec.args,
        vec![server_dir.join("server.py").display().to_string()]
    );
    assert_eq!(
        saitec.env.get("PYTHONIOENCODING"),
        Some(&"utf-8".to_string())
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
fn test_saitec_bootstrap_prefers_private_installed_mcp_resources() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_skills_root = std::env::var_os("SAITEC_SKILLS_ROOT");
    let prev_local_appdata = std::env::var_os("LOCALAPPDATA");
    let temp = tempfile::TempDir::new().unwrap();
    let local_appdata = temp.path().join("local-appdata");
    let private_skills_root = local_appdata
        .join("saitec-tui")
        .join("resources")
        .join(".saitec-mcp")
        .join("SAITEC-Skills");
    let server_dir = private_skills_root.join("mcp_server");
    std::fs::create_dir_all(&server_dir).unwrap();
    std::fs::write(server_dir.join("server.py"), "print('private saitec')\n").unwrap();

    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::remove_var("SAITEC_SKILLS_ROOT");
    crate::env::set_var("LOCALAPPDATA", &local_appdata);

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp
        .path()
        .join("external")
        .join(".saitec_tui")
        .join("mcp.json");
    let config = McpConfig::load_from_file(&mcp_path).unwrap();
    let saitec = config.servers.get("SAITEC-Skills").unwrap();
    assert_eq!(
        saitec.args,
        vec![server_dir.join("server.py").display().to_string()]
    );

    restore_env_var("JCODE_HOME", prev_home);
    restore_env_var("SAITEC_SKILLS_ROOT", prev_skills_root);
    restore_env_var("LOCALAPPDATA", prev_local_appdata);
}

#[test]
fn test_saitec_bootstrap_preserves_existing_servers_and_refreshes_saitec_entry() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_skills_root = std::env::var_os("SAITEC_SKILLS_ROOT");
    let prev_saitec_python = std::env::var_os("SAITEC_TUI_PYTHON");
    let temp = tempfile::TempDir::new().unwrap();
    let skills_root = temp.path().join("vendored-skills");
    let server_dir = skills_root.join("mcp_server");
    std::fs::create_dir_all(&server_dir).unwrap();
    std::fs::write(server_dir.join("server.py"), "print('saitec')\n").unwrap();
    let managed_python = temp.path().join("venv").join("Scripts").join("python.exe");
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_SKILLS_ROOT", &skills_root);
    crate::env::set_var("SAITEC_TUI_PYTHON", &managed_python);

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
    assert_eq!(saitec.command, managed_python.display().to_string());
    assert_eq!(
        saitec.args,
        vec![server_dir.join("server.py").display().to_string()]
    );
    assert_eq!(saitec.env.get("CUSTOM"), Some(&"1".to_string()));
    assert_eq!(
        saitec.env.get("PYTHONPATH"),
        Some(&server_dir.display().to_string())
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
    if let Some(prev_saitec_python) = prev_saitec_python {
        crate::env::set_var("SAITEC_TUI_PYTHON", prev_saitec_python);
    } else {
        crate::env::remove_var("SAITEC_TUI_PYTHON");
    }
}

#[test]
fn test_saitec_bootstrap_skips_when_vendored_script_missing() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let prev_skills_root = std::env::var_os("SAITEC_SKILLS_ROOT");
    let temp = tempfile::TempDir::new().unwrap();
    let skills_root = temp.path().join("missing-vendored-skills");
    std::fs::create_dir_all(&skills_root).unwrap();
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::env::set_var("SAITEC_SKILLS_ROOT", &skills_root);

    crate::saitec::mcp::ensure_bootstrap().unwrap();

    let mcp_path = temp
        .path()
        .join("external")
        .join(".saitec_tui")
        .join("mcp.json");
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
