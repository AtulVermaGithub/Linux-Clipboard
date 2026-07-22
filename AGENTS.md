# Agent Master Roadmap: linux-clipboard (Rust + Slint)

This document is the blueprint and development tracker for **linux-clipboard** (`in.ople.lincb`), a high-performance, native clipboard history manager for Linux. It is built entirely from scratch in **Rust** and **Slint**, avoiding any library lock-ins (no GTK/Qt runtime dependencies), ensuring portability across Ubuntu, Debian, Fedora, Arch Linux, Alpine, and all desktop environments (GNOME, KDE, XFCE, i3, Sway, Hyprland).

---

## 1. Core Technical Stack & Architecture

*   **Programming Language:** Rust
*   **UI Framework:** Slint (`slint` crate)
    *   Compiles UI code directly into native Rust structures.
    *   GPU accelerated via OpenGL/Vulkan, with fallback to software rendering.
    *   Memory footprint: under 12MB RAM; startup time: <5ms.
*   **Database:** SQLite (`rusqlite` crate with the `bundled` feature to avoid system dependencies).
*   **OS Integrations:**
    *   **Clipboard Access:** `arboard` (for text/images) with Wayland data-control support.
    *   **Simulated Paste:** Custom keyboard emulation via X11 XTest and Wayland `/dev/uinput`.
    *   **Single-Instance IPC:** Unix domain sockets (`tokio::net::UnixListener`) for light, instant communication.
    *   **Global Shortcuts:** Desktop environment helper settings (`gsettings`, `kwriteconfig`, `xfconf-query`) and configuration file adapters for tiling window managers.
    *   **System Tray:** Lightweight tray indicator.

---

## 2. Directory Structure

We will structure the project inside `work/` as follows:
vv😀😓😐😆🙃Directory🤔😀😭v🍋
```
work/
├── Cargo.toml             # Project dependencies (Slint, rusqlite, arboard, tokio, etc.)
├── build.rs               # Compiles the Slint markup file
├── Makefile               # Development automation helper
├── ui/
│   ├── app.slint          # Main Slint markup (Layout, Tabs, Cards, Styling)
│   └── icons/             # Graphical assets (PNG/SVG)
├── src/
│   ├── main.rs            # Entry point, Unix socket handler, tokio runtime
│   ├── config.rs          # Settings serializer (~/.config/linux-clipboard/settings.json)
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── db.rs          # SQLite operations (History list, Pinned, Emojis)
│   │   ├── clipboard.rs   # Clipboard watcher and injector daemon
│   │   ├── shortcuts.rs   # Desktop shortcut registers
│   │   └── simulator.rs   # Raw X11 and Wayland uinput key event injection
│   └── ui/
│       ├── mod.rs         # UI controllers and data bindings
│       ├── window.rs      # Window position, centering, and borderless states
│       ├── tray.rs        # System tray context menus
│       └── wizard.rs      # User wizard helper for uinput permissions
└── install.sh             # Curl-ready cross-distro installer
```

---

## 3. Data Schemas & Protocols

### 3.1 SQLite Database Schema (`db.db`)
Stored in `~/.config/linux-clipboard/db.db`:

```sql
CREATE TABLE IF NOT EXISTS history (
    id TEXT PRIMARY KEY,
    item_type TEXT NOT NULL,       -- 'Text', 'RichText', 'Image'
    plain_text TEXT,
    html_content TEXT,
    image_base64 TEXT,
    image_width INTEGER,
    image_height INTEGER,
    timestamp INTEGER NOT NULL,    -- Unix timestamp
    pinned INTEGER NOT NULL DEFAULT 0,
    preview TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS emoji_usage (
    emoji_char TEXT PRIMARY KEY,
    use_count INTEGER NOT NULL DEFAULT 1,
    last_used INTEGER NOT NULL
);
```

### 3.2 Single-Instance IPC Protocol
A Unix domain socket will be bound at `~/.config/linux-clipboard/ipc.sock`.
*   **Primary Instance:** Starts and listens on the socket.
*   **Secondary Instance:** Launched when a hotkey is pressed. Connects to `ipc.sock`, writes arguments (`"toggle"`, `"emoji"`, `"settings"`), and exits.
*   **Message Handler:** The primary instance reads the socket command, activates the Slint window, switches to the requested tab, and repositions the window to the cursor.

---

## 4. Low-Level Integration Specifications

### 4.1 Wayland Input Simulation (`uinput`)
To paste under Wayland:
1.  Open `/dev/uinput` in write mode.
2.  Register virtual keyboard device with keys `KEY_LEFTCTRL` and `KEY_V`.
3.  Simulate Press `KEY_LEFTCTRL` -> Press `KEY_V` -> Release `KEY_V` -> Release `KEY_LEFTCTRL`.
4.  Destroy virtual device.
*Note: Requires user membership in the `input` group and a `udev` rule enabling `/dev/uinput` write permissions.*

### 4.2 X11 Input Simulation
1.  Attempt X11 XTest extension (`xtest_fake_input` via `x11rb`).
2.  Fallback to running `xdotool key --clearmodifiers ctrl+v`.

---

## 5. Slint UI Components (`ui/app.slint`)

The UI will feature:
*   **Drawer Style:** Borderless window with rounded corners.
*   **Search Bar:** Filters history dynamically via SQLite queries.
*   **Tabs:**
    *   *History:* Vertical scrolled list containing cards.
    *   *Emojis:* Grid layout of emojis grouped by categories.
    *   *GIFs:* Placeholder search interface.
*   **Card Controls:** Hover overlays showing "Pin/Unpin" and "Delete" icons.

---

## 6. Detailed Task Checklist

- [ ] **Phase 1: Project Scaffolding**
    - [ ] Create `Cargo.toml` and write a basic `build.rs` to compile Slint.
    - [ ] Write `ui/app.slint` with a basic borderless layout.
    - [ ] Write a starter `src/main.rs` that loads and opens the Slint window.
- [ ] **Phase 2: Database & Config Storage**
    - [ ] Implement `src/config.rs` for settings.
    - [ ] Implement SQLite schema and CRUD methods in `src/backend/db.rs`.
- [ ] **Phase 3: Daemon Services & Single-Instance IPC**
    - [ ] Implement Unix Domain Socket IPC for single-instance control in `src/main.rs`.
    - [ ] Implement clipboard watcher thread in `src/backend/clipboard.rs`.
- [ ] **Phase 4: Input Simulation & OS Grabbing**
    - [ ] Implement paste simulation (`xtest`, `xdotool`, `uinput`) in `src/backend/simulator.rs`.
    - [ ] Implement window positioning and cursor alignment in `src/ui/window.rs`.
    - [ ] Implement global hotkey registers for DEs in `src/backend/shortcuts.rs`.
- [ ] **Phase 5: Slint Interface Polish**
    - [ ] Expand `ui/app.slint` to display full history cards, emojis, and search.
    - [ ] Hook up database queries and UI callback bindings.
- [ ] **Phase 6: Setup Wizard & Tray Icon**
    - [ ] Build first-run wizard inside Slint for uinput checks.
    - [ ] Add system tray support using a Rust system tray crate.
- [ ] **Phase 7: Installer & Build Automation**
    - [ ] Create `Makefile` for local installation and rules.
    - [ ] Write `install.sh` for single-line curl executions.