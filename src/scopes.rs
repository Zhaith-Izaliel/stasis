use std::fmt;

/// All standard Stasis logging scopes
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    Config,
    Cli,
    Client,
    Core,
    Daemon,
    Ipc,
    Log,
    MediaBridge,
    Utils,
    AppError,
    Wayland,
    Media,
    AppInhibit,
    LibInput,
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Scope::Config => "Config",
            Scope::Cli => "CLI",
            Scope::Client => "Client",
            Scope::Core => "Core",
            Scope::Daemon => "Daemon",
            Scope::Ipc => "IPC",
            Scope::Log => "Log",
            Scope::MediaBridge => "MediaBridge",
            Scope::Utils => "Utils",
            Scope::AppError => "AppError",
            Scope::Wayland => "Wayland",
            Scope::Media => "Media",
            Scope::AppInhibit => "AppInhibit",
            Scope::LibInput => "LibInput",
        };
        write!(f, "{}", s)
    }
}
