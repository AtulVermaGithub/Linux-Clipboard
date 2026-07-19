//! Keystroke injection and active window manager
//! Works via X11 XTest/xdotool or Wayland virtual uinput devices

use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

static ACTIVE_WINDOW_ID: AtomicU32 = AtomicU32::new(0);

/// Check if running under X11 or Wayland
pub fn is_x11() -> bool {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    session.to_lowercase() != "wayland"
}

/// Returns the current mouse cursor location (x, y)
pub fn get_cursor_position() -> Option<(i32, i32)> {
    if is_x11() {
        let output = Command::new("xdotool")
            .arg("getmouselocation")
            .output()
            .ok()?;
        let out_str = String::from_utf8_lossy(&output.stdout);
        let mut x = 0;
        let mut y = 0;
        for token in out_str.split_whitespace() {
            if token.starts_with("x:") {
                x = token[2..].parse().unwrap_or(0);
            } else if token.starts_with("y:") {
                y = token[2..].parse().unwrap_or(0);
            }
        }
        Some((x, y))
    } else {
        None
    }
}

/// Query and store the current active window ID
pub fn save_focused_window() {
    if is_x11() {
        if let Ok(output) = Command::new("xdotool").arg("getactivewindow").output() {
            let out_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(id) = out_str.parse::<u32>() {
                ACTIVE_WINDOW_ID.store(id, Ordering::SeqCst);
            }
        }
    }
}

/// Restores focus to the saved active window
pub fn restore_focused_window() -> Result<(), String> {
    if is_x11() {
        let saved_id = ACTIVE_WINDOW_ID.load(Ordering::SeqCst);
        if saved_id != 0 {
            let status = Command::new("xdotool")
                .args(["windowactivate", &saved_id.to_string()])
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("Failed to focus window via xdotool".to_string());
            }
        }
    }
    Ok(())
}

/// Simulates Ctrl+V to paste content into the active application
pub fn simulate_paste_keystroke() -> Result<(), String> {
    // Wait a brief moment for GNOME Wayland/X11 to restore focus to target text field
    thread::sleep(Duration::from_millis(100));

    if is_x11() {
        let status = Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+v"])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err("xdotool command failed".to_string());
    } else {
        simulate_paste_uinput()?;
    }

    Ok(())
}

/// Placeholder to satisfy compiler - Wayland simulator uses dynamic setup
pub fn init_simulator() -> Result<(), String> {
    Ok(())
}

/// Simulate Ctrl+V using uinput (matches target input_simulator.rs exactly)
fn simulate_paste_uinput() -> Result<(), String> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::io::AsRawFd;

    const EV_SYN: u16 = 0x00;
    const EV_KEY: u16 = 0x01;
    const SYN_REPORT: u16 = 0x00;
    const KEY_LEFTCTRL: u16 = 29;
    const KEY_V: u16 = 47;

    fn make_event(type_: u16, code: u16, value: i32) -> [u8; 24] {
        let mut event = [0u8; 24];
        event[16..18].copy_from_slice(&type_.to_ne_bytes());
        event[18..20].copy_from_slice(&code.to_ne_bytes());
        event[20..24].copy_from_slice(&value.to_ne_bytes());
        event
    }

    let mut uinput = OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .map_err(|e| format!("Failed to open /dev/uinput: {}", e))?;

    const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
    const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
    const UI_DEV_SETUP: libc::c_ulong = 0x405c5503;
    const UI_DEV_CREATE: libc::c_ulong = 0x5501;
    const UI_DEV_DESTROY: libc::c_ulong = 0x5502;

    unsafe {
        if libc::ioctl(uinput.as_raw_fd(), UI_SET_EVBIT, EV_KEY as libc::c_int) < 0 {
            return Err("Failed to set EV_KEY".to_string());
        }
        if libc::ioctl(
            uinput.as_raw_fd(),
            UI_SET_KEYBIT,
            KEY_LEFTCTRL as libc::c_int,
        ) < 0
        {
            return Err("Failed to set KEY_LEFTCTRL".to_string());
        }
        if libc::ioctl(uinput.as_raw_fd(), UI_SET_KEYBIT, KEY_V as libc::c_int) < 0 {
            return Err("Failed to set KEY_V".to_string());
        }

        #[repr(C)]
        struct UinputSetup {
            id: [u16; 4],
            name: [u8; 80],
            ff_effects_max: u32,
        }

        let mut setup = UinputSetup {
            id: [0x03, 0x1234, 0x5678, 0x0001],
            name: [0; 80],
            ff_effects_max: 0,
        };
        let name = b"emoji-paste-helper";
        setup.name[..name.len()].copy_from_slice(name);

        if libc::ioctl(uinput.as_raw_fd(), UI_DEV_SETUP, &setup) < 0 {
            return Err("Failed to setup uinput device".to_string());
        }
        if libc::ioctl(uinput.as_raw_fd(), UI_DEV_CREATE) < 0 {
            return Err("Failed to create uinput device".to_string());
        }
    }

    // Wait longer for the virtual device to be recognized by the system
    thread::sleep(Duration::from_millis(50));

    // Press Ctrl
    uinput
        .write_all(&make_event(EV_KEY, KEY_LEFTCTRL, 1))
        .map_err(|e| e.to_string())?;
    uinput
        .write_all(&make_event(EV_SYN, SYN_REPORT, 0))
        .map_err(|e| e.to_string())?;
    uinput.flush().map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));

    // Press V
    uinput
        .write_all(&make_event(EV_KEY, KEY_V, 1))
        .map_err(|e| e.to_string())?;
    uinput
        .write_all(&make_event(EV_SYN, SYN_REPORT, 0))
        .map_err(|e| e.to_string())?;
    uinput.flush().map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));

    // Release V
    uinput
        .write_all(&make_event(EV_KEY, KEY_V, 0))
        .map_err(|e| e.to_string())?;
    uinput
        .write_all(&make_event(EV_SYN, SYN_REPORT, 0))
        .map_err(|e| e.to_string())?;
    uinput.flush().map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(50));

    // Release Ctrl
    uinput
        .write_all(&make_event(EV_KEY, KEY_LEFTCTRL, 0))
        .map_err(|e| e.to_string())?;
    uinput
        .write_all(&make_event(EV_SYN, SYN_REPORT, 0))
        .map_err(|e| e.to_string())?;
    uinput.flush().map_err(|e| e.to_string())?;

    // Wait for events to be processed before destroying device
    thread::sleep(Duration::from_millis(50));

    unsafe {
        libc::ioctl(uinput.as_raw_fd(), UI_DEV_DESTROY);
    }

    Ok(())
}
