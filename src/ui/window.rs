//! Window management and positioning logic using winit integrations

use slint::ComponentHandle;
use crate::backend::simulator::{get_cursor_position, is_x11};
use i_slint_backend_winit::winit::dpi::PhysicalPosition;
use i_slint_backend_winit::winit::window::WindowLevel;
use i_slint_backend_winit::WinitWindowAccessor;

/// Positions the Slint application window near the mouse cursor, clamped to monitor bounds
pub fn position_window<T: ComponentHandle + 'static>(app: &T) {
    let window = app.window();
    let cursor_pos = get_cursor_position();
    
    window.with_winit_window(move |winit_win| {
        let monitor = winit_win.current_monitor().or_else(|| winit_win.primary_monitor());
        
        let (m_x, m_y, m_w, m_h) = if let Some(m) = monitor {
            let pos = m.position();
            let size = m.size();
            (pos.x, pos.y, size.width as i32, size.height as i32)
        } else {
            (0, 0, 1920, 1080) // fallback standard resolution
        };

        // Window dimensions
        let win_w = 360;
        let win_h = 480;

        let (target_x, target_y) = if let Some((cx, cy)) = cursor_pos {
            // Position near cursor: offset slightly so cursor sits near top-left of window
            // but clamp to make sure it stays inside this monitor
            let x = (cx - 20).clamp(m_x + 10, m_x + m_w - win_w - 10);
            let y = (cy - 20).clamp(m_y + 10, m_y + m_h - win_h - 10);
            (x, y)
        } else {
            // Fallback: center in monitor
            let x = m_x + (m_w - win_w) / 2;
            let y = m_y + (m_h - win_h) / 2;
            (x, y)
        };

        winit_win.set_outer_position(PhysicalPosition::new(target_x, target_y));
        
        // Linux specific tweaks:
        // For X11, borderless windows work best when kept on top or manually activated
        if is_x11() {
            winit_win.set_window_level(WindowLevel::AlwaysOnTop);
            
            // Try to skip taskbar/dock to prevent dock shaking
            use i_slint_backend_winit::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = winit_win.window_handle() {
                let xid_u32 = match handle.as_raw() {
                    RawWindowHandle::Xlib(xlib_handle) => Some(xlib_handle.window as u32),
                    RawWindowHandle::Xcb(xcb_handle) => Some(xcb_handle.window.get()),
                    _ => None,
                };
                if let Some(xid) = xid_u32 {
                    use x11rb::protocol::xproto::ConnectionExt;
                    if let Ok((conn, _)) = x11rb::connect(None) {
                        // 1. Set WM_CLASS to match the desktop file and prevent "Unknown" in dock
                        let class_data = b"linux-clipboard\0linux-clipboard\0";
                        let _ = conn.change_property(
                            x11rb::protocol::xproto::PropMode::REPLACE,
                            xid,
                            x11rb::protocol::xproto::AtomEnum::WM_CLASS,
                            x11rb::protocol::xproto::AtomEnum::STRING,
                            8,
                            class_data.len() as u32,
                            class_data,
                        );

                        // 2. Set _NET_WM_STATE_SKIP_TASKBAR
                        if let Ok(reply_state) = conn.intern_atom(false, b"_NET_WM_STATE") {
                            if let Ok(reply_skip) = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR") {
                                if let (Ok(r_state), Ok(r_skip)) = (reply_state.reply(), reply_skip.reply()) {
                                    let net_wm_state = r_state.atom;
                                    let net_wm_state_skip_taskbar = r_skip.atom;
                                    let _ = conn.change_property(
                                        x11rb::protocol::xproto::PropMode::APPEND,
                                        xid,
                                        net_wm_state,
                                        x11rb::protocol::xproto::AtomEnum::ATOM,
                                        32,
                                        1,
                                        &net_wm_state_skip_taskbar.to_ne_bytes(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

/// Setup window focus loss listener to automatically hide when clicking elsewhere.
/// Returns a slint::Timer that must be kept alive by the caller.
pub fn setup_focus_loss_listener(app: &crate::AppWindow) -> slint::Timer {
    let timer = slint::Timer::default();
    let weak_app = app.as_weak();
    
    // We track whether the window has received focus since it was made visible.
    // We also track ticks to give the window manager time to map the window and settle focus.
    let mut has_had_focus = false;
    let mut visible_ticks = 0;
    
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            if let Some(app) = weak_app.upgrade() {
                if app.window().is_visible() {
                    visible_ticks += 1;
                    let is_focused = app.window().with_winit_window(|winit_win| {
                        winit_win.has_focus()
                    }).unwrap_or(false);
                    
                    if is_focused || visible_ticks > 5 {
                        has_had_focus = true;
                    }
                    
                    if visible_ticks > 5 && has_had_focus && !is_focused {
                        // Only hide if focus is lost after a 500ms startup settling period.
                        let _ = app.window().hide();
                        app.invoke_reset_state();
                        has_had_focus = false;
                        visible_ticks = 0;
                    }
                } else {
                    has_had_focus = false;
                    visible_ticks = 0;
                }
            }
        },
    );
    
    timer
}
