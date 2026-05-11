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
        crate::logging::warn("SAITEC MCP bootstrap skipped: vendored mcp_server/server.py not found");
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

    if config.servers.contains_key(SAITEC_MCP_SERVER_NAME) {
        return Ok(());
    }

    config.servers.insert(
        SAITEC_MCP_SERVER_NAME.to_string(),
        McpServerConfig {
            command: python_command(),
            args: vec![server_script.display().to_string()],
            env: default_env(server_script.parent()),
            shared: true,
        },
    );

    config.save_to_file(&mcp_path)?;
    crate::logging::info(&format!(
        "SAITEC MCP bootstrap wrote default config to {}",
        mcp_path.display()
    ));
    Ok(())
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
