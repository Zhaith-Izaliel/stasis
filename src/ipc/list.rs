use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::manager::Manager;

pub const LIST_HELP_MESSAGE: &str = r#"List various items in Stasis

Usage:
  stasis list actions   List available actions based on current config
  stasis list profiles  List available configuration profiles

Examples:
  stasis list actions
  stasis list profiles"#;

pub async fn handle_list_command(
    manager: Arc<Mutex<Manager>>,
    args: &str,
) -> Result<String, String> {
    let args = args.trim();
    
    if args.eq_ignore_ascii_case("help") || args == "-h" || args == "--help" || args.is_empty() {
        return Err(LIST_HELP_MESSAGE.to_string());
    }

    match args {
        "actions" => handle_list_actions(manager).await,
        "profiles" => handle_list_profiles(manager).await,
        _ => Err(format!(
            "Unknown list subcommand: '{}'\n\n{}",
            args,
            LIST_HELP_MESSAGE
        )),
    }
}

async fn handle_list_actions(manager: Arc<Mutex<Manager>>) -> Result<String, String> {
    let mgr = manager.lock().await;
    let actions = mgr.state.get_active_actions();
    
    if actions.is_empty() {
        return Ok("No actions available".to_string());
    }
    
    let action_names: Vec<String> = actions
        .iter()
        .map(|a| a.name.clone())
        .collect();
    
    Ok(action_names.join(", "))
}

async fn handle_list_profiles(manager: Arc<Mutex<Manager>>) -> Result<String, String> {
    let mgr = manager.lock().await;
    let profiles = mgr.list_profiles();
    
    if profiles.is_empty() {
        return Ok("No profiles defined".to_string());
    }
    
    let current = mgr.current_profile();
    let mut response = String::from("Available profiles:\n");
    
    for profile_name in profiles {
        if Some(&profile_name) == current.as_ref() {
            response.push_str(&format!("  * {} (active)\n", profile_name));
        } else {
            response.push_str(&format!("  - {}\n", profile_name));
        }
    }
    
    if current.is_none() {
        response.push_str("\nCurrently using base configuration");
    }
    
    Ok(response)
}
