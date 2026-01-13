use std::fmt;
use std::io;
use rune_cfg::RuneConfig;
use std::path::PathBuf;
use crate::config::model::{StasisConfig, CombinedConfig};
use super::actions::ActionParseError;
use super::base::parse_base_stasis_config;
use super::profiles::load_profiles;
use eventline::{event_info_scoped, event_debug_scoped};

#[derive(Debug)]
pub enum ConfigParseError {
    ActionError(ActionParseError),
    RuneConfig(String),
    Io(io::Error),
}

impl fmt::Display for ConfigParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigParseError::ActionError(e) => write!(f, "Action parse error: {}", e),
            ConfigParseError::RuneConfig(msg) => write!(f, "Configuration error: {}", msg),
            ConfigParseError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ConfigParseError {}

impl From<ActionParseError> for ConfigParseError {
    fn from(err: ActionParseError) -> Self {
        ConfigParseError::ActionError(err)
    }
}

impl From<io::Error> for ConfigParseError {
    fn from(err: io::Error) -> Self {
        ConfigParseError::Io(err)
    }
}

/// Loads the merged configuration from default, system, or user paths
pub async fn load_merged_config() -> Result<RuneConfig, ConfigParseError> {
    let internal_default = include_str!("../../../examples/stasis.rune");
    let mut config = RuneConfig::from_str(internal_default)
        .map_err(|e| ConfigParseError::RuneConfig(format!("failed to parse internal default config: {}", e)))?;
    
    let user_path = dirs::home_dir()
        .map(|mut p| {
            p.push(".config/stasis/stasis.rune");
            p
        });
    
    let system_path = PathBuf::from("/etc/stasis/stasis.rune");
    let share_path = PathBuf::from("/usr/share/stasis/examples/stasis.rune");
    
    if let Some(user_path) = user_path {
        if user_path.exists() {
            config = RuneConfig::from_file(&user_path)
                .map_err(|e| ConfigParseError::RuneConfig(format!("failed to load user config from {}: {}", user_path.display(), e)))?;
            event_debug_scoped!("Stasis", "Loaded config from: {}", user_path.display()).await;
            return Ok(config);
        }
    }
    
    if system_path.exists() {
        config = RuneConfig::from_file(&system_path)
            .map_err(|e| ConfigParseError::RuneConfig(format!("failed to load system config from {}: {}", system_path.display(), e)))?;
        event_debug_scoped!("Stasis", "Loaded config from: {}", system_path.display()).await; 
        return Ok(config);
    }
    
    if share_path.exists() {
        config = RuneConfig::from_file(&share_path)
            .map_err(|e| ConfigParseError::RuneConfig(format!("failed to load shared example config from {}: {}", share_path.display(), e)))?;
        event_debug_scoped!("Stasis", "Loaded config from: {}", share_path.display()).await;
        return Ok(config);
    }
    
    event_debug_scoped!("Stasis", "Using internal default configuration").await;
    Ok(config)
}

/// Loads the base Stasis configuration
pub async fn load_config() -> Result<StasisConfig, ConfigParseError> {
    let config = load_merged_config().await?;
    parse_base_stasis_config(&config)
}

/// Loads the combined configuration including base and all profiles
pub async fn load_combined_config() -> Result<CombinedConfig, ConfigParseError> {
    let config = load_merged_config().await?;
    let base = parse_base_stasis_config(&config)?;
    let profiles = load_profiles(&config, &base)?;
    
    // Collect names as owned Strings to avoid borrowing `profiles` while moving it
    if !profiles.is_empty() {
        let profile_names: Vec<String> = profiles.iter().map(|p| p.name.clone()).collect();
        event_info_scoped!("Stasis", "Profiles loaded: {}", profile_names.join(", ")).await;
    }
    
    Ok(CombinedConfig {
        base,
        profiles,
        active_profile: None,
    })
}
