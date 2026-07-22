//! Database management module using SQLite
//! Manages persistent storage of clipboard history and emoji usage.

use std::path::Path;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

/// Content type for clipboard items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ClipboardContent {
    /// Plain text content
    Text(String),
    /// Rich text with HTML formatting
    RichText { plain: String, html: String },
    /// Image data: base64 encoded PNG, dimensions
    Image {
        base64: String,
        width: u32,
        height: u32,
    },
}

/// A single clipboard history item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    pub content: ClipboardContent,
    pub timestamp: DateTime<Utc>,
    pub pinned: bool,
    pub preview: String,
}

/// Emoji usage details
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub struct EmojiUsage {
    pub char: String,
    pub use_count: u32,
    pub last_used: u64,
}

/// Initialize SQLite database and tables
pub fn init_db(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    
    let conn = Connection::open(db_path)?;
    
    // Create history table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            id TEXT PRIMARY KEY,
            item_type TEXT NOT NULL,
            plain_text TEXT,
            html_text TEXT,
            image_base64 TEXT,
            image_width INTEGER,
            image_height INTEGER,
            timestamp TEXT NOT NULL,
            pinned INTEGER NOT NULL DEFAULT 0,
            preview TEXT NOT NULL
        )",
        [],
    )?;

    // Create emoji usage table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS emoji_usage (
            char TEXT PRIMARY KEY,
            use_count INTEGER NOT NULL DEFAULT 1,
            last_used INTEGER NOT NULL
        )",
        [],
    )?;

    Ok(conn)
}

/// Insert or update a clipboard item in the database
pub fn insert_item(conn: &Connection, item: &ClipboardItem) -> Result<()> {
    // 1. Check for duplicates
    let existing_id: Option<String> = match &item.content {
        ClipboardContent::Text(text) => {
            conn.query_row(
                "SELECT id FROM history WHERE item_type = 'Text' AND plain_text = ?1 LIMIT 1",
                params![text],
                |row| row.get::<_, String>(0),
            ).ok()
        }
        ClipboardContent::RichText { plain, html } => {
            conn.query_row(
                "SELECT id FROM history WHERE item_type = 'RichText' AND plain_text = ?1 AND html_text = ?2 LIMIT 1",
                params![plain, html],
                |row| row.get::<_, String>(0),
            ).ok()
        }
        ClipboardContent::Image { base64, .. } => {
            conn.query_row(
                "SELECT id FROM history WHERE item_type = 'Image' AND image_base64 = ?1 LIMIT 1",
                params![base64],
                |row| row.get::<_, String>(0),
            ).ok()
        }
    };

    let ts_str = item.timestamp.to_rfc3339();

    if let Some(id) = existing_id {
        // Update the timestamp of the existing item to move it to the top
        conn.execute(
            "UPDATE history SET timestamp = ?1 WHERE id = ?2",
            params![ts_str, id],
        )?;
        return Ok(());
    }

    let (item_type, plain, html, img_b64, img_w, img_h) = match &item.content {
        ClipboardContent::Text(text) => ("Text", Some(text.clone()), None, None, None, None),
        ClipboardContent::RichText { plain, html } => {
            ("RichText", Some(plain.clone()), Some(html.clone()), None, None, None)
        }
        ClipboardContent::Image { base64, width, height } => {
            ("Image", None, None, Some(base64.clone()), Some(*width as i64), Some(*height as i64))
        }
    };

    let pinned_val = if item.pinned { 1 } else { 0 };

    conn.execute(
        "INSERT OR REPLACE INTO history (
            id, item_type, plain_text, html_text, image_base64, image_width, image_height, timestamp, pinned, preview
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            item.id,
            item_type,
            plain,
            html,
            img_b64,
            img_w,
            img_h,
            ts_str,
            pinned_val,
            item.preview
        ],
    )?;

    Ok(())
}

