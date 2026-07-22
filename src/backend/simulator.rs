//! Keystroke injection and active window manager
//! Works via X11 XTest/xdotool or Wayland virtual uinput devices

use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as XprotoConnectionExt, InputFocus, ClientMessageEvent, EventMask};
use x11rb::protocol::xtest::ConnectionExt as XtestConnectionExt;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

static ACTIVE_WINDOW_ID: AtomicU32 = AtomicU32::new(0);

/// Check if running under X11 or Wayland
pub fn is_x11() -> bool {
    if let Ok(val) = std::env::var("XDG_SESSION_TYPE") {
        match val.trim().to_lowercase().as_str() {
            "wayland" => return false,
            "x11" => return true,
            _ => {}
        }
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return false;
    }
    if std::env::var_os("DISPLAY").is_some() {
        return true;
    }
    true // Fallback to X11
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
    if let Ok((conn, _)) = x11rb::connect(None) {
        if let Ok(cookie) = conn.get_input_focus() {
            if let Ok(reply) = cookie.reply() {
                let window_id = reply.focus;
                ACTIVE_WINDOW_ID.store(window_id, Ordering::SeqCst);
                eprintln!("[Simulator] Saved focused window: {}", window_id);
            }
        }
    }
}

/// Activates an X11 window using EWMH client message protocol
fn x11_activate_window_by_id(window_id: u32) -> Result<(), String> {
    let (conn, screen_num) =
        x11rb::connect(None).map_err(|e| format!("X11 connect failed: {}", e))?;

    let screen = conn
        .setup()
        .roots
        .get(screen_num)
        .ok_or("Failed to get screen")?;
    let root = screen.root;

    // Get _NET_ACTIVE_WINDOW atom
    let net_active_window = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .map_err(|e| format!("Failed to intern atom: {}", e))?
        .reply()
        .map_err(|e| format!("Failed to get atom reply: {}", e))?
        .atom;

    // Create client message event
    let event = ClientMessageEvent {
        response_type: 33, // ClientMessage
        format: 32,
        sequence: 0,
        window: window_id,
        type_: net_active_window,
        data: [1, 0, 0, 0, 0].into(),
    };

    // Send event to root window
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    )
    .map_err(|e| format!("Failed to send event: {}", e))?;

    conn.flush().map_err(|e| format!("Failed to flush: {}", e))?;
    Ok(())
}

/// Restores focus to the saved active window and verifies it settled.
/// Returns Ok(true) if focus restoration was verified, Ok(false) if it could not be verified in time.
pub fn restore_focused_window() -> Result<bool, String> {
    let saved_id = ACTIVE_WINDOW_ID.load(Ordering::SeqCst);
    eprintln!("[Simulator] Restoring focus to saved window: {}", saved_id);
    if saved_id != 0 {
        // Try EWMH activation first (WM compliant)
        if let Err(e) = x11_activate_window_by_id(saved_id) {
            eprintln!("[Simulator] EWMH activation failed: {}, trying set_input_focus fallback", e);
            if let Ok((conn, _)) = x11rb::connect(None) {
                let _ = conn.set_input_focus(InputFocus::PARENT, saved_id, x11rb::CURRENT_TIME);
                let _ = conn.flush();
            }
        }

        // Wait for focus to settle (up to 150ms)
        if let Ok((conn, _)) = x11rb::connect(None) {
            let start = std::time::Instant::now();
            let budget = Duration::from_millis(150);
            let poll_interval = Duration::from_millis(3);
            let mut confirmed = false;

            while start.elapsed() < budget {
                if let Ok(cookie) = conn.get_input_focus() {
                    if let Ok(reply) = cookie.reply() {
                        if reply.focus == saved_id {
                            confirmed = true;
                            break;
                        }
                    }
                }
                thread::sleep(poll_interval);
            }

            if !confirmed {
                let remaining = budget.saturating_sub(start.elapsed());
                if !remaining.is_zero() {
                    thread::sleep(remaining);
                }
            }
            eprintln!("[Simulator] Focus restoration settled. Confirmed: {}", confirmed);
            return Ok(confirmed);
        }
    }
    Ok(false)
}

