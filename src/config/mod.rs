use std::{path::PathBuf};
use eyre::Result;

pub mod bootstrap;
pub mod info;
pub mod model;
pub mod parser;

/// Determine default config path
pub async fn get_config_path() -> Result<PathBuf> {
    // 1. Check user config first
    if let Some(mut path) = dirs::home_dir() {
        path.push(".config/stasis/stasis.rune");
        if path.exists() {
            return Ok(path);
        }
    }
    
    // 2. Check system config
    let system_path = PathBuf::from("/etc/stasis/stasis.rune");
    if system_path.exists() {
        return Ok(system_path);
    }
    
    // 3. Check shared examples as fallback
    let share_path = PathBuf::from("/usr/share/stasis/examples/stasis.rune");
    if share_path.exists() {
        return Ok(share_path);
    }
    
    Err(eyre::eyre!("Could not find stasis configuration file"))
}
