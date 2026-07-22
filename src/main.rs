//! Main entry point for the linux-clipboard application
//! Sets up the Tokio runtime, handles single-instance check, initializes SQLite,
//! starts the clipboard watcher, and runs the Slint GUI.

slint::include_modules!();

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;
use rusqlite::Connection;
use slint::{ComponentHandle, ModelRc, VecModel};
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use base64::Engine;

mod config;
mod backend;
mod ui;

use backend::db::{ClipboardItem, ClipboardContent};

const APP_NAME: &str = "linux-clipboard";

// Emojis are populated dynamically using the `emojis` crate

/// Helper to resolve configurations directory
fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}

fn is_system_dark_mode() -> bool {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let config_dir = get_config_dir();
    let sock_path = config_dir.join("ipc.sock");

    // Single-instance check
    if sock_path.exists() {
        if let Ok(mut stream) = UnixStream::connect(&sock_path).await {
            // Another instance is running! Write command line and exit.
            let cmd = if args.contains(&"--emoji".to_string()) {
                "emoji"
            } else if args.contains(&"--settings".to_string()) {
                "settings"
            } else {
                "toggle"
            };
            let _ = stream.write_all(cmd.as_bytes()).await;
            return Ok(());
        } else {
            // Stale socket, remove it
            let _ = fs::remove_file(&sock_path);
        }
    }

    // Initialize GTK for the system tray
    gtk::init().ok();

    // Suppress deprecated libayatana-appindicator warning
    gtk::glib::log_set_default_handler(|domain, level, message| {
        if message.contains("libayatana-appindicator is deprecated") {
            return;
        }
        let level_str = match level {
            gtk::glib::LogLevel::Error => "ERROR",
            gtk::glib::LogLevel::Critical => "CRITICAL",
            gtk::glib::LogLevel::Warning => "WARNING",
            gtk::glib::LogLevel::Message => "MESSAGE",
            gtk::glib::LogLevel::Info => "INFO",
            gtk::glib::LogLevel::Debug => "DEBUG",
        };
        eprintln!("({}:{}): {} **: {}", domain.unwrap_or("GLib"), level_str, level_str.to_lowercase(), message);
    });

    // Initialize input simulation device (Wayland uinput or X11)
    if let Err(e) = backend::simulator::init_simulator() {
        eprintln!("[Main] Simulator initialization warning: {}", e);
    }

    // Set up configs
    fs::create_dir_all(&config_dir).ok();
    let config_manager = Arc::new(config::UserSettingsManager::new());
    let settings = config_manager.load();

    // Set up database
    let db_path = config_dir.join("db.db");
    let conn = Arc::new(Mutex::new(backend::db::init_db(&db_path)?));

    // Check if first-run setup is complete
    let setup_path = config_dir.join("setup_done");
    let is_first_run = !setup_path.exists();

    // Register global shortcuts *only* if not first run
    if !is_first_run {
        if let Err(e) = backend::shortcuts::register_shortcuts() {
            eprintln!("[Main] Shortcuts warning: {}", e);
        }
    }

    // Create Slint App Window
    let app = AppWindow::new()?;
    let app_weak = app.as_weak();

    let initial_is_dark = match settings.theme_mode.as_str() {
        "dark" => true,
        "light" => false,
        _ => is_system_dark_mode(),
    };
    app.set_is_dark(initial_is_dark);
    app.set_theme_mode(settings.theme_mode.clone().into());
    app.set_show_setup(is_first_run);
    // Check if --emoji flag is present on startup
    if args.contains(&"--emoji".to_string()) {
        app.set_active_tab(1);
        app.set_search_placeholder("Search emojis...".into());
    }
    
    // Populate initial emojis
    refresh_emojis(app_weak.clone(), 0, String::new());

    // Load initial clipboard history list
    refresh_clips(app_weak.clone(), conn.clone(), String::new());

    // Setup callbacks
    setup_callbacks(&app, conn.clone(), config_manager.clone());

    // Setup focus lost hide and positioning
    let _focus_timer = ui::window::setup_focus_loss_listener(&app);
    ui::window::position_window(&app);

    // Spawn IPC socket listener in background
    let app_weak_ipc = app_weak.clone();
    let sock_path_clone = sock_path.clone();
    tokio::spawn(async move {
        if let Ok(listener) = UnixListener::bind(&sock_path_clone) {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let app_weak_clone = app_weak_ipc.clone();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 32];
                        if let Ok(n) = stream.read(&mut buf).await {
                            let cmd = String::from_utf8_lossy(&buf[..n]);
                            let cmd_str = cmd.trim().to_string();
                            
                            // Wake up UI loop
                            slint::invoke_from_event_loop(move || {
                                if let Some(app) = app_weak_clone.upgrade() {
                                     if app.window().is_visible() && cmd_str == "toggle" {
                                         let _ = app.window().hide();
                                         app.invoke_reset_state();
                                     } else {
                                        // Update active tab based on commands
                                        if cmd_str == "emoji" {
                                            app.set_active_tab(1);
                                            app.set_search_placeholder("Search emojis...".into());
                                        } else {
                                            app.set_active_tab(0);
                                            app.set_search_placeholder("Search history...".into());
                                        }
                                         app.set_selected_index(0);
                                         backend::simulator::save_focused_window();
                                         ui::window::position_window(&app);
                                         let _ = app.window().show();
                                     }
                                }
                            }).ok();
                        }
                    });
                }
            }
        }
    });

    // Start background clipboard watcher
    let app_weak_watcher = app_weak.clone();
    let conn_watcher = conn.clone();
    let config_manager_watcher = config_manager.clone();
    std::thread::spawn(move || {
        let mut clean_counter = 0;
        let mut theme_check_counter = 0;
        let mut current_applied_dark = initial_is_dark;

        loop {
            std::thread::sleep(Duration::from_millis(500));
            clean_counter += 1;
            theme_check_counter += 1;

            let settings = config_manager_watcher.load();

            // Check system theme change (~2s)
            if theme_check_counter >= 4 {
                theme_check_counter = 0;
                let target_is_dark = match settings.theme_mode.as_str() {
                    "dark" => true,
                    "light" => false,
                    _ => is_system_dark_mode(),
                };
                if target_is_dark != current_applied_dark {
                    current_applied_dark = target_is_dark;
                    let app_weak_c = app_weak_watcher.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak_c.upgrade() {
                            app.set_is_dark(target_is_dark);
                        }
                    }).ok();
                }
            }

            // Periodic database size cleanup (~30s)
            if clean_counter >= 60 {
                clean_counter = 0;
                let db = conn_watcher.lock();
                if let Ok(true) = backend::db::cleanup_old_items(
                    &db,
                    settings.max_history_size,
                    settings.auto_delete_interval_in_minutes(),
                ) {
                    let app_weak_c = app_weak_watcher.clone();
                    let conn_c = conn_watcher.clone();
                    slint::invoke_from_event_loop(move || {
                        refresh_clips(app_weak_c, conn_c, String::new());
                    }).ok();
                }
            }

            let last_text_hash_val = backend::clipboard::LAST_TEXT_HASH.load(std::sync::atomic::Ordering::SeqCst);
            let last_image_hash_val = backend::clipboard::LAST_IMAGE_HASH.load(std::sync::atomic::Ordering::SeqCst);

            // Check Clipboard Text
            if let Ok(text) = backend::clipboard::get_current_text() {
                if !text.trim().is_empty() && !text.starts_with("\u{fffd}PNG") && !text.contains('\0') {
                    let text_hash = backend::clipboard::calculate_hash(&text);
                    if text_hash != last_text_hash_val {
                        backend::clipboard::LAST_TEXT_HASH.store(text_hash, std::sync::atomic::Ordering::SeqCst);
                        backend::clipboard::LAST_IMAGE_HASH.store(0, std::sync::atomic::Ordering::SeqCst);

                        // Insert new clipboard item
                        let cleaned: String = text
                            .chars()
                            .map(|c| if c == '\r' || c == '\n' || c == '\t' { ' ' } else { c })
                            .collect();
                        
                        let mut collapsed = String::new();
                        let mut prev_was_space = false;
                        for c in cleaned.chars() {
                            if c == ' ' {
                                if !prev_was_space {
                                    collapsed.push(c);
                                    prev_was_space = true;
                                }
                            } else {
                                collapsed.push(c);
                                prev_was_space = false;
                            }
                        }
                        let collapsed_trimmed = collapsed.trim().to_string();

                        let preview = if collapsed_trimmed.chars().count() > 80 {
                            format!("{}...", collapsed_trimmed.chars().take(80).collect::<String>())
                        } else {
                            collapsed_trimmed
                        };

                        let item = ClipboardItem {
                            id: uuid::Uuid::new_v4().to_string(),
                            content: ClipboardContent::Text(text),
                            timestamp: chrono::Utc::now(),
                            pinned: false,
                            preview,
                        };

                        let db = conn_watcher.lock();
                        if backend::db::insert_item(&db, &item).is_ok() {
                            let app_weak_c = app_weak_watcher.clone();
                            let conn_c = conn_watcher.clone();
                            slint::invoke_from_event_loop(move || {
                                refresh_clips(app_weak_c, conn_c, String::new());
                            }).ok();
                        }
                    }
                }
            }

            // Check Clipboard Image
            if let Ok(Some((image_data, hash))) = backend::clipboard::get_current_image() {
                if hash != last_image_hash_val {
                    backend::clipboard::LAST_IMAGE_HASH.store(hash, std::sync::atomic::Ordering::SeqCst);
                    backend::clipboard::LAST_TEXT_HASH.store(0, std::sync::atomic::Ordering::SeqCst);

                    if let Some(b64) = backend::clipboard::convert_image_to_base64(&image_data) {
                        let item = ClipboardItem {
                            id: uuid::Uuid::new_v4().to_string(),
                            content: ClipboardContent::Image {
                                base64: b64,
                                width: image_data.width as u32,
                                height: image_data.height as u32,
                            },
                            timestamp: chrono::Utc::now(),
                            pinned: false,
                            preview: format!("Image ({}x{})", image_data.width, image_data.height),
                        };

                        let db = conn_watcher.lock();
                        if backend::db::insert_item(&db, &item).is_ok() {
                            let app_weak_c = app_weak_watcher.clone();
                            let conn_c = conn_watcher.clone();
                            slint::invoke_from_event_loop(move || {
                                refresh_clips(app_weak_c, conn_c, String::new());
                            }).ok();
                        }
                    }
                }
            }
        }
    });

    // Start System Tray Icon
    let _tray = ui::tray::setup_tray().ok();

    // Show window if not started in background
    if !args.contains(&"--background".to_string()) {
        let _ = app.window().show();
    }

    slint::run_event_loop_until_quit()?;

    // Unregister stale IPC socket file on exit
    let _ = fs::remove_file(&sock_path);
    Ok(())
}

