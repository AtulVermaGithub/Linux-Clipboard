//! System Tray Icon and Context Menu setup

use tray_icon::{
    menu::{Menu, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

/// Setup the system tray icon and menu
pub fn setup_tray() -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    let show_item = MenuItem::new("Show History", true, None);
    let settings_item = MenuItem::new("Settings", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    menu.append(&show_item)?;
    menu.append(&settings_item)?;
    menu.append(&quit_item)?;

    // Dynamically create a 16x16 blue/white clipboard symbol icon in memory
    let mut pixels = vec![0u8; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let idx = (y * 16 + x) * 4;
            // Draw a basic bordered square in blue
            if x == 0 || x == 15 || y == 0 || y == 15 {
                pixels[idx] = 0;     // R
                pixels[idx + 1] = 120; // G
                pixels[idx + 2] = 212; // B
                pixels[idx + 3] = 255; // A
            } else if y >= 4 && y <= 12 && x >= 4 && x <= 11 {
                // Clipboard sheet (white)
                pixels[idx] = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 255;
                pixels[idx + 3] = 255;
            } else if y >= 2 && y <= 3 && x >= 6 && x <= 9 {
                // Clip (grey)
                pixels[idx] = 150;
                pixels[idx + 1] = 150;
                pixels[idx + 2] = 150;
                pixels[idx + 3] = 255;
            } else {
                // Transparent background
                pixels[idx + 3] = 0;
            }
        }
    }

    let icon = Icon::from_rgba(pixels, 16, 16)?;

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("linux-clipboard")
        .with_icon(icon)
        .build()?;

    Ok(tray_icon)
}
