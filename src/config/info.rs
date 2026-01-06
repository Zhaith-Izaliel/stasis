use std::{collections::BTreeSet, time::Duration};
use crate::{config::model::StasisConfig, core::utils};

#[derive(Debug, Clone, Copy)]
pub struct InfoSections {
    pub status: bool,
    pub config: bool,
    pub actions: bool,
}

impl Default for InfoSections {
    fn default() -> Self {
        Self {
            status: true,
            config: true,
            actions: true,
        }
    }
}

impl InfoSections {
    // Helper to count how many sections are enabled
    fn count_enabled(&self) -> usize {
        (self.status as usize) + (self.config as usize) + (self.actions as usize)
    }
}

impl StasisConfig {
    pub fn pretty_print(
        &self,
        idle_time: Option<Duration>,
        uptime: Option<Duration>,
        is_paused: Option<bool>,
        is_manually_paused: Option<bool>,
        app_blocking: Option<bool>,
        media_blocking: Option<bool>,
        media_bridge_active: Option<bool>,
        active_profile: Option<&str>,
        available_profiles: Option<&[String]>,
        sections: InfoSections,
    ) -> String {
        let mut out = String::new();
        
        // Check if any action has per-action notification timeout
        let has_per_action_timeouts = self.actions.iter()
            .any(|a| a.notify_seconds_before.is_some());
        
        // Calculate the global pipe position
        let all_labels = vec![
            "Active Profile", "Idle Time", "Uptime", "Paused", "Manually Paused", 
            "App Blocking", "Media Blocking", "Media Bridge",
            "PreSuspendCommand", "MonitorMedia", "IgnoreRemoteMedia", 
            "RespectInhibitors", "NotifyOnUnpause", "NotifyBeforeAction",
            "NotifySecondsBefore", "DebounceSeconds", "LidCloseAction", "LidOpenAction", 
            "LockDetectionType", "InhibitApps", "Profiles",
            "Timeout", "Kind", "Command", "LockCommand", "Notification", 
            "NotifySecondsBefore", "ResumeCommand"
        ];
        
        let max_label = all_labels.iter()
            .map(|s| s.len())
            .max()
            .unwrap_or(0);
        
        // Only show section headers if displaying multiple sections
        let show_headers = sections.count_enabled() > 1;
        
        // Status section
        if sections.status {
            if show_headers {
                out.push_str("◆ STATUS\n");
            }
            
            let profile_display = active_profile.unwrap_or("base config");
            out.push_str(&format!("  {:<width$}    │ {}\n", "Active Profile", profile_display, width = max_label));
            
            if let Some(idle) = idle_time {
                out.push_str(&format!("  {:<width$}    │ {}\n", "Idle Time", utils::format_duration(idle), width = max_label));
            }
            if let Some(up) = uptime {
                out.push_str(&format!("  {:<width$}    │ {}\n", "Uptime", utils::format_duration(up), width = max_label));
            }
            if let Some(paused) = is_paused {
                let indicator = if paused { "●" } else { "○" };
                out.push_str(&format!("  {:<width$}    │ {} {}\n", "Paused", indicator, paused, width = max_label));
            }
            if let Some(manually_paused) = is_manually_paused {
                let indicator = if manually_paused { "●" } else { "○" };
                out.push_str(&format!("  {:<width$}    │ {} {}\n", "Manually Paused", indicator, manually_paused, width = max_label));
            }
            if let Some(app_paused) = app_blocking {
                let indicator = if app_paused { "●" } else { "○" };
                out.push_str(&format!("  {:<width$}    │ {} {}\n", "App Blocking", indicator, app_paused, width = max_label));
            }
            if let Some(media_paused) = media_blocking {
                let indicator = if media_paused { "●" } else { "○" };
                out.push_str(&format!("  {:<width$}    │ {} {}\n", "Media Blocking", indicator, media_paused, width = max_label));
            }
            if let Some(bridge_active) = media_bridge_active {
                let indicator = if bridge_active { "●" } else { "○" };
                out.push_str(&format!("  {:<width$}    │ {} {}\n", "Media Bridge", indicator, bridge_active, width = max_label));
            }
            
            out.push('\n');
        }
        
        // Config section
        if sections.config {
            if show_headers {
                out.push_str("◆ CONFIGURATION\n");
            }
            
            out.push_str(&format!(
                "  {:<width$}    │ {}\n",
                "PreSuspendCommand",
                self.pre_suspend_command.as_deref().unwrap_or("none"),
                width = max_label
            ));
            out.push_str(&format!(
                "  {:<width$}    │ {}\n",
                "MonitorMedia",
                if self.monitor_media { "✓ enabled" } else { "✗ disabled" },
                width = max_label
            ));
            out.push_str(&format!(
                "  {:<width$}    │ {}\n", 
                "IgnoreRemoteMedia",
                if self.ignore_remote_media { "✓ enabled" } else { "✗ disabled" },
                width = max_label
            ));
            out.push_str(&format!(
                "  {:<width$}    │ {}\n",
                "RespectInhibitors",
                if self.respect_wayland_inhibitors { "✓ enabled" } else { "✗ disabled" },
                width = max_label
            ));
            out.push_str(&format!(
                "  {:<width$}    │ {}\n",
                "NotifyOnUnpause",
                if self.notify_on_unpause { "✓ enabled" } else { "✗ disabled" },
                width = max_label
            ));
            out.push_str(&format!(
                "  {:<width$}    │ {}\n",
                "NotifyBeforeAction",
                if self.notify_before_action { "✓ enabled" } else { "✗ disabled" },
                width = max_label
            ));
            out.push_str(&format!(
                "  {:<width$}    │ {:?}\n",
                "LockDetectionType",
                self.lock_detection_type,
                width = max_label
            ));
            
            if !has_per_action_timeouts {
                out.push_str(&format!("  {:<width$}    │ {}s\n", "NotifySecondsBefore", self.notify_seconds_before, width = max_label));
            } else {
                out.push_str(&format!("  {:<width$}    │ {}\n", "NotifySecondsBefore", "per-action", width = max_label));
            }
            
            out.push_str(&format!("  {:<width$}    │ {}s\n", "DebounceSeconds", self.debounce_seconds, width = max_label));
            out.push_str(&format!("  {:<width$}    │ {}\n", "LidCloseAction", self.lid_close_action, width = max_label));
            out.push_str(&format!("  {:<width$}    │ {}\n", "LidOpenAction", self.lid_open_action, width = max_label));
            
            let apps = if self.inhibit_apps.is_empty() {
                "none".to_string()
            } else {
                self.inhibit_apps
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!("  {:<width$}    │ {}\n", "InhibitApps", apps, width = max_label));
            
            let profiles_display = if let Some(profiles) = available_profiles {
                if profiles.is_empty() {
                    "none".to_string()
                } else {
                    profiles.join(", ")
                }
            } else {
                "none".to_string()
            };
            out.push_str(&format!("  {:<width$}    │ {}\n", "Profiles", profiles_display, width = max_label));
            
            out.push('\n');
        }
        
        // Actions section
        if sections.actions {
            if show_headers {
                out.push_str("◆ ACTIONS\n");
            }
            
            let mut seen_groups = BTreeSet::new();
            let mut action_counter = 1;
            
            for action in &self.actions {
                let group = if action.name.starts_with("ac.") {
                    "AC Power"
                } else if action.name.starts_with("battery.") {
                    "Battery Power"
                } else {
                    "Desktop"
                };
                
                if seen_groups.insert(group) {
                    out.push_str(&format!("  [{group}]\n"));
                }
                
                let display_name = action.name
                    .strip_prefix("ac.")
                    .or_else(|| action.name.strip_prefix("battery."))
                    .unwrap_or(&action.name);
                
                out.push_str(&format!(
                    "  {}. {}\n",
                    action_counter,
                    display_name
                ));
                
                out.push_str(&format!("     {:<width$} │ {}s\n", "Timeout", action.timeout, width = max_label));
                out.push_str(&format!("     {:<width$} │ {}\n", "Kind", action.kind, width = max_label));
                out.push_str(&format!("     {:<width$} │ {}\n", "Command", action.command, width = max_label));
                
                if let Some(lock_cmd) = &action.lock_command {
                    out.push_str(&format!("     {:<width$} │ {}\n", "LockCommand", lock_cmd, width = max_label));
                }
                if let Some(notification) = &action.notification {
                    out.push_str(&format!("     {:<width$} │ {}\n", "Notification", notification, width = max_label));
                    
                    if has_per_action_timeouts {
                        if let Some(notify_seconds) = action.notify_seconds_before {
                            out.push_str(&format!("     {:<width$} │ {}s\n", "NotifySecondsBefore", notify_seconds, width = max_label));
                        } else {
                            out.push_str(&format!("     {:<width$} │ none\n", "NotifySecondsBefore", width = max_label));
                        }
                    }
                }
                if let Some(resume_cmd) = &action.resume_command {
                    out.push_str(&format!("     {:<width$} │ {}\n", "ResumeCommand", resume_cmd, width = max_label));
                }
                
                action_counter += 1;
            }
        }
        
        out
    }
}
