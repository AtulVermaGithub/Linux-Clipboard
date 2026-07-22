#!/bin/bash
# =============================================================================
# lincb.ople.in - One-Line Universal Installer Script
# Usage: curl -sS https://lincb.ople.in/install.sh | bash
# Homepage: https://lincb.ople.in
# GitHub:   https://github.com/AtulVermaGithub/Linux-Clipboard
# =============================================================================

set -e

DEFAULT_VERSION="0.0.1"
APP_NAME="lincb.ople.in"
PKG_NAME="lincb.ople.in"
GITHUB_REPO="AtulVermaGithub/Linux-Clipboard"

# Styling & Colors
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
BOLD='\033[1m'
RESET='\033[0m'

print_banner() {
    echo -e "${CYAN}"
    echo '  ╔══════════════════════════════════════════════════════════════════╗'
    echo '  ║                     lincb.ople.in Installer                      ║'
    echo '  ║       Native Windows 11-style Clipboard History for Linux        ║'
    echo '  ╚══════════════════════════════════════════════════════════════════╝'
    echo -e "${RESET}"
}

log()   { echo -e "${CYAN}[INFO]${RESET} $*"; }
ok()    { echo -e "${GREEN}[  ✓ ]${RESET} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${RESET} $*"; }
error() { error -e "${RED}[ERR ]${RESET} $*"; }

# ─── Step 1: Detect Architecture & Environment ──────────────────────────────
check_architecture() {
    ARCH="$(uname -m)"
    if [ "$ARCH" != "x86_64" ] && [ "$ARCH" != "amd64" ]; then
        echo -e "${RED}[ERR ]${RESET} Architecture '$ARCH' is currently not supported (x86_64 required)."
        exit 1
    fi
}

check_root_or_sudo() {
    if [ "$EUID" -eq 0 ]; then
        SUDO_CMD=""
        TARGET_USER="${SUDO_USER:-$USER}"
    else
        if command -v sudo &>/dev/null; then
            SUDO_CMD="sudo"
            TARGET_USER="$USER"
        else
            echo -e "${RED}[ERR ]${RESET} 'sudo' command not found. Please run this script as root or install sudo."
            exit 1
        fi
    fi
}

detect_distro() {
    DISTRO_ID="unknown"
    DISTRO_LIKE=""
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        DISTRO_ID="${ID:-unknown}"
        DISTRO_LIKE="${ID_LIKE:-}"
    fi
    log "Detected system OS: ${BOLD}${DISTRO_ID}${RESET}"
}

# ─── Step 2: Fetch Latest Release Version from GitHub API ────────────────────
get_latest_version() {
    local tag=""
    if command -v curl &>/dev/null; then
        tag="$(curl -sSL "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
    elif command -v wget &>/dev/null; then
        tag="$(wget -qO- "https://api.github.com/repos/${GITHUB_REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
    fi

    if [ -n "$tag" ]; then
        # Strip leading 'v' if present (e.g. v0.0.1 -> 0.0.1)
        echo "${tag#v}"
    else
        echo "$DEFAULT_VERSION"
    fi
}

is_valid_package() {
    local file="$1"
    [ -s "$file" ] || return 1
    if head -n 5 "$file" 2>/dev/null | grep -iq "<html\|<!DOCTYPE html\|404: Not Found\|401: Unauthorized"; then
        return 1
    fi
    return 0
}

