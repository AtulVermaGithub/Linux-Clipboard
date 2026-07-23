//! OCR Text Extraction module using Tesseract C-API (leptess)
//! Accepts cropped image buffers, recognizes English text, and returns formatted String

use leptess::LepTess;
use std::path::Path;
use slint::ComponentHandle;
use std::sync::atomic::{AtomicBool, Ordering};

/// Flag indicating whether screen region OCR is actively running.
/// Used by the clipboard watcher loop to ignore temporary screenshot clips.
pub static IS_OCR_RUNNING: AtomicBool = AtomicBool::new(false);

/// Perform OCR text extraction on an image file or in-memory image buffer
pub fn extract_text_from_image_buffer(image_bytes: &[u8]) -> Result<String, String> {
    let tmp_path = std::env::temp_dir().join(format!("lincb_ocr_{}.png", uuid::Uuid::new_v4()));
    std::fs::write(&tmp_path, image_bytes)
        .map_err(|e| format!("Failed to write OCR temp image: {}", e))?;

    let result = extract_text_from_file(&tmp_path);
    let _ = std::fs::remove_file(&tmp_path);
    result
}

/// Perform OCR text extraction directly from image path
pub fn extract_text_from_file(image_path: &Path) -> Result<String, String> {
    let mut lt = LepTess::new(None, "eng")
        .map_err(|e| format!("Failed to initialize Tesseract (is tesseract-ocr installed?): {}", e))?;

    lt.set_image(image_path)
        .map_err(|e| format!("Leptonica failed to load image: {}", e))?;

    let text = lt.get_utf8_text()
        .map_err(|e| format!("Tesseract text recognition failed: {}", e))?;

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("No text recognized in selected image region".to_string());
    }

    Ok(trimmed.to_string())
}

/// Triggers interactive screen region capture via xdg-desktop-portal,
/// performs Tesseract OCR, and ingests extracted text into active clipboard & SQLite history.
///
/// Uses our own native portal client (no external screenshot tools required).
pub fn run_ocr_capture_and_ingest(
    db: std::sync::Arc<parking_lot::Mutex<rusqlite::Connection>>,
    app_weak: slint::Weak<crate::AppWindow>,
) {
    std::thread::spawn(move || {
        IS_OCR_RUNNING.store(true, Ordering::SeqCst);

        struct OcrGuard;
        impl Drop for OcrGuard {
            fn drop(&mut self) {
                // If a screenshot image landed on clipboard from portal, record its hash so watcher ignores it
                if let Ok(Some((_, hash))) = crate::backend::clipboard::get_current_image() {
                    crate::backend::clipboard::LAST_IMAGE_HASH.store(hash, Ordering::SeqCst);
                }
                IS_OCR_RUNNING.store(false, Ordering::SeqCst);
            }
        }
        let _guard = OcrGuard;

        // Check if OCR feature is enabled in config
        let config_mgr = crate::config::UserSettingsManager::new();
        let settings = config_mgr.load();
        if !settings.enable_ocr_feature {
            eprintln!("[OCR] Screen OCR feature is disabled in preferences.");
            return;
        }

        // Hide main window during screen capture so it doesn't appear in screenshot
        let app_weak_clone = app_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = app_weak_clone.upgrade() {
                let _ = app.window().hide();
            }
        });

        // Give compositor 300ms to fully unmap our window before showing the selection UI
        std::thread::sleep(std::time::Duration::from_millis(300));

        // Use our native xdg-desktop-portal client to show region selection UI
        let captured_path = match crate::backend::screen_capture::capture_region_via_portal() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("[OCR] {}", e);
                return;
            }
        };

        eprintln!("[OCR] Captured region: {}", captured_path.display());

        // Perform OCR text recognition on the captured image
        let ocr_result = extract_text_from_file(&captured_path);

        // Clean up the temp file created by the portal
        let _ = std::fs::remove_file(&captured_path);

        match ocr_result {
            Ok(text) => {
                eprintln!("[OCR] Extracted text:\n{}", text);

                // Push to clipboard & database
                if let Err(e) = crate::backend::clipboard::push_extracted_text(&text, &db) {
                    eprintln!("[OCR Error]: Failed to push to clipboard: {}", e);
                } else {
                    // Refresh Slint UI data model safely on main thread
                    let db_clone = db.clone();
                    let app_weak_clone = app_weak.clone();
                    slint::invoke_from_event_loop(move || {
                        crate::ui::helpers::refresh_clips(app_weak_clone, db_clone, String::new());
                    }).ok();
                }
            }
            Err(err) => {
                eprintln!("[OCR Error]: {}", err);
            }
        }
    });
}
