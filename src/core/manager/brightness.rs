use std::fs;
use std::path::Path;
use tokio::process::Command;

use crate::log::{log_error_message, log_message};
use crate::core::manager::state::ManagerState;

#[derive(Clone, Debug)]
struct BrightnessState {
    value: u32,
    max_brightness: u32,
    device: String,
}

pub async fn capture_brightness(state: &mut ManagerState) -> Result<(), std::io::Error> {
    // Try sysfs method first
    if let Some(sys_brightness) = capture_sysfs_brightness() {
        log_message(&format!("Captured brightness via sysfs: {}/{} on device '{}'", 
            sys_brightness.value, sys_brightness.max_brightness, sys_brightness.device));

        // Store the full u32 value - don't truncate!
        state.previous_brightness = Some(sys_brightness.value);
        state.max_brightness = Some(sys_brightness.max_brightness);
        state.brightness_device = Some(sys_brightness.device);
        return Ok(());
    }

    // Fallback to brightnessctl
    log_message("Falling back to brightnessctl for brightness capture");
    match Command::new("brightnessctl").arg("get").output().await {
        Ok(out) if out.status.success() => {
            let val = String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse::<u32>()
                .unwrap_or(0);
            state.previous_brightness = Some(val);
            log_message(&format!("Captured brightness via brightnessctl: {}", val));
        }
        Ok(out) => {
            log_error_message(&format!("brightnessctl get failed: {:?}", out.status));
        }
        Err(e) => {
            log_error_message(&format!("Failed to execute brightnessctl: {}", e));
        }
    }

    Ok(())
}

pub async fn restore_brightness(state: &mut ManagerState) -> Result<(), std::io::Error> {
    if let Some(level) = state.previous_brightness {
        log_message(&format!("Attempting to restore brightness to {}", level));

        // Try sysfs restore first if we have device info
        if let (Some(device), Some(_max)) = (&state.brightness_device, state.max_brightness) {
            if restore_sysfs_brightness_to_device(device, level).is_ok() {
                log_message("Brightness restored via sysfs");
                state.previous_brightness = None;
                state.max_brightness = None;
                state.brightness_device = None;
                return Ok(());
            }
        }

        // Fallback to generic sysfs restore
        if restore_sysfs_brightness(level).is_ok() {
            log_message("Brightness restored via sysfs (generic)");
        } else {
            log_message("Falling back to brightnessctl for brightness restore");
            if let Err(e) = Command::new("brightnessctl")
                .arg("set")
                .arg(level.to_string())
                .output()
                .await
            {
                log_error_message(&format!("Failed to restore brightness: {}", e));
            }
        }

        // Reset stored brightness
        state.previous_brightness = None;
        state.max_brightness = None;
        state.brightness_device = None;
    }
    Ok(())
}

fn capture_sysfs_brightness() -> Option<BrightnessState> {
    let base = Path::new("/sys/class/backlight");
    let device_entry = fs::read_dir(base).ok()?.next()?;
    let device = device_entry.ok()?.file_name().to_string_lossy().to_string();

    let current = fs::read_to_string(base.join(&device).join("brightness")).ok()?;
    let max = fs::read_to_string(base.join(&device).join("max_brightness")).ok()?;
    
    Some(BrightnessState {
        value: current.trim().parse().ok()?,
        max_brightness: max.trim().parse().ok()?,
        device,
    })
}

fn restore_sysfs_brightness_to_device(device: &str, value: u32) -> Result<(), std::io::Error> {
    let base = Path::new("/sys/class/backlight");
    let path = base.join(device).join("brightness");
    fs::write(&path, value.to_string())?;
    Ok(())
}

fn restore_sysfs_brightness(value: u32) -> Result<(), std::io::Error> {
    let base = Path::new("/sys/class/backlight");

    let entry = fs::read_dir(base)
        .ok()
        .and_then(|mut it| it.next())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No backlight device found"))??;

    let device = entry.file_name().to_string_lossy().to_string();
    let path = base.join(device).join("brightness");
    fs::write(&path, value.to_string())?;

    Ok(())
}
