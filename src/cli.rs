use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "Stasis",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct Args {
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    
    #[arg(short, long, action)]
    pub verbose: bool,
    
    #[command(subcommand)]
    pub command: Option<Command>
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(about = "Reload the configuration without restarting Stasis")]
    Reload,
    
    #[command(about = "Pause all timers indefinitely, for a duration, or until a time", disable_help_flag = true)]
    Pause {
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            help = "Optional: 'for <duration>' (e.g., 'for 5m') or 'until <time>' (e.g., 'until 1:30pm')"
        )]
        args: Vec<String>,
    },
    
    #[command(about = "Resume timers after a pause")]
    Resume,
    
    #[command(about = "List actions or profiles", disable_help_flag = true)]
    List {
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            help = "Subcommand: 'actions' or 'profiles'"
        )]
        args: Vec<String>,
    },
    
    #[command(about = "Manually trigger a specific idle action by name")]
    Trigger { 
        #[arg(help = "Action name to trigger (e.g., 'brightness', 'dpms', 'lock_screen', 'pre_suspend', 'suspend')")]
        step: String,
    },
    
    #[command(about = "Toggle manual idle inhibition (for status bars such as Waybar)")]
    ToggleInhibit,
    
    #[command(about = "Stop the currently running instances of Stasis")]
    Stop,
    
    #[command(about = "Display current session information")]
    Info {
        #[arg(long, help = "Output as JSON (for Waybar or scripts)")]
        json: bool,
        
        #[arg(help = "Section to display: 'status', 'config', 'actions', or combinations like 'status,config'")]
        section: Option<String>,
    },
 
    #[command(about = "Dump recent log lines (for debugging)")]
    Dump {
        #[arg(default_value_t = 20, help = "Number of recent lines to show")]
        lines: usize,
    },
    
    #[command(about = "Switch to a specific profile or back to base config")]
    Profile {
        #[arg(help = "Profile name to activate, or 'none' to use base config")]
        name: String,
    },
}
