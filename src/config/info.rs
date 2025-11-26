use std::{collections::BTreeSet, time::Duration};
use crate::{config::model::StasisConfig, core::utils};

impl StasisConfig {
    pub fn pretty_print(
        &self,
        idle_time: Option<Duration>,
        uptime: Option<Duration>,
        is_paused: Option<bool>,
        is_manually_paused: Option<bool>,
        app_blocking: Option<bool>,
        media_blocking: Option<bool>,
    ) -> String {
        let mut out = String::new();
        out.push_str("Status:\n");
        
        if let Some(idle) = idle_time {
            out.push_str(&format!("  Idle Time          = {}\n", utils::format_duration(idle)));
        }
        if let Some(up) = uptime {
            out.push_str(&format!("  Uptime             = {}\n", utils::format_duration(up)));
        }
        if let Some(paused) = is_paused {
            out.push_str(&format!("  Paused             = {}\n", paused));
        }
        if let Some(manually_paused) = is_manually_paused {
            out.push_str(&format!("  Manually Paused    = {}\n", manually_paused));
        }
        if let Some(app_paused) = app_blocking {
            out.push_str(&format!("  App Blocking       = {}\n", app_paused));
        }
        if let Some(media_paused) = media_blocking {
            out.push_str(&format!("  Media Blocking     = {}\n", media_paused));
        }
        
        // General settings
        out.push_str("\nConfig:\n");
        out.push_str(&format!(
            "  PreSuspendCommand  = {}\n",
            self.pre_suspend_command.as_deref().unwrap_or("-")
        ));
        out.push_str(&format!(
            "  MonitorMedia       = {}\n",
            if self.monitor_media { "true" } else { "false" }
        ));
        out.push_str(&format!("  IgnoreRemoteMedia  = {}\n", self.ignore_remote_media));
        out.push_str(&format!(
            "  RespectInhibitors  = {}\n",
            if self.respect_wayland_inhibitors { "true" } else { "false" }
        ));
        out.push_str(&format!(
            "  NotifyOnUnpause    = {}\n",
            if self.notify_on_unpause { "true" } else { "false" }
        ));
        out.push_str(&format!("  DebounceSeconds    = {}\n", self.debounce_seconds));
        out.push_str(&format!("  LidCloseAction     = {}\n", self.lid_close_action));
        out.push_str(&format!("  LidOpenAction      = {}\n", self.lid_open_action));
        
        let apps = if self.inhibit_apps.is_empty() {
            "-".to_string()
        } else {
            self.inhibit_apps
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        out.push_str(&format!("  InhibitApps        = {}\n", apps));
        
        // Actions
        out.push_str("\nActions (fires in sequence):\n");
        // Track groups in order of first occurrence
        let mut seen_groups = BTreeSet::new();
        let mut action_counter = 1;
        
        for action in &self.actions {
            let group = if action.name.starts_with("ac.") {
                "AC"
            } else if action.name.starts_with("battery.") {
                "Battery"
            } else {
                "Desktop"
            };
            
            // Print group header only once
            if seen_groups.insert(group) {
                out.push_str(&format!("  [{}]\n", group));
            }
            
            // Strip the prefix from the action name for display
            let display_name = action.name
                .strip_prefix("ac.")
                .or_else(|| action.name.strip_prefix("battery."))
                .unwrap_or(&action.name);
            
            // Action number and name
            out.push_str(&format!("    {}. {}\n", action_counter, display_name));
            
            // Aligned key-value pairs
            out.push_str(&format!("       Timeout        : {}\n", action.timeout));
            out.push_str(&format!("       Kind           : {}\n", action.kind));
            out.push_str(&format!("       Command        : \"{}\"\n", action.command));
            
            if let Some(lock_cmd) = &action.lock_command {
                out.push_str(&format!("       LockCommand    : \"{}\"\n", lock_cmd));
            }
            if let Some(resume_cmd) = &action.resume_command {
                out.push_str(&format!("       ResumeCommand  : \"{}\"\n", resume_cmd));
            }
            
            // Blank line between actions for readability
            out.push('\n');
            
            action_counter += 1;
        }
        
        out
    }
}
