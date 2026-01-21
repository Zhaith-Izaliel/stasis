// Author: Dustin Pilgrim
// License: MIT

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;

use crate::core::events::Event;
use crate::core::manager_msg::ManagerMsg;

// Any new pause/resume bumps this generation, so old scheduled resumes become no-ops.
static PAUSE_GEN: AtomicU64 = AtomicU64::new(1);

pub const PAUSE_HELP_MESSAGE: &str = r#"Usage:
  stasis pause
  stasis pause for <duration>
  stasis pause until <time>

Examples:
  stasis pause
  stasis pause for 5m
  stasis pause for 1h30m
  stasis pause for 250ms
  stasis pause until 1:30pm
  stasis pause until 13:30

Duration format:
  - a sequence of <number><unit> parts, like: 1h30m, 5m, 10s, 250ms
  - units: ms, s, m, h, d

Notes:
  - `pause` with no args pauses until you run `stasis resume`.
  - `pause for/until` schedules an automatic resume in the daemon.
"#;

pub async fn handle_pause(args: &str, tx: &mpsc::Sender<ManagerMsg>) -> String {
    let args = args.trim();

    // Match old behavior: `pause help` prints usage.
    if args.eq_ignore_ascii_case("help") || args == "-h" || args == "--help" {
        return PAUSE_HELP_MESSAGE.to_string();
    }

    // Always pause first.
    let now_ms = crate::core::utils::now_ms();
    if tx
        .send(ManagerMsg::Event(Event::ManualPause { now_ms }))
        .await
        .is_err()
    {
        return "ERROR: daemon event channel closed".to_string();
    }

    // No args => indefinite pause until manual resume.
    if args.is_empty() {
        // Invalidate any previous scheduled resumes (since user explicitly paused again).
        PAUSE_GEN.fetch_add(1, Ordering::SeqCst);
        return "Idle timers paused".to_string();
    }

    // Parse: "for ..." | "until ..."
    let parts: Vec<&str> = args.split_whitespace().collect();
    let (mode, rest) = match parts.as_slice() {
        ["for", rest @ ..] if !rest.is_empty() => ("for", rest.join(" ")),
        ["until", rest @ ..] if !rest.is_empty() => ("until", rest.join(" ")),
        _ => {
            return format!("ERROR: invalid pause syntax\n\n{}", PAUSE_HELP_MESSAGE);
        }
    };

    // Compute delay for auto-resume.
    let delay = match mode {
        "for" => match parse_duration(rest.trim()) {
            Ok(d) => d,
            Err(e) => return format!("ERROR: {e}\n\n{}", PAUSE_HELP_MESSAGE),
        },
        "until" => match parse_until_local_time(rest.trim()) {
            Ok(d) => d,
            Err(e) => return format!("ERROR: {e}\n\n{}", PAUSE_HELP_MESSAGE),
        },
        _ => unreachable!(),
    };

    // Human-facing message for the notification when the pause expires.
    // (Manager will emit this if notify_on_unpause=true)
    let notify_message: String = match mode {
        "for" => format!("Resume idle manager after {} pause", rest.trim()),
        "until" => format!(
            "Resume idle manager: pause-until time reached ({})",
            rest.trim()
        ),
        _ => "Resume idle manager".to_string(),
    };

    // If delay is zero-ish, resume immediately (still goes through daemon state machine).
    // Also: bump generation so only this scheduled resume is valid.
    let my_gen = PAUSE_GEN.fetch_add(1, Ordering::SeqCst) + 1;

    {
        let tx2 = tx.clone();
        let msg2 = notify_message.clone();
        tokio::spawn(async move {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }

            // Only resume if we're still the latest scheduled pause/resume intent.
            if PAUSE_GEN.load(Ordering::SeqCst) != my_gen {
                return;
            }

            let now_ms = crate::core::utils::now_ms();
            let _ = tx2
                .send(ManagerMsg::Event(Event::PauseExpired {
                    now_ms,
                    message: msg2,
                }))
                .await;
        });
    }

    match mode {
        "for" => format!("Idle timers paused for {}", rest.trim()),
        "until" => format!("Idle timers paused until {}", rest.trim()),
        _ => "Idle timers paused".to_string(),
    }
}

pub async fn handle_resume(tx: &mpsc::Sender<ManagerMsg>) -> String {
    // Invalidate any pending scheduled resumes.
    PAUSE_GEN.fetch_add(1, Ordering::SeqCst);

    let now_ms = crate::core::utils::now_ms();
    if tx
        .send(ManagerMsg::Event(Event::ManualResume { now_ms }))
        .await
        .is_err()
    {
        return "ERROR: daemon event channel closed".to_string();
    }
    "Idle timers resumed".to_string()
}

// ---------------- parsing ----------------

