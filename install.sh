#!/bin/bash
# linux-clipboard single-line installation script
# Detects distro, installs build dependencies, sets up Rust, and compiles/installs the app.

set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
RESET='\033[0m'

echo -e "${CYAN}=====================================================${RESET}"
echo -e "${CYAN}             Linux-Clipboard Installer               ${RESET}"
echo -e "${CYAN}                         by                          ${RESET}"
echo -e "${CYAN}             github.com/AtulVermaGithub              ${RESET}"
echo -e "${CYAN}=====================================================${RESET}"

# 1. Detect Distro & Install Dependencies
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS_ID=$ID
else
    OS_ID="unknown"
fi

echo -e "Detecting system distribution... ${YELLOW}$OS_ID${RESET}"

case "$OS_ID" in
    ubuntu|debian|pop|linuxmint)
        echo -e "Installing build dependencies via ${GREEN}apt${RESET}..."
        sudo apt update
        sudo apt install -y build-essential pkg-config libfontconfig1-dev \
                            libx11-dev libxtst-dev libxdo-dev libglib2.0-dev \
                            libgtk-3-dev curl git
        ;;
    fedora)
        echo -e "Installing build dependencies via ${GREEN}dnf${RESET}..."
        sudo dnf groupinstall -y "Development Tools"
        sudo dnf install -y fontconfig-devel libX11-devel libXtst-devel \
                            libxdo-devel glib2-devel gtk3-devel curl git
        ;;
    arch|manjaro|endeavouros)
        echo -e "Installing build dependencies via ${GREEN}pacman${RESET}..."
        sudo pacman -Syu --needed --noconfirm base-devel fontconfig libx11 \
                                           libxtst xdotool glib2 gtk3 curl git
        ;;
    *)
        echo -e "${YELLOW}Warning: Unknown distribution. Please make sure build tools, fontconfig, X11, and xdotool development libraries are installed.${RESET}"
        ;;
esac

# 2. Check and Install Rust / Cargo
if ! command -v cargo &> /dev/null; then
    echo -e "${CYAN}Rust/Cargo not found. Installing via rustup...${RESET}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo -e "${GREEN}✓ Rust installed successfully.${RESET}"
else
    echo -e "Rust/Cargo version: ${GREEN}$(cargo --version)${RESET}"
fi

# Make sure PATH has cargo bin in current shell script context
export PATH="$HOME/.cargo/bin:$PATH"

# 3. Handle Local vs Remote Execution
TEMP_DIR=""
if [ -f "./Cargo.toml" ] && grep -q 'name = "linux-clipboard"' ./Cargo.toml; then
    echo -e "${GREEN}Running installation in local project directory.${RESET}"
else
    echo -e "${CYAN}Running installer remotely. Cloning repository...${RESET}"
    TEMP_DIR=$(mktemp -d)
    git clone https://github.com/in-ople/linux-clipboard.git "$TEMP_DIR"
    cd "$TEMP_DIR/work"
fi

# 4. Compilation and Installation
echo -e "${CYAN}Compiling linux-clipboard in release mode...${RESET}"
cargo build --release

echo -e "${CYAN}Installing files...${RESET}"
sudo make install

# 5. Configure Permissions
echo -e "${CYAN}Installing uinput udev rules...${RESET}"
make install-rules

echo -e "${CYAN}Configuring input group permissions...${RESET}"
make add-user-group

# Clean up temp clone if remote
if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
fi

echo -e "${GREEN}=====================================================${RESET}"
echo -e "${GREEN}  ✓ Linux-Clipboard installed successfully!           ${RESET}"
echo -e "${GREEN}=====================================================${RESET}"
echo -e "${YELLOW}IMPORTANT:${RESET}"
echo -e "1. You ${RED}MUST log out and log back in${RESET} for the input group permissions to take effect."
echo -e "2. Shortcuts [Super+V] and [Super+.] are now configured for GNOME, KDE, or XFCE."
echo -e "   For custom window managers, map keys to: 'linux-clipboard' and 'linux-clipboard --emoji'."
echo ""
