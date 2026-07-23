//! Main entry point for the linux-clipboard application.
//! Sets up the Tokio runtime, handles single-instance check, initializes SQLite,
//! starts the clipboard watcher, and runs the Slint GUI event loop.

slint::include_modules!();

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;
use rusqlite::Connection;
use slint::ComponentHandle;

mod config;
mod backend;
mod ui;

use backend::db::{ClipboardItem, ClipboardContent};
use backend::theme::is_system_dark_mode;
use backend::ipc::{handle_single_instance, spawn_ipc_listener};
use ui::helpers::{refresh_clips, refresh_emojis};

const APP_NAME: &str = "lincb.ople.in";

/// Helper to resolve configurations directory
fn get_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let config_dir = get_config_dir();
    let sock_path = config_dir.join("ipc.sock");

    // Single-instance check
    if handle_single_instance(&sock_path, &args).await? {
        return Ok(());
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

    // Always register/update desktop environment shortcuts on startup
    if let Err(e) = backend::shortcuts::register_shortcuts() {
        eprintln!("[Main] Shortcuts registration warning: {}", e);
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
    app.set_enable_ocr(settings.enable_ocr_feature);

    let force_setup = args.contains(&"--setup".to_string());
    app.set_show_setup(is_first_run || force_setup);
    
    // Check if CLI flags are present on startup
    if args.contains(&"--emoji".to_string()) {
        app.set_active_tab(1);
        app.set_search_placeholder("Search emojis...".into());
    } else if args.contains(&"--ocr".to_string()) {
        crate::backend::ocr::run_ocr_capture_and_ingest(conn.clone(), app_weak.clone());
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
    spawn_ipc_listener(&sock_path, app_weak.clone(), conn.clone());

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

            // Check Clipboard Image (skipped if OCR capture is running)
            if !backend::ocr::IS_OCR_RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
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

    // 11b. Fix Single Shortcut instantly (toggle, emoji, ocr)
    app.on_fix_single_shortcut(move |sc_type| {
        let sc_str = sc_type.to_string();
        if let Err(e) = backend::shortcuts::fix_single_shortcut(&sc_str) {
            eprintln!("[Main] Failed to fix single shortcut {}: {}", sc_str, e);
        } else {
            eprintln!("[Main] Successfully registered single shortcut {}", sc_str);
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

    // 13. Toggle OCR setting
    let config_manager_ocr = config_manager.clone();
    let app_weak_ocr = app_weak.clone();
    app.on_toggle_ocr(move |enabled| {
        if let Some(app) = app_weak_ocr.upgrade() {
            app.set_enable_ocr(enabled);
            let mut settings = config_manager_ocr.load();
            settings.enable_ocr_feature = enabled;
            let _ = config_manager_ocr.save(&settings);
        }
    });

    // 14. Open Setup Wizard
    let app_weak_open_setup = app_weak.clone();
    app.on_open_setup(move || {
        if let Some(app) = app_weak_open_setup.upgrade() {
            app.set_setup_step(0);
            app.set_show_setup(true);
        }
    });
}
