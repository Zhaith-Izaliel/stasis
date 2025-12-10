use std::fs;
use std::path::Path;
use tokio::process::Command;

use crate::core::manager::state::ManagerState;
use crate::{sinfo, sdebug, serror};

#[derive(Clone, Debug)]
struct BrightnessState {
    value: u32,
    max_brightness: u32,
    device: String,
}

pub async fn capture_brightness(state: &mut ManagerState) -> Result<(), std::io::Error> {
    // Try sysfs method first
    if let Some(sys_brightness) = capture_sysfs_brightness() {
        sdebug!(
            "Brightness",
            "Captured brightness via sysfs: {}/{} on device '{}'",
            sys_brightness.value,
            sys_brightness.max_brightness,
            sys_brightness.device
        );

        // Use the BrightnessManager to store state
        state.brightness.store(
            sys_brightness.value,
            sys_brightness.max_brightness,
            sys_brightness.device,
        );
        return Ok(());
    }

    // Fallback to brightnessctl
    sinfo!("Brightness", "Falling back to brightnessctl for brightness capture");

    match Command::new("brightnessctl").arg("get").output().await {
        Ok(out) if out.status.success() => {
            let val = String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse::<u32>()
                .unwrap_or(0);
            state.brightness.store_simple(val);
            sdebug!("Brightness", "Captured brightness via brightnessctl: {}", val);
        }
        Ok(out) => {
            serror!("Brightness", "brightnessctl get failed: {:?}", out.status);
        }
        Err(e) => {
            serror!("Brightness", "Failed to execute brightnessctl: {}", e);
        }
    }

    Ok(())
}

pub async fn restore_brightness(state: &mut ManagerState) -> Result<(), std::io::Error> {
    let (brightness, device, _max) = state.brightness.get_restore_info();

    if let Some(level) = brightness {
        sinfo!("Brightness", "Attempting to restore brightness to {}", level);

        // Try sysfs restore first if we have device info
        if let Some(device_name) = device {
            if restore_sysfs_brightness_to_device(&device_name, level).is_ok() {
                sinfo!("Brightness", "Brightness restored via sysfs");
                state.brightness.clear();
                return Ok(());
            }
        }

        // Fallback to generic sysfs restore
        if restore_sysfs_brightness(level).is_ok() {
            sinfo!("Brightness", "Brightness restored via sysfs (generic)");
        } else {
            sinfo!("Brightness", "Falling back to brightnessctl for brightness restore");
            if let Err(e) = Command::new("brightnessctl")
                .arg("set")
                .arg(level.to_string())
                .output()
                .await
            {
                serror!("Brightness", "Failed to restore brightness: {}", e);
            }
        }

        state.brightness.clear();
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
