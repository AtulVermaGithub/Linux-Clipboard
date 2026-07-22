//! Helper functions for the Slint user interface.
//! Manages refreshing the clipboard history list and emoji grid views.

use std::sync::Arc;
use parking_lot::Mutex;
use rusqlite::Connection;
use base64::Engine;
use slint::{ModelRc, VecModel};
use crate::{AppWindow, SlintClipItem, SlintEmojiRow, SlintEmojiItem};
use crate::backend::db::{ClipboardContent, get_history};

/// Helper to decode base64 PNG into a Slint image
pub fn load_slint_image_from_base64(base64_str: &str) -> Option<slint::Image> {
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
pub fn refresh_clips(app_weak: slint::Weak<AppWindow>, conn: Arc<Mutex<Connection>>, search_text: String) {
    if let Some(app) = app_weak.upgrade() {
        let db = conn.lock();
        if let Ok(history) = get_history(&db) {
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

/// Refreshes the Slint UI emoji grid using emojis crate
pub fn refresh_emojis(app_weak: slint::Weak<AppWindow>, category_idx: i32, search_text: String) {
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
