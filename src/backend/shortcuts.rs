//! Desktop Environment shortcut configuration manager
//! Registers global keybindings using native utilities (gsettings, kwriteconfig, xfconf-query).

use std::process::Command;

/// Standard shortcut configs
pub struct DesktopShortcut {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub gnome_binding: &'static str,
    pub kde_shortcut_key: &'static str,
    pub xfce_property: &'static str,
}

const SHORTCUTS: &[DesktopShortcut] = &[
    DesktopShortcut {
        id: "lincb-ople-in-toggle",
        name: "Toggle Clipboard History",
        command: "lincb.ople.in",
        gnome_binding: "<Alt>v",
        kde_shortcut_key: "Alt+V",
        xfce_property: "/commands/custom/<Alt>v",
    },
    DesktopShortcut {
        id: "lincb-ople-in-emoji",
        name: "Open Emoji Picker",
        command: "lincb.ople.in --emoji",
        gnome_binding: "<Alt>period",
        kde_shortcut_key: "Alt+.",
        xfce_property: "/commands/custom/<Alt>period",
    },
    DesktopShortcut {
        id: "lincb-ople-in-ocr",
        name: "Extract Screen Text (OCR)",
        command: "lincb.ople.in --ocr",
        gnome_binding: "<Alt><Shift>t",
        kde_shortcut_key: "Alt+Shift+T",
        xfce_property: "/commands/custom/<Alt><Shift>t",
    },
];

