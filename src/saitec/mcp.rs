use crate::mcp::{McpConfig, McpServerConfig};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SAITEC_MCP_SERVER_NAME: &str = "SAITEC-Skills";
pub const SAITEC_TUI_PYTHON: &str = "SAITEC_TUI_PYTHON";
pub const SAITEC_SKILLS_ROOT: &str = "SAITEC_SKILLS_ROOT";

pub fn mcp_config_file() -> Result<PathBuf> {
    crate::storage::user_home_path(".saitec_tui/mcp.json")
}

pub fn ensure_bootstrap() -> Result<()> {
    let Some(server_script) = resolve_server_script() else {
        crate::logging::warn(
            "SAITEC MCP bootstrap skipped: vendored mcp_server/server.py not found",
        );
        return Ok(());
    };

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

    let desired_command = python_command();
    let desired_args = vec![server_script.display().to_string()];
    let desired_env = default_env(server_script.parent());
    let mut changed = false;

    match config.servers.get_mut(SAITEC_MCP_SERVER_NAME) {
        Some(server) => {
            if server.command != desired_command {
                server.command = desired_command;
                changed = true;
            }
            if server.args != desired_args {
                server.args = desired_args;
                changed = true;
            }
            for (key, value) in desired_env {
                if server.env.get(&key) != Some(&value) {
                    server.env.insert(key, value);
                    changed = true;
                }
            }
            if !server.shared {
                server.shared = true;
                changed = true;
            }
        }
        None => {
            config.servers.insert(
                SAITEC_MCP_SERVER_NAME.to_string(),
                McpServerConfig {
                    command: desired_command,
                    args: desired_args,
                    env: desired_env,
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
        server.env.insert(
            crate::subscription_catalog::JCODE_API_KEY_ENV.to_string(),
            api_key,
        );
    }

    server.env.insert(
        "CORE_API_BASE".to_string(),
        crate::saitec::auth::core_api_base(),
    );

    if let Ok(home) = crate::saitec::paths::home_dir() {
        server
            .env
            .insert("SAITEC_TUI_HOME".to_string(), home.display().to_string());
    }
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

fn python_command() -> String {
    std::env::var(SAITEC_TUI_PYTHON).unwrap_or_else(|_| "python".to_string())
}

fn default_env(server_dir: Option<&Path>) -> HashMap<String, String> {
    let mut env = HashMap::from([("PYTHONIOENCODING".to_string(), "utf-8".to_string())]);
    if let Some(dir) = server_dir {
        env.insert("PYTHONPATH".to_string(), dir.display().to_string());
    }
    env
}

fn resolve_server_script() -> Option<PathBuf> {
    resolve_skills_root()
        .map(|root| root.join("mcp_server").join("server.py"))
        .filter(|path| path.exists())
}

fn resolve_skills_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var(SAITEC_SKILLS_ROOT) {
        let path = PathBuf::from(root);
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        let exe_dir = current_exe.parent();
        if let Some(dir) = exe_dir {
            for ancestor in dir.ancestors().take(4) {
                let release_relative = ancestor.join("resources").join("SAITEC-Skills");
                if release_relative.exists() {
                    return Some(release_relative);
                }

                let sibling = ancestor.join("SAITEC-Skills");
                if sibling.exists() {
                    return Some(sibling);
                }
            }
        }
    }

    std::env::current_dir()
        .ok()
        .and_then(|cwd| find_vendor_root_from(&cwd))
}

fn find_vendor_root_from(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        let candidate = ancestor.join("_vendor").join("SAITEC-Skills");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}