/// Load entire clipboard history from the database
pub fn get_history(conn: &Connection) -> Result<Vec<ClipboardItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, item_type, plain_text, html_text, image_base64, image_width, image_height, timestamp, pinned, preview 
         FROM history ORDER BY pinned DESC, timestamp DESC"
    )?;

    let item_iter = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let item_type: String = row.get(1)?;
        let plain_text: Option<String> = row.get(2)?;
        let html_text: Option<String> = row.get(3)?;
        let image_base64: Option<String> = row.get(4)?;
        let image_width: Option<i64> = row.get(5)?;
        let image_height: Option<i64> = row.get(6)?;
        let timestamp_str: String = row.get(7)?;
        let pinned_val: i32 = row.get(8)?;
        let preview: String = row.get(9)?;

        let content = match item_type.as_str() {
            "RichText" => ClipboardContent::RichText {
                plain: plain_text.unwrap_or_default(),
                html: html_text.unwrap_or_default(),
            },
            "Image" => ClipboardContent::Image {
                base64: image_base64.unwrap_or_default(),
                width: image_width.unwrap_or(0) as u32,
                height: image_height.unwrap_or(0) as u32,
            },
            _ => ClipboardContent::Text(plain_text.unwrap_or_default()),
        };

        let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(ClipboardItem {
            id,
            content,
            timestamp,
            pinned: pinned_val != 0,
            preview,
        })
    })?;

    let mut items = Vec::new();
    for item in item_iter {
        items.push(item?);
    }
    Ok(items)
}

/// Delete a specific clipboard item by ID
pub fn delete_item(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
    Ok(())
}

/// Toggle the pinned state of a clipboard item
pub fn toggle_pin(conn: &Connection, id: &str) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT pinned FROM history WHERE id = ?1")?;
    let currently_pinned: i32 = stmt.query_row(params![id], |row| row.get(0))?;
    let new_pinned = if currently_pinned == 0 { 1 } else { 0 };

    conn.execute(
        "UPDATE history SET pinned = ?1 WHERE id = ?2",
        params![new_pinned, id],
    )?;

    Ok(new_pinned != 0)
}

/// Clear all unpinned items from clipboard history
pub fn clear_history(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM history WHERE pinned = 0", [])?;
    Ok(())
}

/// Enforces database limits by deleting old unpinned items
pub fn cleanup_old_items(conn: &Connection, max_items: usize, auto_delete_minutes: u64) -> Result<bool> {
    let mut changed = false;

    // 1. Time-based cleanup
    if auto_delete_minutes > 0 {
        let threshold = Utc::now() - chrono::Duration::minutes(auto_delete_minutes as i64);
        let threshold_str = threshold.to_rfc3339();
        let rows_deleted = conn.execute(
            "DELETE FROM history WHERE pinned = 0 AND timestamp < ?1",
            params![threshold_str],
        )?;
        if rows_deleted > 0 {
            changed = true;
        }
    }

    // 2. Count-based limit
    let total_count: i64 = conn.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;
    if total_count > max_items as i64 {
        let mut stmt = conn.prepare(
            "SELECT id FROM history WHERE pinned = 0 
             ORDER BY timestamp ASC LIMIT ?1"
        )?;
        let excess = total_count - max_items as i64;
        let id_iter = stmt.query_map(params![excess], |row| row.get::<_, String>(0))?;
        
        let mut ids_to_delete = Vec::new();
        for id_res in id_iter {
            ids_to_delete.push(id_res?);
        }

        if !ids_to_delete.is_empty() {
            let tx = conn.unchecked_transaction()?;
            for id in &ids_to_delete {
                tx.execute("DELETE FROM history WHERE id = ?1", params![id])?;
            }
            tx.commit()?;
            changed = true;
        }
    }

    Ok(changed)
}

/// Increment emoji usage count or insert new usage details
pub fn record_emoji_usage(conn: &Connection, emoji: &str) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO emoji_usage (char, use_count, last_used) 
         VALUES (?1, 1, ?2) 
         ON CONFLICT(char) DO UPDATE SET 
            use_count = use_count + 1, 
            last_used = ?2",
        params![emoji, now],
    )?;
    Ok(())
}

/// Retrieve top emojis ordered by usage frequency
#[allow(dead_code)]
pub fn get_recent_emojis(conn: &Connection, limit: usize) -> Result<Vec<EmojiUsage>> {
    let mut stmt = conn.prepare(
        "SELECT char, use_count, last_used 
         FROM emoji_usage 
         ORDER BY use_count DESC, last_used DESC 
         LIMIT ?1"
    )?;

    let emoji_iter = stmt.query_map(params![limit], |row| {
        Ok(EmojiUsage {
            char: row.get(0)?,
            use_count: row.get(1)?,
            last_used: row.get::<_, i64>(2)? as u64,
        })
    })?;

    let mut list = Vec::new();
    for emoji in emoji_iter {
        list.push(emoji?);
    }
    Ok(list)
}