fn parse_duration(s: &str) -> Result<Duration, String> {
    // Accept "1h30m", "5m", "10s", "250ms", "1d2h3m4s"
    let s = s.trim();
    if s.is_empty() {
        return Err("missing duration after 'for'".into());
    }

    let mut i = 0usize;
    let bytes = s.as_bytes();
    let mut total_ms: u128 = 0;

    while i < bytes.len() {
        // skip whitespace
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        // parse number
        let start_num = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if start_num == i {
            return Err(format!("Duration format: expected number at '{}'", &s[i..]));
        }
        let n: u128 = s[start_num..i]
            .parse()
            .map_err(|_| "Duration format: invalid number".to_string())?;

        // parse unit (ms|s|m|h|d)
        let start_unit = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        if start_unit == i {
            return Err("Duration format: missing unit (ms/s/m/h/d)".into());
        }
        let unit = &s[start_unit..i].to_ascii_lowercase();

        let add_ms: u128 = match unit.as_str() {
            "ms" => n,
            "s" => n * 1000,
            "m" => n * 60 * 1000,
            "h" => n * 60 * 60 * 1000,
            "d" => n * 24 * 60 * 60 * 1000,
            _ => return Err(format!("Duration format: unknown unit '{unit}' (ms/s/m/h/d)")),
        };

        total_ms = total_ms
            .checked_add(add_ms)
            .ok_or_else(|| "Duration too large".to_string())?;
    }

    Ok(Duration::from_millis(
        u64::try_from(total_ms).map_err(|_| "Duration too large".to_string())?,
    ))
}

fn parse_until_local_time(s: &str) -> Result<Duration, String> {
    // Accept:
    //  - "13:30"
    //  - "1:30pm" / "1:30 pm"
    //  - "1pm" / "1 pm"
    let raw = s.trim();
    if raw.is_empty() {
        return Err("missing time after 'until'".into());
    }

    let mut t = raw.to_ascii_lowercase();
    t.retain(|c| !c.is_whitespace());

    let (is_pm, is_am, t) = if let Some(x) = t.strip_suffix("pm") {
        (true, false, x.to_string())
    } else if let Some(x) = t.strip_suffix("am") {
        (false, true, x.to_string())
    } else {
        (false, false, t)
    };

    let (hour, min) = if let Some((hh, mm)) = t.split_once(':') {
        let h: i32 = hh.parse().map_err(|_| "Invalid time (hour)".to_string())?;
        let m: i32 = mm.parse().map_err(|_| "Invalid time (minute)".to_string())?;
        (h, m)
    } else {
        // "1pm" style
        let h: i32 = t.parse().map_err(|_| "Invalid time".to_string())?;
        (h, 0)
    };

    if min < 0 || min > 59 {
        return Err("Invalid time: minute must be 0..59".into());
    }

    let mut hour = hour;

    if is_am || is_pm {
        // 12-hour clock
        if hour < 1 || hour > 12 {
            return Err("Invalid time: hour must be 1..12 for am/pm".into());
        }
        if is_pm && hour != 12 {
            hour += 12;
        }
        if is_am && hour == 12 {
            hour = 0;
        }
    } else {
        // 24-hour clock
        if hour < 0 || hour > 23 {
            return Err("Invalid time: hour must be 0..23".into());
        }
    }

    // Compute next occurrence of that local time using libc (no chrono dependency).
    unsafe {
        use libc::{localtime_r, mktime, time_t, tm};

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "System time before UNIX_EPOCH".to_string())?;
        let now_secs_u64 = now.as_secs();

        let now_tt: time_t = now_secs_u64 as time_t;

        let mut now_tm: tm = std::mem::zeroed();
        if localtime_r(&now_tt as *const time_t, &mut now_tm as *mut tm).is_null() {
            return Err("Failed to read local time".into());
        }

        // Build a tm for "today at target HH:MM:00"
        let mut target_tm = now_tm;
        target_tm.tm_hour = hour;
        target_tm.tm_min = min;
        target_tm.tm_sec = 0;
        target_tm.tm_isdst = -1; // let libc determine DST

        let mut target_tt = mktime(&mut target_tm as *mut tm);
        if target_tt == -1 {
            return Err("Invalid time (mktime failed)".into());
        }

        // If target <= now, schedule for tomorrow.
        if (target_tt as i64) <= (now_tt as i64) {
            let mut tomorrow_tm = now_tm;
            tomorrow_tm.tm_mday += 1;
            tomorrow_tm.tm_hour = hour;
            tomorrow_tm.tm_min = min;
            tomorrow_tm.tm_sec = 0;
            tomorrow_tm.tm_isdst = -1;

            target_tt = mktime(&mut tomorrow_tm as *mut tm);
            if target_tt == -1 {
                return Err("Invalid time (mktime failed)".into());
            }
        }

        let delta_secs = (target_tt as i64) - (now_tt as i64);
        if delta_secs <= 0 {
            return Ok(Duration::from_millis(0));
        }

        Ok(Duration::from_secs(delta_secs as u64))
    }
}
