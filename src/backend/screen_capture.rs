//! Native screen region capture using xdg-desktop-portal via zbus.
//! Calls org.freedesktop.portal.Screenshot with interactive=true.
//! No external tools (gnome-screenshot, grim, slurp, etc.) required.

use std::collections::HashMap;
use std::path::PathBuf;
use zbus::blocking::{Connection, MessageIterator};
use zbus::MatchRule;
use zbus::zvariant::{OwnedValue, Value};

/// Capture a screen region using the xdg-desktop-portal Screenshot portal.
/// Shows GNOME's / KDE's native interactive screenshot UI (region-selection crosshair).
/// Returns the path to the saved PNG file on success.
pub fn capture_region_via_portal() -> Result<PathBuf, String> {
    // Connect to the session bus
    let conn = Connection::session()
        .map_err(|e| format!("[OCR] Failed to connect to D-Bus session: {}", e))?;

    // Build options: interactive=true triggers region-selection UI in GNOME / KDE Plasma
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("interactive", Value::Bool(true));
    options.insert("handle_token", Value::Str("lincb_ocr1".into()));

    // Call org.freedesktop.portal.Screenshot.Screenshot
    // Returns the request object path we must subscribe to
    let reply = conn
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Screenshot"),
            "Screenshot",
            &("", &options),
        )
        .map_err(|e| format!("[OCR] Portal Screenshot call failed: {}", e))?;

    let handle: zbus::zvariant::OwnedObjectPath = reply
        .body()
        .deserialize()
        .map_err(|e| format!("[OCR] Failed to read portal response handle: {}", e))?;

    let handle_str = handle.as_str().to_owned();

    // Build a match rule to listen for Response signals on our specific request handle
    let match_rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.portal.Request")
        .map_err(|e| format!("[OCR] Bad interface name: {}", e))?
        .member("Response")
        .map_err(|e| format!("[OCR] Bad member name: {}", e))?
        .path(handle_str.as_str())
        .map_err(|e| format!("[OCR] Bad path: {}", e))?
        .build();

    // Create a blocking iterator that yields only matching signals
    let mut iter = MessageIterator::for_match_rule(match_rule, &conn, Some(1))
        .map_err(|e| format!("[OCR] Failed to subscribe to portal Response: {}", e))?;

    // Wait up to 60 seconds for the user to complete or cancel selection
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);

    loop {
        if std::time::Instant::now() > deadline {
            return Err("[OCR] Timed out waiting for screenshot selection.".to_string());
        }

        match iter.next() {
            Some(Ok(msg)) => {
                // Response body: (u response_code, a{sv} results)
                let (code, results): (u32, HashMap<String, OwnedValue>) = msg
                    .body()
                    .deserialize()
                    .map_err(|e| format!("[OCR] Failed to decode portal response: {}", e))?;

                if code != 0 {
                    return Err(format!(
                        "[OCR] User cancelled screenshot selection (code: {}).",
                        code
                    ));
                }

                // Extract the 'uri' from results
                let uri_value = results
                    .get("uri")
                    .ok_or_else(|| "[OCR] Portal response missing 'uri' field.".to_string())?;

                let uri_str: String = match <&str>::try_from(uri_value) {
                    Ok(s) => s.to_string(),
                    Err(_) => uri_value.to_string().trim_matches('"').to_string(),
                };

                // Convert file:// URI to PathBuf
                let path_encoded = uri_str
                    .strip_prefix("file://")
                    .ok_or_else(|| format!("[OCR] Portal URI is not a file:// path: {}", uri_str))?;

                let decoded = percent_encoding::percent_decode_str(path_encoded)
                    .decode_utf8_lossy()
                    .to_string();

                return Ok(PathBuf::from(decoded));
            }
            Some(Err(e)) => {
                return Err(format!("[OCR] Error receiving portal signal: {}", e));
            }
            None => {
                // Iterator exhausted without receiving signal; retry loop
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}