/// Helper to decode base64 PNG into a Slint image
fn load_slint_image_from_base64(base64_str: &str) -> Option<slint::Image> {
    let png_bytes = base64::prelude::BASE64_STANDARD.decode(base64_str).ok()?;
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let bytes = &buf[..info.buffer_size()];
    
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
        bytes,
        info.width,
        info.height,
    );
    Some(slint::Image::from_rgba8(buffer))
}

/// Refreshes the Slint UI with history items from SQLite
fn refresh_clips(app_weak: slint::Weak<AppWindow>, conn: Arc<Mutex<Connection>>, search_text: String) {
    if let Some(app) = app_weak.upgrade() {
        let db = conn.lock();
        if let Ok(history) = backend::db::get_history(&db) {
            let filter = search_text.to_lowercase();
            let items: Vec<SlintClipItem> = history
                .into_iter()
                .filter(|item| {
                    if filter.is_empty() {
                        true
                    } else {
                        item.preview.to_lowercase().contains(&filter)
                    }
                })
                .map(|item| {
                    let ts_local = item.timestamp.with_timezone(&chrono::Local);
                    let ts_str = ts_local.format("%Y-%m-%d %H:%M:%S").to_string();
                    
                    let (item_type, plain_text, b64) = match item.content {
                        ClipboardContent::Text(text) => ("Text", text, String::new()),
                        ClipboardContent::RichText { plain, .. } => ("RichText", plain, String::new()),
                        ClipboardContent::Image { base64, .. } => ("Image", String::new(), base64),
                    };

                    let slint_img = if item_type == "Image" && !b64.is_empty() {
                        load_slint_image_from_base64(&b64).unwrap_or_default()
                    } else {
                        slint::Image::default()
                    };

                    SlintClipItem {
                        id: item.id.into(),
                        item_type: item_type.into(),
                        plain_text: plain_text.into(),
                        timestamp_str: ts_str.into(),
                        pinned: item.pinned,
                        preview: item.preview.into(),
                        image_base64: b64.into(),
                        image: slint_img,
                    }
                })
                .collect();
            
            app.set_clips(ModelRc::new(VecModel::from(items)));
            app.set_selected_index(0);
        }
    }
}

