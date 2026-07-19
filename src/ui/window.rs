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
        }
    });
}

/// Setup window focus loss listener to automatically hide when clicking elsewhere.
/// Returns a slint::Timer that must be kept alive by the caller.
pub fn setup_focus_loss_listener<T: ComponentHandle + 'static>(app: &T) -> slint::Timer {
    let timer = slint::Timer::default();
    let weak_app = app.as_weak();
    
    // We track whether the window has received focus since it was made visible.
    // This prevents the window from hiding itself before the window manager has finished mapping it.
    let mut has_had_focus = false;
    
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            if let Some(app) = weak_app.upgrade() {
                if app.window().is_visible() {
                    let is_focused = app.window().with_winit_window(|winit_win| {
                        winit_win.has_focus()
                    }).unwrap_or(false);
                    
                    if is_focused {
                        has_had_focus = true;
                    } else if has_had_focus {
                        // Only hide if the window actually had focus and has now lost it.
                        let _ = app.window().hide();
                        has_had_focus = false; // Reset state
                    }
                } else {
                    has_had_focus = false; // Reset state when hidden
                }
            }
        },
    );
    
    timer
}