/// Helper to detect if a shell command is available
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Detects the active desktop environment based on env variables
pub fn detect_desktop_environment() -> String {
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        let l = desktop.to_lowercase();
        if l.contains("gnome") || l.contains("ubuntu") || l.contains("unity") || l.contains("budgie") {
            return "gnome".to_string();
        }
        if l.contains("kde") || l.contains("plasma") {
            return "kde".to_string();
        }
        if l.contains("xfce") {
            return "xfce".to_string();
        }
    }
    
    // Fallbacks
    if command_exists("gsettings") {
        "gnome".to_string()
    } else if command_exists("kwriteconfig6") || command_exists("kwriteconfig5") {
        "kde".to_string()
    } else if command_exists("xfconf-query") {
        "xfce".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Check if Super+V is already registered with another action
pub fn check_shortcut_conflict() -> Result<Option<String>, String> {
    let de = detect_desktop_environment();
    if de == "gnome" {
        if !command_exists("gsettings") {
            return Ok(None);
        }

        // 1. Check custom keybindings list
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings"])
            .output()
            .map_err(|e| e.to_string())?;
        
        let list_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if list_str.starts_with('[') && list_str.ends_with(']') {
            let custom_list: Vec<String> = list_str[1..list_str.len() - 1]
                .split(',')
                .map(|s| s.trim().trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect();

            for path in custom_list {
                if path.contains("lincb.ople.in") {
                    continue;
                }

                // Query binding
                let base_path = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
                let b_output = Command::new("gsettings")
                    .args(["get", &format!("{}:{}", base_path, path), "binding"])
                    .output();
                
                if let Ok(out) = b_output {
                    let binding = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if binding == "'<Super>v'" || binding == "'<Super>period'" {
                        // Found a conflict! Retrieve the shortcut name
                        let n_output = Command::new("gsettings")
                            .args(["get", &format!("{}:{}", base_path, path), "name"])
                            .output()
                            .ok();
                        let name = n_output
                            .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_matches('\'').to_string())
                            .unwrap_or_else(|| "Unknown Shortcut".to_string());
                        
                        return Ok(Some(format!("'{}' ({})", name, binding)));
                    }
                }
            }
        }

        // 2. Check system message tray toggle (default Super+V in older/newer GNOME)
        let sys_output = Command::new("gsettings")
            .args(["get", "org.gnome.shell.keybindings", "toggle-message-tray"])
            .output();
        if let Ok(out) = sys_output {
            let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if val.contains("<Super>v") && !val.contains("<Shift>") {
                return Ok(Some("GNOME Notification Message Tray (<Super>v)".to_string()));
            }
        }
    } else if de == "xfce" {
        if !command_exists("xfconf-query") {
            return Ok(None);
        }
        let output = Command::new("xfconf-query")
            .args(["--channel", "xfce4-keyboard-shortcuts", "--property", "/commands/custom/<Super>v"])
            .output();
        if let Ok(out) = output {
            let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !val.is_empty() && !val.contains("lincb.ople.in") && !val.contains("Failed to query") {
                return Ok(Some(format!("XFCE Custom Shortcut: {}", val)));
            }
        }
    }
    
    Ok(None)
}

/// Resolves conflicting keybindings and registers linux-clipboard global hotkeys
pub fn fix_shortcut_conflict() -> Result<(), String> {
    let de = detect_desktop_environment();
    if de == "gnome" {
        if !command_exists("gsettings") {
            return Err("gsettings tool not found".to_string());
        }

        // 1. Clear custom conflicting shortcuts
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings"])
            .output()
            .map_err(|e| e.to_string())?;
        
        let list_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if list_str.starts_with('[') && list_str.ends_with(']') {
            let mut custom_list: Vec<String> = list_str[1..list_str.len() - 1]
                .split(',')
                .map(|s| s.trim().trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let mut paths_to_remove = Vec::new();

            for path in &custom_list {
                if path.contains("lincb.ople.in") {
                    continue;
                }

                let base_path = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
                let b_output = Command::new("gsettings")
                    .args(["get", &format!("{}:{}", base_path, path), "binding"])
                    .output();
                
                if let Ok(out) = b_output {
                    let binding = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if binding == "'<Super>v'" || binding == "'<Super>period'" {
                        // Clear binding
                        Command::new("gsettings")
                            .args(["set", &format!("{}:{}", base_path, path), "binding", "''"])
                            .status().ok();
                        paths_to_remove.push(path.clone());
                    }
                }
            }

            custom_list.retain(|p| !paths_to_remove.contains(p));
            let list_formatted = format!("[{}]", custom_list.iter().map(|s| format!("'{}'", s)).collect::<Vec<String>>().join(", "));
            Command::new("gsettings")
                .args(["set", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings", &list_formatted])
                .status().ok();
        }

        // 2. Clear system message tray conflicts (<Super>v)
        let sys_output = Command::new("gsettings")
            .args(["get", "org.gnome.shell.keybindings", "toggle-message-tray"])
            .output();
        if let Ok(out) = sys_output {
            let val = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if val.contains("<Super>v") && !val.contains("<Shift>") {
                Command::new("gsettings")
                    .args(["set", "org.gnome.shell.keybindings", "toggle-message-tray", "['<Super><Shift>v']"])
                    .status().ok();
            }
        }

        // 3. Clear IBus and GTK Emoji chooser hotkeys so Super+. never types underlined 'e'
        Command::new("gsettings")
            .args(["set", "org.freedesktop.ibus.panel.emoji", "hotkey", "@as []"])
            .status().ok();
        Command::new("gsettings")
            .args(["set", "org.gtk.Settings.EmojiChooser", "trigger-combo", "''"])
            .status().ok();
        Command::new("gsettings")
            .args(["set", "org.gtk.v4.Settings.EmojiChooser", "trigger-combo", "''"])
            .status().ok();
    } else if de == "xfce" {
        if command_exists("xfconf-query") {
            Command::new("xfconf-query")
                .args(["--channel", "xfce4-keyboard-shortcuts", "--property", "/commands/custom/<Super>v", "--reset"])
                .status().ok();
        }
    }

    // Now run clean registrations
    register_shortcuts()
}

/// Register desktop environment shortcuts
pub fn register_shortcuts() -> Result<(), String> {
    let de = detect_desktop_environment();
    match de.as_str() {
        "gnome" => register_gnome()?,
        "kde" => register_kde()?,
        "xfce" => register_xfce()?,
        _ => {
            return Err("Unsupported desktop environment. Map manually to 'lincb.ople.in'.".to_string());
        }
    }
    Ok(())
}

/// Unregister desktop environment shortcuts
#[allow(dead_code)]
pub fn unregister_shortcuts() -> Result<(), String> {
    let de = detect_desktop_environment();
    match de.as_str() {
        "gnome" => unregister_gnome()?,
        "kde" => unregister_kde()?,
        "xfce" => unregister_xfce()?,
        _ => {}
    }
    Ok(())
}

// --- GNOME gsettings shortcut configuration ---
fn register_gnome() -> Result<(), String> {
    if !command_exists("gsettings") {
        return Err("gsettings tool not found".to_string());
    }

    let base_path = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
    let keybindings_list_schema = "org.gnome.settings-daemon.plugins.media-keys";

    let output = Command::new("gsettings")
        .args(["get", keybindings_list_schema, "custom-keybindings"])
        .output()
        .map_err(|e| e.to_string())?;
    
    let list_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut custom_list: Vec<String> = if list_str.starts_with('[') && list_str.ends_with(']') {
        list_str[1..list_str.len() - 1]
            .split(',')
            .map(|s| s.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    for sc in SHORTCUTS {
        let binding_path = format!("/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/{}/", sc.id);
        
        Command::new("gsettings")
            .args(["set", &format!("{}:{}", base_path, binding_path), "name", sc.name])
            .status().ok();
        Command::new("gsettings")
            .args(["set", &format!("{}:{}", base_path, binding_path), "command", sc.command])
            .status().ok();
        Command::new("gsettings")
            .args(["set", &format!("{}:{}", base_path, binding_path), "binding", sc.gnome_binding])
            .status().ok();

        if !custom_list.contains(&binding_path) {
            custom_list.push(binding_path);
        }
    }

    let list_formatted = format!("[{}]", custom_list.iter().map(|s| format!("'{}'", s)).collect::<Vec<String>>().join(", "));
    Command::new("gsettings")
        .args(["set", keybindings_list_schema, "custom-keybindings", &list_formatted])
        .status()
        .map_err(|e| format!("Failed to update custom-keybindings list: {}", e))?;

    Ok(())
}

#[allow(dead_code)]
fn unregister_gnome() -> Result<(), String> {
    if !command_exists("gsettings") {
        return Ok(());
    }

    let keybindings_list_schema = "org.gnome.settings-daemon.plugins.media-keys";
    
    let output = Command::new("gsettings")
        .args(["get", keybindings_list_schema, "custom-keybindings"])
        .output()
        .map_err(|e| e.to_string())?;
    
    let list_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if list_str.starts_with('[') && list_str.ends_with(']') {
        let mut custom_list: Vec<String> = list_str[1..list_str.len() - 1]
            .split(',')
            .map(|s| s.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect();

        for sc in SHORTCUTS {
            let binding_path = format!("/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/{}/", sc.id);
            custom_list.retain(|p| p != &binding_path);
            
            let base_path = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
            Command::new("gsettings")
                .args(["reset-recursively", &format!("{}:{}", base_path, binding_path)])
                .status().ok();
        }

        let list_formatted = format!("[{}]", custom_list.iter().map(|s| format!("'{}'", s)).collect::<Vec<String>>().join(", "));
        Command::new("gsettings")
            .args(["set", keybindings_list_schema, "custom-keybindings", &list_formatted])
            .status().ok();
    }

    Ok(())
}

// --- KDE Plasma shortcut configuration ---
fn register_kde() -> Result<(), String> {
    let kwc = if command_exists("kwriteconfig6") {
        "kwriteconfig6"
    } else if command_exists("kwriteconfig5") {
        "kwriteconfig5"
    } else {
        return Err("KDE config utility (kwriteconfig) not found".to_string());
    };

    for sc in SHORTCUTS {
        Command::new(kwc)
            .args(["--file", "kglobalshortcutsrc", "--group", "lincb.ople.in", "--key", sc.id, sc.command])
            .status().ok();
            
        Command::new(kwc)
            .args(["--file", "kglobalshortcutsrc", "--group", "lincb.ople.in", "--key", &format!("{}_key", sc.id), sc.kde_shortcut_key])
            .status().ok();
    }

    Command::new("qdbus")
        .args(["org.kde.kglobalaccel", "/kglobalaccel", "org.kde.KGlobalAccel.reconfigure"])
        .status().ok();

    Ok(())
}

#[allow(dead_code)]
fn unregister_kde() -> Result<(), String> {
    let kwc = if command_exists("kwriteconfig6") {
        "kwriteconfig6"
    } else if command_exists("kwriteconfig5") {
        "kwriteconfig5"
    } else {
        return Ok(());
    };

    for sc in SHORTCUTS {
        Command::new(kwc)
            .args(["--file", "kglobalshortcutsrc", "--group", "lincb.ople.in", "--key", sc.id, ""])
            .status().ok();
    }

    Command::new("qdbus")
        .args(["org.kde.kglobalaccel", "/kglobalaccel", "org.kde.KGlobalAccel.reconfigure"])
        .status().ok();

    Ok(())
}

// --- XFCE shortcut configuration ---
fn register_xfce() -> Result<(), String> {
    if !command_exists("xfconf-query") {
        return Err("xfconf-query utility not found".to_string());
    }

    for sc in SHORTCUTS {
        Command::new("xfconf-query")
            .args(["--channel", "xfce4-keyboard-shortcuts", "--property", sc.xfce_property, "--create", "--type", "string", "--set", sc.command])
            .status().ok();
    }

    Ok(())
}

#[allow(dead_code)]
fn unregister_xfce() -> Result<(), String> {
    if !command_exists("xfconf-query") {
        return Ok(());
    }

    for sc in SHORTCUTS {
        Command::new("xfconf-query")
            .args(["--channel", "xfce4-keyboard-shortcuts", "--property", sc.xfce_property, "--reset"])
            .status().ok();
    }

    Ok(())
}

/// Fixes/registers a single specific shortcut instantly ("toggle", "emoji", or "ocr")
pub fn fix_single_shortcut(shortcut_type: &str) -> Result<(), String> {
    let target_sc = match shortcut_type {
        "toggle" => &SHORTCUTS[0],
        "emoji" => &SHORTCUTS[1],
        "ocr" => &SHORTCUTS[2],
        _ => return Err("Unknown shortcut type".to_string()),
    };

    let de = detect_desktop_environment();
    match de.as_str() {
        "gnome" => fix_single_gnome(target_sc)?,
        "kde" => fix_single_kde(target_sc)?,
        "xfce" => fix_single_xfce(target_sc)?,
        _ => {
            return Err("Unsupported DE".to_string());
        }
    }
    Ok(())
}

fn fix_single_gnome(sc: &DesktopShortcut) -> Result<(), String> {
    if !command_exists("gsettings") {
        return Err("gsettings tool not found".to_string());
    }

    let base_path = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
    let keybindings_list_schema = "org.gnome.settings-daemon.plugins.media-keys";

    // 1. Clear system toggle-message-tray if fixing Super+V
    if sc.gnome_binding == "<Super>v" {
        Command::new("gsettings")
            .args(["set", "org.gnome.shell.keybindings", "toggle-message-tray", "['<Super><Shift>v']"])
            .status().ok();
    }
    if sc.gnome_binding == "<Super>period" {
        Command::new("gsettings")
            .args(["set", "org.freedesktop.ibus.panel.emoji", "hotkey", "@as []"])
            .status().ok();
        Command::new("gsettings")
            .args(["set", "org.gtk.Settings.EmojiChooser", "trigger-combo", "''"])
            .status().ok();
        Command::new("gsettings")
            .args(["set", "org.gtk.v4.Settings.EmojiChooser", "trigger-combo", "''"])
            .status().ok();
    }

    // 2. Read custom-keybindings list
    let output = Command::new("gsettings")
        .args(["get", keybindings_list_schema, "custom-keybindings"])
        .output()
        .map_err(|e| e.to_string())?;

    let list_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut custom_list: Vec<String> = if list_str.starts_with('[') && list_str.ends_with(']') {
        list_str[1..list_str.len() - 1]
            .split(',')
            .map(|s| s.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    let binding_path = format!("/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/{}/", sc.id);

    Command::new("gsettings")
        .args(["set", &format!("{}:{}", base_path, binding_path), "name", sc.name])
        .status().ok();
    Command::new("gsettings")
        .args(["set", &format!("{}:{}", base_path, binding_path), "command", sc.command])
        .status().ok();
    Command::new("gsettings")
        .args(["set", &format!("{}:{}", base_path, binding_path), "binding", sc.gnome_binding])
        .status().ok();

    if !custom_list.contains(&binding_path) {
        custom_list.push(binding_path);
    }

    let list_formatted = format!("[{}]", custom_list.iter().map(|s| format!("'{}'", s)).collect::<Vec<String>>().join(", "));
    Command::new("gsettings")
        .args(["set", keybindings_list_schema, "custom-keybindings", &list_formatted])
        .status()
        .map_err(|e| format!("Failed to update gsettings custom-keybindings: {}", e))?;

    Ok(())
}

fn fix_single_kde(sc: &DesktopShortcut) -> Result<(), String> {
    let kwc = if command_exists("kwriteconfig6") {
        "kwriteconfig6"
    } else if command_exists("kwriteconfig5") {
        "kwriteconfig5"
    } else {
        return Err("kwriteconfig utility not found".to_string());
    };

    Command::new(kwc)
        .args(["--file", "kglobalshortcutsrc", "--group", "lincb.ople.in", "--key", sc.id, sc.command])
        .status().ok();
    Command::new(kwc)
        .args(["--file", "kglobalshortcutsrc", "--group", "lincb.ople.in", "--key", &format!("{}_key", sc.id), sc.kde_shortcut_key])
        .status().ok();

    Command::new("qdbus")
        .args(["org.kde.kglobalaccel", "/kglobalaccel", "org.kde.KGlobalAccel.reconfigure"])
        .status().ok();

    Ok(())
}

fn fix_single_xfce(sc: &DesktopShortcut) -> Result<(), String> {
    if !command_exists("xfconf-query") {
        return Err("xfconf-query utility not found".to_string());
    }

    Command::new("xfconf-query")
        .args(["--channel", "xfce4-keyboard-shortcuts", "--property", sc.xfce_property, "--create", "--type", "string", "--set", sc.command])
        .status().ok();

    Ok(())
}