# ─── Step 3: Helper Functions for Dynamic Package Downloads ─────────────────
download_package() {
    local filename="$1"
    local subpath="$2" # e.g. "deb" or "arch" or "rpm" or "tarball"
    local dest="$3"

    # Local fallback if testing directly in project directory
    if [ -f "releases/${subpath}/${filename}" ]; then
        log "Using local package file: releases/${subpath}/${filename}"
        cp "releases/${subpath}/${filename}" "$dest"
        return 0
    fi

    # URL 1: GitHub /releases/latest/download redirect (Always latest release file)
    local url1="https://github.com/${GITHUB_REPO}/releases/latest/download/${filename}"

    # URL 2: Explicit version release tag download
    local url2="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/${filename}"

    # URL 3: GitHub main branch raw storage
    local url3="https://raw.githubusercontent.com/${GITHUB_REPO}/main/releases/${subpath}/${filename}"

    # URL 4: Web domain fallback
    local url4="https://lincb.ople.in/releases/${subpath}/${filename}"

    local fetch_cmd=""
    if command -v curl &>/dev/null; then
        fetch_cmd="curl -sSL -f"
    elif command -v wget &>/dev/null; then
        fetch_cmd="wget -qO-"
    else
        echo -e "${RED}[ERR ]${RESET} Neither curl nor wget found. Please install curl or wget."
        exit 1
    fi

    log "Downloading ${filename} (v${VERSION})..."
    for url in "$url1" "$url2" "$url3" "$url4"; do
        if $fetch_cmd "$url" > "$dest" 2>/dev/null && is_valid_package "$dest"; then
            return 0
        fi
    done

    echo -e "${RED}[ERR ]${RESET} Could not download valid package file (${filename})."
    echo -e "${YELLOW}[HINT]${RESET} Because your GitHub repository is currently PRIVATE, release files cannot be downloaded over public URL until published."
    exit 1
}

# ─── Step 4: Package-Manager Specific Installation ────────────────────────────
install_deb() {
    log "Installing via Debian / Ubuntu package (.deb)..."
    local DEB_FILE="${PKG_NAME}_${VERSION}_amd64.deb"
    local TMP_DEB="$(mktemp /tmp/lincb_XXXXXX.deb)"

    download_package "$DEB_FILE" "deb" "$TMP_DEB"

    log "Installing system dependencies and package..."
    $SUDO_CMD apt-get update -qq || true
    $SUDO_CMD apt-get install -y -qq "$TMP_DEB" 2>/dev/null || \
    ($SUDO_CMD dpkg -i "$TMP_DEB" && $SUDO_CMD apt-get install -f -y -qq)

    rm -f "$TMP_DEB"
}

install_arch() {
    log "Installing via Arch Linux package (.pkg.tar.zst)..."
    local PKG_FILE="${PKG_NAME}-${VERSION}-1-x86_64.pkg.tar.zst"
    local TMP_PKG="$(mktemp /tmp/lincb_XXXXXX.pkg.tar.zst)"

    download_package "$PKG_FILE" "arch" "$TMP_PKG"

    log "Installing with pacman..."
    $SUDO_CMD pacman -Sy --needed --noconfirm xdotool gtk3 2>/dev/null || true
    $SUDO_CMD pacman -U --noconfirm "$TMP_PKG"

    rm -f "$TMP_PKG"
}

install_rpm() {
    log "Installing via RPM package (.rpm)..."
    local RPM_FILE="${PKG_NAME}-${VERSION}-1.x86_64.rpm"
    local TMP_RPM="$(mktemp /tmp/lincb_XXXXXX.rpm)"

    download_package "$RPM_FILE" "rpm" "$TMP_RPM"

    log "Installing with dnf/rpm..."
    if command -v dnf &>/dev/null; then
        $SUDO_CMD dnf install -y "$TMP_RPM"
    else
        $SUDO_CMD rpm -Uvh --nodeps "$TMP_RPM"
    fi

    rm -f "$TMP_RPM"
}

install_tarball_generic() {
    log "Installing via generic standalone release..."
    local TAR_FILE="${APP_NAME}-${VERSION}-linux-x86_64.tar.gz"
    local TMP_DIR="$(mktemp -d /tmp/lincb_install_XXXXXX)"
    local TMP_TAR="$TMP_DIR/lincb.tar.gz"

    download_package "$TAR_FILE" "tarball" "$TMP_TAR"

    tar -xzf "$TMP_TAR" -C "$TMP_DIR"
    local EXTRACTED_DIR="$(find "$TMP_DIR" -maxdepth 1 -type d -name "lincb*" | head -1)"
    [ -z "$EXTRACTED_DIR" ] && EXTRACTED_DIR="$TMP_DIR"

    if [ -f "$EXTRACTED_DIR/install.sh" ]; then
        (cd "$EXTRACTED_DIR" && $SUDO_CMD bash ./install.sh)
    else
        $SUDO_CMD install -Dm755 "$EXTRACTED_DIR/$APP_NAME" "/usr/local/bin/$APP_NAME"
        $SUDO_CMD ln -sf "/usr/local/bin/$APP_NAME" "/usr/local/bin/linux-clipboard"
        if [ -f "$EXTRACTED_DIR/icon.png" ]; then
            $SUDO_CMD install -Dm644 "$EXTRACTED_DIR/icon.png" "/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"
        fi
    fi

    rm -rf "$TMP_DIR"
}

