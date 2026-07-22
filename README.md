# 📋 Linux Clipboard (`lincb.ople.in`)

<p align="center">
  <img src="icon.png" width="128" height="128" alt="lincb.ople.in logo" />
</p>

<h3 align="center">A Native Windows 11-Style Clipboard History Manager for Linux</h3>

<p align="center">
  <a href="https://lincb.ople.in"><img src="https://img.shields.io/badge/Website-lincb.ople.in-0078D4?style=for-the-badge&logo=firefox" alt="Website" /></a>
  <a href="https://github.com/AtulVermaGithub/Linux-Clipboard/releases"><img src="https://img.shields.io/badge/Release-v0.0.1-brightgreen?style=for-the-badge&logo=github" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue?style=for-the-badge" alt="License" /></a>
  <img src="https://img.shields.io/badge/Built_With-Rust_%26_Slint-orange?style=for-the-badge&logo=rust" alt="Rust & Slint" />
</p>

---

## ⚡ Quick One-Line Installation

Install **`lincb.ople.in`** automatically on **any Linux distribution** (Ubuntu, Debian, Arch, Fedora, Mint, Pop!_OS, openSUSE) with a single command:

```bash
curl -sS https://lincb.ople.in/install.sh | bash
```

> **Note:** The installer automatically detects your Linux distribution, downloads the matching package (`.deb`, `.pkg.tar.zst`, `.rpm`, or AppImage), sets up system dependencies, configures `uinput` permissions, and creates desktop launchers automatically.

---

## ✨ Features

- 🗂️ **Windows 11 Experience**: Familiar `Super+V` clipboard history overlay and `Super+.` emoji picker.
- 🦀 **Ultra-Fast & Lightweight**: Built natively with **Rust** and **Slint GUI** — consumes under 25MB RAM with sub-millisecond hotkey response.
- 🎨 **Modern Design**: Fluent design language, glassmorphism effects, light/dark theme adaptation, search filtering, and pinned clip management.
- 🖼️ **Rich Content Support**: Stores and previews text snippets, screenshots, formatted code, and emojis.
- ⚙️ **Wayland & X11 Native**: Built-in support for GNOME, KDE Plasma, XFCE, Hyprland, and custom window managers.

---

## 📦 Package Installation Options

If you prefer to download and install pre-built packages manually:

### 1. Ubuntu / Debian / Pop!_OS / Mint (`.deb`)
```bash
sudo apt install ./lincb.ople.in_0.0.1_amd64.deb
```

### 2. Arch Linux / Manjaro / EndeavourOS (`.pkg.tar.zst`)
```bash
sudo pacman -U lincb.ople.in-0.0.1-1-x86_64.pkg.tar.zst
```

### 3. Fedora / RHEL / openSUSE (`.rpm`)
```bash
sudo dnf install ./lincb.ople.in-0.0.1-1.x86_64.rpm
```

### 4. Portable AppImage (No installation required)
```bash
chmod +x lincb.ople.in-0.0.1-x86_64.AppImage
./lincb.ople.in-0.0.1-x86_64.AppImage
```

---

## ⌨️ Keyboard Shortcuts & Usage

| Shortcut | Action |
|----------|--------|
| **`Super + V`** | Toggle Clipboard History modal |
| **`Super + .`** | Toggle Emoji Picker panel |
| **`Up / Down`** | Navigate through clip history |
| **`Enter`** | Paste selected clip into current active window |
| **`Delete`** | Remove selected clip from history |

---

## 🛠️ Building from Source

### Prerequisites (Ubuntu/Debian):
```bash
sudo apt update && sudo apt install -y build-essential pkg-config libfontconfig1-dev libx11-dev libxtst-dev libxdo-dev libglib2.0-dev libgtk-3-dev
```

### Build & Install:
```bash
git clone https://github.com/AtulVermaGithub/Linux-Clipboard.git
cd Linux-Clipboard/work
cargo build --release
sudo make install
```

---

## 🗑️ Uninstallation

To remove `lincb.ople.in` from your system:

- **Ubuntu / Debian**: `sudo apt remove lincb.ople.in`
- **Arch Linux**: `sudo pacman -R lincb.ople.in`
- **Fedora / RHEL**: `sudo dnf remove lincb.ople.in`

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
Created with ❤️ by **[ople.in](https://lincb.ople.in)**.
