// This module provides an interface to an external media bridge (Python script + browser extension)
// that reports per-tab media playback state via Unix socket. It's treated as external infrastructure,
// similar to how we interface with MPRIS or D-Bus.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use serde_json::Value;

const BRIDGE_SOCKET: &str = "/tmp/media_bridge.sock";

/// Browser media state reported by the external bridge
#[derive(Debug, Clone, PartialEq)]
pub struct BrowserMediaState {
    pub playing: bool,
    pub tab_count: usize,
    pub playing_tabs: Vec<i32>,
}

impl BrowserMediaState {
    /// Create an empty state (no media playing)
    pub fn empty() -> Self {
        Self {
            playing: false,
            tab_count: 0,
            playing_tabs: vec![],
        }
    }

    /// Get the number of tabs currently playing media
    pub fn playing_tab_count(&self) -> usize {
        self.playing_tabs.len()
    }

    /// Check if state has changed compared to another state
    pub fn has_changed_from(&self, other: &Self) -> bool {
        self.playing != other.playing
            || self.tab_count != other.tab_count
            || self.playing_tabs != other.playing_tabs
    }
}

/// Check if the external media bridge is available
/// 
/// This checks for the Unix socket that the bridge creates.
/// The bridge is an external service (Python script + browser extension)
/// that must be running separately from stasis.
pub fn is_available() -> bool {
    std::path::Path::new(BRIDGE_SOCKET).exists()
}

/// Query the current media state from the external bridge
/// 
/// # Protocol
/// - Send: "status" command over Unix socket
/// - Receive: JSON response with format:
///   ```json
///   {
///     "playing": bool,
///     "tab_count": int,
///     "playing_tabs": [int]
///   }
///   ```
/// 
/// # Errors
/// Returns error string if:
/// - Cannot connect to socket (bridge not running)
/// - Communication failure
/// - Invalid JSON response
pub fn query_status() -> Result<BrowserMediaState, String> {
    let mut stream = UnixStream::connect(BRIDGE_SOCKET)
        .map_err(|e| format!("Failed to connect to media bridge: {}", e))?;
    
    stream
        .write_all(b"status")
        .map_err(|e| format!("Failed to send query: {}", e))?;
    
    let mut buffer = vec![0u8; 4096];
    let size = stream
        .read(&mut buffer)
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    if size == 0 {
        return Err("Empty response from bridge".to_string());
    }
    
    parse_bridge_response(&buffer[..size])
}

/// Parse JSON response from the bridge
fn parse_bridge_response(data: &[u8]) -> Result<BrowserMediaState, String> {
    let resp_str = String::from_utf8_lossy(data);
    let json: Value = serde_json::from_str(&resp_str)
        .map_err(|e| format!("Invalid JSON from bridge: {}", e))?;
    
    Ok(BrowserMediaState {
        playing: json["playing"].as_bool().unwrap_or(false),
        tab_count: json["tab_count"].as_u64().unwrap_or(0) as usize,
        playing_tabs: json["playing_tabs"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64())
                    .map(|i| i as i32)
                    .collect()
            })
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_state() {
        let state = BrowserMediaState::empty();
        assert!(!state.playing);
        assert_eq!(state.tab_count, 0);
        assert_eq!(state.playing_tab_count(), 0);
    }

    #[test]
    fn test_state_comparison() {
        let state1 = BrowserMediaState {
            playing: true,
            tab_count: 2,
            playing_tabs: vec![1, 2],
        };
        
        let state2 = BrowserMediaState {
            playing: true,
            tab_count: 2,
            playing_tabs: vec![1, 2],
        };
        
        let state3 = BrowserMediaState {
            playing: true,
            tab_count: 2,
            playing_tabs: vec![1, 3], // Different tab
        };

        assert!(!state1.has_changed_from(&state2));
        assert!(state1.has_changed_from(&state3));
    }

    #[test]
    fn test_parse_valid_response() {
        let json = r#"{"playing": true, "tab_count": 3, "playing_tabs": [1, 5, 9]}"#;
        let result = parse_bridge_response(json.as_bytes()).unwrap();
        
        assert!(result.playing);
        assert_eq!(result.tab_count, 3);
        assert_eq!(result.playing_tabs, vec![1, 5, 9]);
    }

    #[test]
    fn test_parse_empty_response() {
        let json = r#"{"playing": false, "tab_count": 0, "playing_tabs": []}"#;
        let result = parse_bridge_response(json.as_bytes()).unwrap();
        
        assert!(!result.playing);
        assert_eq!(result.tab_count, 0);
        assert!(result.playing_tabs.is_empty());
    }
}