# ─── Step 5: System Integration & Permissions ──────────────────────────────
setup_permissions_and_autostart() {
    log "Configuring udev permissions & autostart..."

    # 1. udev rules for uinput access without root
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660"' | $SUDO_CMD tee /etc/udev/rules.d/99-lincb-uinput.rules >/dev/null
    $SUDO_CMD modprobe uinput 2>/dev/null || true
    $SUDO_CMD udevadm control --reload-rules 2>/dev/null || true
    $SUDO_CMD udevadm trigger 2>/dev/null || true

    # 2. User group membership
    if [ -n "$TARGET_USER" ] && [ "$TARGET_USER" != "root" ]; then
        if getent group input >/dev/null 2>&1; then
            $SUDO_CMD usermod -aG input "$TARGET_USER" 2>/dev/null || true
            ok "Added user '${BOLD}$TARGET_USER${RESET}' to '${BOLD}input${RESET}' group."
        fi
    fi

    # 3. Update icon cache
    if command -v gtk-update-icon-cache &>/dev/null; then
        $SUDO_CMD gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
    fi
}

# ─── Main Routine ─────────────────────────────────────────────────────────────
main() {
    print_banner
    check_architecture
    check_root_or_sudo
    detect_distro

    # Dynamically query GitHub for latest release version
    VERSION="$(get_latest_version)"
    log "Target version: ${BOLD}v${VERSION}${RESET}"

    local installed=0
    case "$DISTRO_ID" in
        ubuntu|debian|pop|linuxmint|elementary|raspbian|kali)
            install_deb && installed=1 || install_tarball_generic && installed=1
            ;;
        arch|manjaro|endeavouros|garuda|artix)
            install_arch && installed=1 || install_tarball_generic && installed=1
            ;;
        fedora|rhel|centos|rocky|almalinux|opensuse*)
            install_rpm && installed=1 || install_tarball_generic && installed=1
            ;;
        *)
            if [[ "$DISTRO_LIKE" == *"debian"* ]] || [[ "$DISTRO_LIKE" == *"ubuntu"* ]]; then
                install_deb && installed=1 || install_tarball_generic && installed=1
            elif [[ "$DISTRO_LIKE" == *"arch"* ]]; then
                install_arch && installed=1 || install_tarball_generic && installed=1
            elif [[ "$DISTRO_LIKE" == *"fedora"* ]] || [[ "$DISTRO_LIKE" == *"rhel"* ]]; then
                install_rpm && installed=1 || install_tarball_generic && installed=1
            else
                install_tarball_generic && installed=1
            fi
            ;;
    esac

    if [ "$installed" -ne 1 ]; then
        echo -e "${RED}[ERR ]${RESET} Installation failed."
        exit 1
    fi

    setup_permissions_and_autostart

    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════════════╗${RESET}"
    echo -e "${GREEN}║  ✓  lincb.ople.in v${VERSION} installed successfully!                ║${RESET}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════════════╝${RESET}"
    echo ""
    echo -e "${BOLD}To launch the application:${RESET}"
    echo -e "  • Type ${CYAN}lincb.ople.in${RESET} or ${CYAN}linux-clipboard${RESET} in terminal"
    echo -e "  • Or open ${CYAN}Linux Clipboard${RESET} from your application launcher"
    echo ""
    echo -e "${YELLOW}IMPORTANT:${RESET} Please ${BOLD}log out and log back in${RESET} so the 'input' group permission takes effect."
    echo ""
}

main "$@"
