use crate::mcp::{McpConfig, McpServerConfig, McpTransport};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SAITEC_MCP_SERVER_NAME: &str = "SAITEC-Skills";

pub fn mcp_config_file() -> Result<PathBuf> {
    crate::storage::user_home_path(".saitec_tui/mcp.json")
}

pub fn ensure_bootstrap() -> Result<()> {
    let mcp_path = mcp_config_file()?;
    let mut config = if mcp_path.exists() {
        match McpConfig::load_from_file(&mcp_path) {
            Ok(config) => config,
            Err(err) => {
                crate::logging::warn(&format!(
                    "SAITEC MCP bootstrap skipped: failed to parse {}: {}",
                    mcp_path.display(),
                    err
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
            if server.transport != McpTransport::Http {
                server.transport = McpTransport::Http;
                changed = true;
            }
            if !server.command.is_empty() {
                server.command.clear();
                changed = true;
            }
            if !server.args.is_empty() {
                server.args.clear();
                changed = true;
            }
            if !server.env.is_empty() {
                server.env.clear();
                changed = true;
            }
            if server.url.as_deref() != Some(url.as_str()) {
                server.url = Some(url.clone());
                changed = true;
            }
            if let Some(ref key) = api_key {
                if server.headers.get("X-API-Key").map(String::as_str) != Some(key.as_str()) {
                    server.headers.insert("X-API-Key".to_string(), key.clone());
                    changed = true;
                }
            }
            if !server.shared {
                server.shared = true;
                changed = true;
            }
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

pub fn apply_runtime_env(config: &mut McpConfig) {
    let Some(server) = config.servers.get_mut(SAITEC_MCP_SERVER_NAME) else {
        return;
    };
    if let Some(api_key) = runtime_api_key() {
        server
            .headers
            .insert("X-API-Key".to_string(), api_key);
    }
    server.url = Some(crate::saitec::auth::saitec_mcp_url());
}

/// Reconnect SAITEC-Skills MCP server with fresh credentials.
/// Called after SAITEC login to inject the new API key into the running MCP server.
pub async fn reconnect_saitec_mcp() {
    let Some(pool) = crate::mcp::pool::get_shared_pool() else {
        crate::logging::debug("SAITEC MCP reconnect skipped: shared pool not initialized");
        return;
    };

    // Disconnect the existing SAITEC-Skills server
    pool.disconnect_server(SAITEC_MCP_SERVER_NAME).await;

    // Load fresh config (triggers apply_runtime_env with current credentials)
    let config = McpConfig::load();
    let Some(server_config) = config.servers.get(SAITEC_MCP_SERVER_NAME) else {
        crate::logging::warn(&format!(
            "SAITEC MCP reconnect skipped: {} not found in config",
            SAITEC_MCP_SERVER_NAME
        ));
        return;
    };

    // Reconnect with fresh credentials
    if let Err(err) = pool
        .connect_server(SAITEC_MCP_SERVER_NAME, server_config)
        .await
    {
        crate::logging::warn(&format!("SAITEC MCP reconnect failed: {:#}", err));
    } else {
        crate::logging::info("SAITEC MCP reconnected with fresh credentials");
    }
}

/// Disconnect SAITEC-Skills MCP server.
/// Called after SAITEC logout to remove the API key from the running MCP server.
pub async fn disconnect_saitec_mcp() {
    let Some(pool) = crate::mcp::pool::get_shared_pool() else {
        crate::logging::debug("SAITEC MCP disconnect skipped: shared pool not initialized");
        return;
    };

    pool.disconnect_server(SAITEC_MCP_SERVER_NAME).await;
    crate::logging::info("SAITEC MCP disconnected after logout");
}

fn runtime_api_key() -> Option<String> {
    crate::subscription_catalog::configured_api_key().or_else(|| {
        crate::saitec::auth::load_session()
            .ok()
            .flatten()
            .and_then(|session| {
                let trimmed = session.api_key.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpConfig;

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
            email: None,
            phone: None,
            display_name: None,
            api_key_id: None,
            api_key_name: None,
            api_key_created_at: None,
            api_key_expires_at: None,
            last_validated_at: None,
        })
        .unwrap();

        // Run the bootstrap.
        crate::saitec::mcp::ensure_bootstrap().unwrap();

        let path = crate::saitec::mcp::mcp_config_file().unwrap();
        let cfg = McpConfig::load_from_file(&path).unwrap();
        let saitec = cfg
            .servers
            .get(crate::saitec::mcp::SAITEC_MCP_SERVER_NAME)
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

    #[test]
    fn ensure_bootstrap_skips_when_no_api_key_present() {
        let _guard = crate::storage::lock_test_env();
        let temp = tempfile::TempDir::new().unwrap();
        crate::env::set_var("JCODE_HOME", temp.path());
        crate::env::remove_var("SAITEC_API_KEY");

        // Even without an API key, bootstrap should still succeed (it just
        // writes an entry without the X-API-Key header).
        crate::saitec::mcp::ensure_bootstrap().unwrap();

        let path = temp.path().join("external").join(".saitec_tui").join("mcp.json");
        if path.exists() {
            let cfg = McpConfig::load_from_file(&path).unwrap();
            let saitec = cfg
                .servers
                .get(SAITEC_MCP_SERVER_NAME)
                .expect("SAITEC-Skills entry must be written");
            assert_eq!(saitec.transport, McpTransport::Http);
            assert!(saitec.headers.get("X-API-Key").is_none());
        }
    }

    #[test]
    fn ensure_bootstrap_preserves_existing_servers_and_migrates_saitec_to_http() {
        let _guard = crate::storage::lock_test_env();
        let temp = tempfile::TempDir::new().unwrap();
        crate::env::set_var("JCODE_HOME", temp.path());
        crate::env::set_var("SAITEC_API_KEY", "sk-refresh");

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
        )
        .unwrap();

        crate::saitec::mcp::ensure_bootstrap().unwrap();

        let config = McpConfig::load_from_file(&mcp_path).unwrap();
        let existing = config.servers.get("existing-server").unwrap();
        assert_eq!(existing.command, "existing-bin");
        assert_eq!(existing.args, vec!["--flag"]);
        assert_eq!(existing.env.get("EXISTING"), Some(&"1".to_string()));

        let saitec = config.servers.get(SAITEC_MCP_SERVER_NAME).unwrap();
        assert_eq!(saitec.transport, McpTransport::Http);
        assert!(saitec.command.is_empty(), "stdio fields should be cleared");
        assert!(saitec.args.is_empty());
        assert!(saitec.env.is_empty());
        assert_eq!(
            saitec.headers.get("X-API-Key").map(String::as_str),
            Some("sk-refresh")
        );
        assert!(saitec.url.as_deref().unwrap().ends_with("/mcp"));
    }
}
