use eyre::{Result, WrapErr};
use rune_cfg::RuneConfig;
use std::path::PathBuf;

use crate::{sdebug, sinfo};
use crate::config::model::{StasisConfig, CombinedConfig};

use super::base::parse_base_stasis_config;
use super::profiles::load_profiles;

/// Loads the merged configuration from default, system, or user paths
pub fn load_merged_config() -> Result<RuneConfig> {
    let internal_default = include_str!("../../../examples/stasis.rune");
    let mut config = RuneConfig::from_str(internal_default)
        .wrap_err("failed to parse internal default config")?;

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
                .wrap_err_with(|| format!("failed to load user config from {}", user_path.display()))?;
            sdebug!("Stasis", "Loaded config from: {}", user_path.display());
            return Ok(config);
        }
    }

    if system_path.exists() {
        config = RuneConfig::from_file(&system_path)
            .wrap_err_with(|| format!("failed to load system config from {}", system_path.display()))?;
        sdebug!("Stasis", "Loaded config from: {}", system_path.display()); 
        return Ok(config);
    }

    if share_path.exists() {
        config = RuneConfig::from_file(&share_path)
            .wrap_err_with(|| format!("failed to load shared example config from {}", share_path.display()))?;
        sdebug!("Stasis", "Loaded config from: {}", share_path.display());
        return Ok(config);
    }

    sdebug!("Stasis", "Using internal default configuration");
    Ok(config)
}

/// Loads the base Stasis configuration
pub fn load_config() -> Result<StasisConfig> {
    let config = load_merged_config().wrap_err("failed to load configuration")?;
    parse_base_stasis_config(&config)
}

/// Loads the combined configuration including base and all profiles
pub fn load_combined_config() -> Result<CombinedConfig> {
    let config = load_merged_config().wrap_err("failed to load configuration")?;
    let base = parse_base_stasis_config(&config)?;
    let profiles = load_profiles(&config, &base)?;
    
    if !profiles.is_empty() {
        let profile_names: Vec<_> = profiles.iter().map(|p| p.name.as_str()).collect();
        sinfo!("Stasis", "Profiles loaded: {}", profile_names.join(", "));
    }
    
    Ok(CombinedConfig {
        base,
        profiles,
        active_profile: None,
    })
}
