use anyhow::Result;
use std::path::PathBuf;

pub fn home_dir() -> Result<PathBuf> {
    crate::storage::jcode_dir()
}

pub fn auth_file() -> Result<PathBuf> {
    Ok(home_dir()?.join("auth.json"))
}