fn refresh_emojis(app_weak: slint::Weak<AppWindow>, category_idx: i32, search_text: String) {
    if let Some(app) = app_weak.upgrade() {
        let filter = search_text.to_lowercase();
        
        let emoji_iter = if !filter.is_empty() {
            // Search across all emojis
            emojis::iter()
                .filter(|e| {
                    e.name().to_lowercase().contains(&filter) ||
                    e.shortcode().map(|s| s.to_lowercase().contains(&filter)).unwrap_or(false)
                })
                .collect::<Vec<_>>()
        } else {
            // Map category index to emojis::Group
            let group_opt = match category_idx {
                0 => Some(emojis::Group::SmileysAndEmotion),
                1 => Some(emojis::Group::PeopleAndBody),
                2 => Some(emojis::Group::AnimalsAndNature),
                3 => Some(emojis::Group::FoodAndDrink),
                4 => Some(emojis::Group::Activities),
                5 => Some(emojis::Group::TravelAndPlaces),
                6 => Some(emojis::Group::Objects),
                7 => Some(emojis::Group::Symbols),
                8 => Some(emojis::Group::Flags),
                _ => None,
            };

            if let Some(group) = group_opt {
                group.emojis().collect::<Vec<_>>()
            } else {
                emojis::Group::SmileysAndEmotion.emojis().collect::<Vec<_>>()
            }
        };

        let mut emoji_rows: Vec<SlintEmojiRow> = Vec::new();
        let mut current_row = Vec::new();

        for emoji in emoji_iter {
            current_row.push(SlintEmojiItem {
                character: emoji.as_str().into(),
                description: emoji.name().into(),
            });
            if current_row.len() == 6 {
                emoji_rows.push(SlintEmojiRow {
                    cols: ModelRc::new(VecModel::from(current_row)),
                });
                current_row = Vec::new();
            }
        }
        if !current_row.is_empty() {
            emoji_rows.push(SlintEmojiRow {
                cols: ModelRc::new(VecModel::from(current_row)),
            });
        }
        app.set_emoji_rows(ModelRc::new(VecModel::from(emoji_rows)));
    }
}

