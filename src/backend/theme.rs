//! System theme detection utilities for Linux desktop environments.
//! Supports DBus Desktop Portals, GNOME gsettings, and KDE globals configuration files.

/// Detects if the current system is using dark mode.
/// Queries standard DBus settings portal, GNOME shell preferences, and KDE globals settings.
pub fn is_system_dark_mode() -> bool {
    // 1. Try DBus Desktop portal (standard on modern Linux)
    if let Ok(output) = std::process::Command::new("dbus-send")
        .args(&[
            "--print-reply",
            "--dest=org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings.Read",
            "string:org.freedesktop.appearance",
            "string:color-scheme",
        ])
        .output()
    {
        if output.status.success() {
            let res = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if res.contains("uint32 1") {
                return true;
            } else if res.contains("uint32 2") || res.contains("uint32 0") {
                return false;
            }
        }
    }

    // 2. Try busctl call org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop org.freedesktop.portal.Settings Read ss org.freedesktop.appearance color-scheme
    if let Ok(output) = std::process::Command::new("busctl")
        .args(&[
            "call",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "Read",
            "ss",
            "org.freedesktop.appearance",
            "color-scheme",
        ])
        .output()
    {
        if output.status.success() {
            let res = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if res.contains(" 1") {
                return true;
            } else if res.contains(" 2") || res.contains(" 0") {
                return false;
            }
        }
    }

    // 3. Try running `gsettings get org.gnome.desktop.interface color-scheme`
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(&["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if output.status.success() {
            let res = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if res.contains("dark") {
                return true;
            } else if res.contains("light") {
                return false;
            }
        }
    }

    // 4. Try checking org.gnome.desktop.interface gtk-theme
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(&["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        if output.status.success() {
            let res = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if res.contains("dark") {
                return true;
            } else if res.contains("light") {
                return false;
            }
        }
    }

    // 5. Fallback: check KDE configuration file: ~/.config/kdeglobals
    if let Ok(home) = std::env::var("HOME") {
        let kde_globals = std::path::Path::new(&home).join(".config/kdeglobals");
        if kde_globals.exists() {
            if let Ok(content) = std::fs::read_to_string(kde_globals) {
                for line in content.lines() {
                    if line.to_lowercase().contains("colorscheme=") {
                        if line.to_lowercase().contains("dark") {
                            return true;
                        } else if line.to_lowercase().contains("light") {
                            return false;
                        }
                    }
                }
            }
        }
    }

    false // Default fallback is light mode
}