/// Simulate Ctrl+V using X11 XTest extension
fn simulate_paste_xtest() -> Result<(), String> {
    const CTRL_L_KEYCODE: u8 = 37;
    const V_KEYCODE: u8 = 55;

    let (conn, screen_num) =
        x11rb::connect(None).map_err(|e| format!("X11 connect failed: {}", e))?;
    let screen = &conn.setup().roots[screen_num];
    let root_window = screen.root;

    conn.xtest_get_version(2, 1)
        .map_err(|e| format!("XTest version query failed: {}", e))?
        .reply()
        .map_err(|e| format!("XTest version query failed: {}", e))?;

    conn.sync().map_err(|e| format!("Sync setup failed: {}", e))?;

    // Press Ctrl
    conn.xtest_fake_input(2, CTRL_L_KEYCODE, 0, root_window, 0, 0, 0)
        .map_err(|e| format!("Failed to press Ctrl: {}", e))?;
    conn.sync().map_err(|e| format!("Sync Ctrl press failed: {}", e))?;
    thread::sleep(Duration::from_millis(50));

    // Press V
    conn.xtest_fake_input(2, V_KEYCODE, 0, root_window, 0, 0, 0)
        .map_err(|e| format!("Failed to press V: {}", e))?;
    conn.sync().map_err(|e| format!("Sync V press failed: {}", e))?;
    thread::sleep(Duration::from_millis(50));

    // Release V
    conn.xtest_fake_input(3, V_KEYCODE, 0, root_window, 0, 0, 0)
        .map_err(|e| format!("Failed to release V: {}", e))?;
    conn.sync().map_err(|e| format!("Sync V release failed: {}", e))?;
    thread::sleep(Duration::from_millis(50));

    // Release Ctrl
    conn.xtest_fake_input(3, CTRL_L_KEYCODE, 0, root_window, 0, 0, 0)
        .map_err(|e| format!("Failed to release Ctrl: {}", e))?;
    conn.sync().map_err(|e| format!("Final sync failed: {}", e))?;

    Ok(())
}

/// Simulate Ctrl+V using xdotool
fn simulate_paste_xdotool() -> Result<(), String> {
    let output = Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+v"])
        .output()
        .map_err(|e| format!("Failed to run xdotool key: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("xdotool key failed: {}", stderr))
    }
}

/// Simulates Ctrl+V to paste content into the active application
pub fn simulate_paste_keystroke() -> Result<(), String> {
    eprintln!("[SimulatePaste] Sending Ctrl+V (session: {})...",
        if is_x11() { "X11" } else { "Wayland" });

    // Match Tauri reference: xdotool→XTest→uinput on X11, uinput-only on Wayland
    let strategies: &[(&str, fn() -> Result<(), String>)] = if is_x11() {
        &[
            ("xdotool", simulate_paste_xdotool as fn() -> Result<(), String>),
            ("XTest", simulate_paste_xtest as fn() -> Result<(), String>),
            ("uinput", simulate_paste_uinput as fn() -> Result<(), String>),
        ]
    } else {
        &[
            ("uinput", simulate_paste_uinput as fn() -> Result<(), String>),
        ]
    };

    for (name, func) in strategies {
        match func() {
            Ok(()) => {
                eprintln!("[SimulatePaste] Ctrl+V sent via {}", name);
                return Ok(());
            }
            Err(err) => {
                eprintln!("[SimulatePaste] {} failed: {}", name, err);
            }
        }
    }

    eprintln!("[SimulatePaste] All strategies failed!");
    Err("All paste methods failed".to_string())
}

/// Placeholder to satisfy compiler - Wayland simulator uses dynamic setup
pub fn init_simulator() -> Result<(), String> {
    Ok(())
}

/// Simulate Ctrl+V using uinput
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

    // Wait for the virtual device to be recognized by the compositor
    // Critical: too short and Ctrl keypress gets lost, resulting in 'v' being typed
    thread::sleep(Duration::from_millis(200));

    // Press Ctrl
    uinput
        .write_all(&make_event(EV_KEY, KEY_LEFTCTRL, 1))
        .map_err(|e| e.to_string())?;
    uinput
        .write_all(&make_event(EV_SYN, SYN_REPORT, 0))
        .map_err(|e| e.to_string())?;
    uinput.flush().map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(80));

    // Press V
    uinput
        .write_all(&make_event(EV_KEY, KEY_V, 1))
        .map_err(|e| e.to_string())?;
    uinput
        .write_all(&make_event(EV_SYN, SYN_REPORT, 0))
        .map_err(|e| e.to_string())?;
    uinput.flush().map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(80));

    // Release V
    uinput
        .write_all(&make_event(EV_KEY, KEY_V, 0))
        .map_err(|e| e.to_string())?;
    uinput
        .write_all(&make_event(EV_SYN, SYN_REPORT, 0))
        .map_err(|e| e.to_string())?;
    uinput.flush().map_err(|e| e.to_string())?;
    thread::sleep(Duration::from_millis(80));

    // Release Ctrl
    uinput
        .write_all(&make_event(EV_KEY, KEY_LEFTCTRL, 0))
        .map_err(|e| e.to_string())?;
    uinput
        .write_all(&make_event(EV_SYN, SYN_REPORT, 0))
        .map_err(|e| e.to_string())?;
    uinput.flush().map_err(|e| e.to_string())?;

    // Wait for events to be processed before destroying device
    thread::sleep(Duration::from_millis(80));

    unsafe {
        libc::ioctl(uinput.as_raw_fd(), UI_DEV_DESTROY);
    }

    Ok(())
}