/// Sets up the Slint component callbacks
fn setup_callbacks(
    app: &AppWindow,
    conn: Arc<Mutex<Connection>>,
    config_manager: Arc<config::UserSettingsManager>,
) {
    let app_weak = app.as_weak();
    
    // 1. Paste Item
    let conn_c = conn.clone();
    let app_weak_c = app_weak.clone();
    app.on_paste_item(move |id| {
        let content_opt = {
            let db = conn_c.lock();
            if let Ok(history) = backend::db::get_history(&db) {
                history.iter().find(|i| i.id == id.as_str()).map(|i| i.content.clone())
            } else {
                None
            }
        };

        if let Some(content) = content_opt {
            // Hide window immediately (queued in event loop)
            if let Some(app) = app_weak_c.upgrade() {
                let _ = app.window().hide();
                app.invoke_reset_state();
            }

                // Spawn background thread for focus restore + clipboard + paste
                // This lets the Slint event loop process the window hide first
                std::thread::spawn(move || {
                    // Wait for the window to actually hide and compositor to process it
                    std::thread::sleep(std::time::Duration::from_millis(150));

                    // Restore active window focus and verify it settled
                    match backend::simulator::restore_focused_window() {
                        Ok(true) => {
                            // Focus settled successfully
                        }
                        _ => {
                            // Focus restoration could not be verified in time - sleep a bit extra to be safe
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }

                    // Set clipboard robustly
                    match &content {
                        ClipboardContent::Text(text) => {
                            let _ = backend::clipboard::set_text_robust(text);
                        }
                        ClipboardContent::RichText { plain, html } => {
                            let _ = backend::clipboard::set_html_robust(html, plain);
                        }
                        ClipboardContent::Image { base64, width, height } => {
                            let _ = backend::clipboard::set_image_robust(base64, *width, *height);
                        }
                    }

                    // Wait for clipboard to settle
                    std::thread::sleep(std::time::Duration::from_millis(60));

                    // Simulate paste
                    if let Err(e) = backend::simulator::simulate_paste_keystroke() {
                        eprintln!("[Main] Paste simulation failed: {}", e);
                    }

                    // Post-paste delay to let target app read clipboard
                    std::thread::sleep(std::time::Duration::from_millis(250));
                });
        }
    });

    // 2. Delete Item
    let conn_c = conn.clone();
    let app_weak_c = app_weak.clone();
    app.on_delete_item(move |id| {
        {
            let db = conn_c.lock();
            let _ = backend::db::delete_item(&db, id.as_str());
        }
        refresh_clips(app_weak_c.clone(), conn_c.clone(), String::new());
    });

    // 3. Toggle Pin
    let conn_c = conn.clone();
    let app_weak_c = app_weak.clone();
    app.on_toggle_pin(move |id| {
        {
            let db = conn_c.lock();
            let _ = backend::db::toggle_pin(&db, id.as_str());
        }
        refresh_clips(app_weak_c.clone(), conn_c.clone(), String::new());
    });

    // 4. Clear History
    let conn_c = conn.clone();
    let app_weak_c = app_weak.clone();
    app.on_clear_history(move || {
        {
            let db = conn_c.lock();
            let _ = backend::db::clear_history(&db);
        }
        refresh_clips(app_weak_c.clone(), conn_c.clone(), String::new());
    });

    // 5. Search Changed
    let conn_c = conn.clone();
    let app_weak_c = app_weak.clone();
    app.on_search_changed(move |text| {
        if let Some(app) = app_weak_c.upgrade() {
            let active_tab = app.get_active_tab();
            if active_tab == 1 {
                let category_idx = app.get_active_emoji_category();
                refresh_emojis(app_weak_c.clone(), category_idx, text.to_string());
            } else {
                refresh_clips(app_weak_c.clone(), conn_c.clone(), text.to_string());
            }
        }
    });

    // 5b. Emoji Category Changed
    let app_weak_c = app_weak.clone();
    app.on_emoji_category_changed(move |category_idx| {
        refresh_emojis(app_weak_c.clone(), category_idx, String::new());
    });

    // 6. Record Emoji Click
    let conn_c = conn.clone();
    let app_weak_c = app_weak.clone();
    app.on_record_emoji(move |emoji| {
        {
            let db = conn_c.lock();
            let _ = backend::db::record_emoji_usage(&db, emoji.as_str());
        }
        
        // Clone emoji string for background thread
        let emoji_str = emoji.to_string();

        // Hide window immediately (queued in event loop)
        if let Some(app) = app_weak_c.upgrade() {
            let _ = app.window().hide();
            app.invoke_reset_state();
        }

        // Spawn background thread for paste
        std::thread::spawn(move || {
            // Wait for the window to actually hide
            std::thread::sleep(std::time::Duration::from_millis(150));

            // Set to clipboard
            let _ = backend::clipboard::set_text_robust(&emoji_str);

            // Restore focus and verify it settled
            match backend::simulator::restore_focused_window() {
                Ok(true) => {
                    // Focus settled successfully
                }
                _ => {
                    // Focus restoration could not be verified in time - sleep a bit extra to be safe
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }

            // Wait for clipboard to settle
            std::thread::sleep(std::time::Duration::from_millis(60));

            // Simulate paste
            if let Err(e) = backend::simulator::simulate_paste_keystroke() {
                eprintln!("[Main] Paste simulation failed: {}", e);
            }

            // Post-paste delay
            std::thread::sleep(std::time::Duration::from_millis(250));
        });
    });

    // 7. Open Settings
    app.on_open_settings(move || {
        // Dynamic opening placeholder
        eprintln!("[UI] Settings window triggered");
    });

    // 8. Close Window
    let app_weak_c = app_weak.clone();
    app.on_close_window(move || {
        if let Some(app) = app_weak_c.upgrade() {
            let _ = app.window().hide();
            app.invoke_reset_state();
        }
    });

    // 9. Finish Setup
    let app_weak_c = app_weak.clone();
    app.on_finish_setup(move || {
        let setup_path = get_config_dir().join("setup_done");
        std::fs::write(&setup_path, "done").ok();
        let _ = backend::shortcuts::register_shortcuts();
        if let Some(app) = app_weak_c.upgrade() {
            app.set_show_setup(false);
        }
    });

    // 10. Check Shortcuts
    let app_weak_check = app_weak.clone();
    app.on_check_shortcuts(move || {
        if let Some(app) = app_weak_check.upgrade() {
            app.set_setup_step(1);
            app.set_shortcut_status("checking".into());
            
            match backend::shortcuts::check_shortcut_conflict() {
                Ok(Some(details)) => {
                    app.set_shortcut_status("conflict".into());
                    app.set_shortcut_details(details.into());
                }
                Ok(None) => {
                    match backend::shortcuts::register_shortcuts() {
                        Ok(_) => {
                            app.set_shortcut_status("ok".into());
                        }
                        Err(_) => {
                            app.set_shortcut_status("failed".into());
                        }
                    }
                }
                Err(_) => {
                    app.set_shortcut_status("failed".into());
                }
            }
        }
    });

    // 11. Fix Shortcuts
    let app_weak_fix = app_weak.clone();
    app.on_fix_shortcuts(move || {
        if let Some(app) = app_weak_fix.upgrade() {
            app.set_shortcut_status("checking".into());
            match backend::shortcuts::fix_shortcut_conflict() {
                Ok(_) => {
                    app.set_shortcut_status("ok".into());
                }
                Err(_) => {
                    app.set_shortcut_status("failed".into());
                }
            }
        }
    });

    // 12. Change Theme settings
    let config_manager_theme = config_manager.clone();
    let app_weak_theme = app_weak.clone();
    app.on_change_theme(move |mode| {
        if let Some(app) = app_weak_theme.upgrade() {
            let mode_str = mode.to_string();
            
            // Update UI property state
            app.set_theme_mode(mode.clone());
            
            // Save to settings
            let mut settings = config_manager_theme.load();
            settings.theme_mode = mode_str.clone();
            let _ = config_manager_theme.save(&settings);
            
            // Re-apply theme instantly
            let is_dark = match mode_str.as_str() {
                "dark" => true,
                "light" => false,
                _ => is_system_dark_mode(),
            };
            app.set_is_dark(is_dark);
        }
    });
}
