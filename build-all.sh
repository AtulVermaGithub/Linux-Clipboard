#!/bin/bash
# =============================================================================
# lincb.ople.in - Multi-distro Release Build Script
# Version: 0.0.1 (Beta)
# Produces: .deb, Arch .pkg.tar.zst, .rpm, AppImage, and generic .tar.gz
# =============================================================================

set -e

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
BOLD='\033[1m'
RESET='\033[0m'

APP_NAME="lincb.ople.in"
PKG_NAME="lincb.ople.in"
VERSION="0.0.2"
ARCH="amd64"
ARCH_LINUX="x86_64"
DESCRIPTION="A native Clipboard History Manager for Linux, built with Rust and Slint"
MAINTAINER="ople.in <admin@ople.in>"
HOMEPAGE="https://lincb.ople.in"
LICENSE="MIT"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="$SCRIPT_DIR"
RELEASES_DIR="$WORK_DIR/releases"
BINARY="$WORK_DIR/target/release/$APP_NAME"
ICON="$WORK_DIR/icon.png"

log()  { echo -e "${CYAN}[BUILD]${RESET} $*"; }
ok()   { echo -e "${GREEN}  ✓${RESET} $*"; }
warn() { echo -e "${YELLOW}  ⚠${RESET} $*"; }
err()  { echo -e "${RED}  ✗${RESET} $*"; exit 1; }

banner() {
    echo ""
    echo -e "${BOLD}${CYAN}╔══════════════════════════════════════════════════╗${RESET}"
    echo -e "${BOLD}${CYAN}║  lincb.ople.in  –  Release Builder  v${VERSION}        ║${RESET}"
    echo -e "${BOLD}${CYAN}╚══════════════════════════════════════════════════╝${RESET}"
    echo ""
}

# ─── Step 0: Compile ───────────────────────────────────────────────────────
compile() {
    if [ -f "$BINARY" ]; then
        ok "Binary already exists: $BINARY ($(du -sh "$BINARY" | cut -f1)) — skipping recompile."
        ok "To force recompile, delete the binary and re-run this script."
        return
    fi
    log "Compiling release binary..."
    cd "$WORK_DIR"

    [ -d "/tmp/zig-linux-x86_64-0.13.0" ] && export PATH="/tmp/zig-linux-x86_64-0.13.0:$PATH"

    # Support Zig cross-compilation for universal GLIBC 2.17 baseline
    if command -v cargo-zigbuild &>/dev/null && command -v zig &>/dev/null; then
        log "Using cargo-zigbuild targeting universal GLIBC 2.17 baseline..."
        cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17
        mkdir -p target/release
        local FOUND_BIN="$(find target -type f -name "lincb-ople-in" -not -path "*/deps/*" -not -path "*/build/*" | head -1)"
        if [ -n "$FOUND_BIN" ]; then
            cp -f "$FOUND_BIN" "$BINARY"
        fi
    else
        log "Compiling standard release build..."
        cargo build --release
        local FOUND_BIN="$(find target -type f -name "lincb-ople-in" -not -path "*/deps/*" -not -path "*/build/*" | head -1)"
        if [ -n "$FOUND_BIN" ]; then
            cp -f "$FOUND_BIN" "$BINARY"
        fi
    fi

    [ -f "$BINARY" ] || err "Binary not found at $BINARY after compilation!"
    ok "Binary compiled: $BINARY ($(du -sh "$BINARY" | cut -f1))"
}

# ─── Step 1: .deb (Debian / Ubuntu / Mint / Pop!_OS) ──────────────────────
build_deb() {
    log "Building .deb package..."
    local PKG_DIR="$RELEASES_DIR/deb/build/${PKG_NAME}_${VERSION}_amd64"
    rm -rf "$PKG_DIR"

    # Directory tree
    install -dm755 "$PKG_DIR/DEBIAN"
    install -dm755 "$PKG_DIR/usr/bin"
    install -dm755 "$PKG_DIR/usr/share/applications"
    install -dm755 "$PKG_DIR/usr/share/icons/hicolor/256x256/apps"
    install -dm755 "$PKG_DIR/etc/xdg/autostart"
    install -dm755 "$PKG_DIR/usr/lib/udev/rules.d"

    # Binary & legacy symlink
    install -m755 "$BINARY" "$PKG_DIR/usr/bin/$APP_NAME"
    ln -sf "$APP_NAME" "$PKG_DIR/usr/bin/linux-clipboard"

    # Icon
    install -m644 "$ICON" "$PKG_DIR/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"

    # Desktop file
    cat > "$PKG_DIR/usr/share/applications/${APP_NAME}.desktop" << EOF
[Desktop Entry]
Name=Linux Clipboard
Comment=Lightweight, native clipboard history manager
Exec=/usr/bin/${APP_NAME}
Icon=${APP_NAME}
Terminal=false
Type=Application
Categories=Utility;
StartupNotify=false
StartupWMClass=lincb.ople.in
EOF
    cp "$PKG_DIR/usr/share/applications/${APP_NAME}.desktop" \
       "$PKG_DIR/etc/xdg/autostart/${APP_NAME}.desktop"

    # udev rule
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660"' \
        > "$PKG_DIR/usr/lib/udev/rules.d/99-lincb-uinput.rules"

    # DEBIAN/control
    INSTALLED_SIZE=$(du -sk "$PKG_DIR" | cut -f1)
    cat > "$PKG_DIR/DEBIAN/control" << EOF
Package: ${PKG_NAME}
Version: ${VERSION}
Architecture: amd64
Maintainer: ${MAINTAINER}
Installed-Size: ${INSTALLED_SIZE}
Depends: libgtk-3-0, libglib2.0-0, libc6 (>= 2.17), xdotool, libxdo3
Section: utils
Priority: optional
Homepage: ${HOMEPAGE}
Description: ${DESCRIPTION}
 lincb.ople.in is a fast, native clipboard history manager for Linux.
 It works on X11 and Wayland, supports text, image, and emoji clips,
 and provides a keyboard-driven interface inspired by Windows 11.
EOF

    # DEBIAN/postinst
    cat > "$PKG_DIR/DEBIAN/postinst" << 'EOF'
#!/bin/sh
set -e
# Add current user to input group for uinput access
if [ "$1" = "configure" ]; then
    modprobe uinput 2>/dev/null || true
    udevadm control --reload-rules 2>/dev/null || true
    udevadm trigger 2>/dev/null || true
    if [ -n "$SUDO_USER" ]; then
        usermod -aG input "$SUDO_USER" 2>/dev/null || true
    fi
fi
# Update icon cache
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
fi
EOF
    chmod 755 "$PKG_DIR/DEBIAN/postinst"

    # DEBIAN/prerm
    cat > "$PKG_DIR/DEBIAN/prerm" << 'EOF'
#!/bin/sh
set -e
pkill -f "lincb.ople.in" 2>/dev/null || true
EOF
    chmod 755 "$PKG_DIR/DEBIAN/prerm"

    # Build the .deb
    local OUT="$RELEASES_DIR/deb/${PKG_NAME}_${VERSION}_amd64.deb"
    fakeroot dpkg-deb --build "$PKG_DIR" "$OUT"
    ok "DEB package: $OUT ($(du -sh "$OUT" | cut -f1))"
}

# ─── Step 2: Arch Linux PKGBUILD + .pkg.tar.zst ───────────────────────────
build_arch() {
    log "Building Arch Linux package..."
    local ARCH_DIR="$RELEASES_DIR/arch/build"
    rm -rf "$ARCH_DIR"
    mkdir -p "$ARCH_DIR"

    # Copy binary and assets for packaging
    local STAGE="$ARCH_DIR/pkg"
    install -dm755 "$STAGE/usr/bin"
    install -dm755 "$STAGE/usr/share/applications"
    install -dm755 "$STAGE/usr/share/icons/hicolor/256x256/apps"
    install -dm755 "$STAGE/etc/xdg/autostart"
    install -dm755 "$STAGE/usr/lib/udev/rules.d"

    install -m755 "$BINARY" "$STAGE/usr/bin/$APP_NAME"
    ln -sf "$APP_NAME" "$STAGE/usr/bin/linux-clipboard"
    install -m644 "$ICON"   "$STAGE/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"

    cat > "$STAGE/usr/share/applications/${APP_NAME}.desktop" << EOF
[Desktop Entry]
Name=Linux Clipboard
Comment=Lightweight, native clipboard history manager
Exec=/usr/bin/${APP_NAME}
Icon=${APP_NAME}
Terminal=false
Type=Application
Categories=Utility;
StartupNotify=false
StartupWMClass=lincb.ople.in
EOF
    cp "$STAGE/usr/share/applications/${APP_NAME}.desktop" \
       "$STAGE/etc/xdg/autostart/${APP_NAME}.desktop"
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660"' \
        > "$STAGE/usr/lib/udev/rules.d/99-lincb-uinput.rules"

    # PKGBUILD (for AUR / manual build reference)
    cat > "$RELEASES_DIR/arch/PKGBUILD" << EOF
# Maintainer: ${MAINTAINER}
pkgname=${PKG_NAME}
pkgver=${VERSION}
pkgrel=1
pkgdesc='${DESCRIPTION}'
arch=('x86_64')
url='${HOMEPAGE}'
license=('${LICENSE}')
depends=('gtk3' 'glib2' 'libx11' 'libxtst' 'xdotool')
source=("\${pkgname}-\${pkgver}-\${pkgarch}.tar.gz")
sha256sums=('SKIP')

package() {
    cd "\$srcdir"
    install -Dm755 usr/bin/${APP_NAME} "\${pkgdir}/usr/bin/${APP_NAME}"
    ln -sf ${APP_NAME} "\${pkgdir}/usr/bin/linux-clipboard"
    install -Dm644 usr/share/applications/${APP_NAME}.desktop "\${pkgdir}/usr/share/applications/${APP_NAME}.desktop"
    install -Dm644 usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png "\${pkgdir}/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"
    install -Dm644 etc/xdg/autostart/${APP_NAME}.desktop "\${pkgdir}/etc/xdg/autostart/${APP_NAME}.desktop"
    install -Dm644 usr/lib/udev/rules.d/99-lincb-uinput.rules "\${pkgdir}/usr/lib/udev/rules.d/99-lincb-uinput.rules"
}
EOF

    # .install hook
    cat > "$RELEASES_DIR/arch/${PKG_NAME}.install" << 'EOF'
post_install() {
    # Ensure xdotool / libxdo is installed
    if ! command -v xdotool >/dev/null 2>&1; then
        echo "--> Auto-installing missing dependency: xdotool..."
        pacman -Sy --needed --noconfirm xdotool 2>/dev/null || true
    fi
    modprobe uinput 2>/dev/null || true
    udevadm control --reload-rules 2>/dev/null || true
    udevadm trigger 2>/dev/null || true
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
    echo "NOTE: Add yourself to the 'input' group and re-login:"
    echo "  sudo usermod -aG input \$USER"
}
post_upgrade() { post_install; }
pre_remove() { pkill -f 'lincb.ople.in' 2>/dev/null || true; }
EOF

    # Create source tarball that PKGBUILD references
    local TAR_NAME="${PKG_NAME}-${VERSION}-x86_64.tar.gz"
    (cd "$STAGE" && tar -czf "$RELEASES_DIR/arch/$TAR_NAME" .)

    # Create .PKGINFO
    INSTALLED_SIZE=$(du -sk "$STAGE" | cut -f1)
    cat > "$STAGE/.PKGINFO" << EOF
pkgname = ${PKG_NAME}
pkgver = ${VERSION}-1
arch = x86_64
pkgdesc = ${DESCRIPTION}
url = ${HOMEPAGE}
builddate = $(date +%s)
packager = ${MAINTAINER}
size = $((INSTALLED_SIZE * 1024))
depend = gtk3
depend = glib2
depend = libx11
depend = libxtst
depend = xdotool
EOF

    # Create .INSTALL from the install hook
    cp "$RELEASES_DIR/arch/${PKG_NAME}.install" "$STAGE/.INSTALL"

    # Pack Arch package (.pkg.tar.zst or fallback to .pkg.tar.xz)
    local PKG_OUT_NAME="${PKG_NAME}-${VERSION}-1-x86_64.pkg.tar.zst"

    if command -v zstd &>/dev/null; then
        (cd "$STAGE" && tar -c --zstd -f "$RELEASES_DIR/arch/$PKG_OUT_NAME" .PKGINFO .INSTALL usr etc 2>/dev/null)
    elif command -v zstdmt &>/dev/null; then
        (cd "$STAGE" && tar -c --use-compress-program=zstdmt -f "$RELEASES_DIR/arch/$PKG_OUT_NAME" .PKGINFO .INSTALL usr etc 2>/dev/null)
    else
        PKG_OUT_NAME="${PKG_NAME}-${VERSION}-1-x86_64.pkg.tar.xz"
        (cd "$STAGE" && tar -cJf "$RELEASES_DIR/arch/$PKG_OUT_NAME" .PKGINFO .INSTALL usr etc 2>/dev/null)
    fi

    ok "Arch package:  $RELEASES_DIR/arch/$PKG_OUT_NAME ($(du -sh "$RELEASES_DIR/arch/$PKG_OUT_NAME" | cut -f1))"
    ok "PKGBUILD:      $RELEASES_DIR/arch/PKGBUILD  (for AUR submission)"
}

# ─── Step 3: .rpm (Fedora / RHEL / openSUSE) ──────────────────────────────
build_rpm() {
    log "Building RPM spec + package..."
    local RPM_DIR="$RELEASES_DIR/rpm"
    local STAGE_DIR="$RPM_DIR/STAGE"

    # RPM build tree
    mkdir -p "$RPM_DIR"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS} "$STAGE_DIR"
    local SPEC="$RPM_DIR/SPECS/${PKG_NAME}.spec"

    # Stage files into STAGE_DIR
    rm -rf "$STAGE_DIR"
    install -dm755 "$STAGE_DIR/usr/bin"
    install -dm755 "$STAGE_DIR/usr/share/applications"
    install -dm755 "$STAGE_DIR/usr/share/icons/hicolor/256x256/apps"
    install -dm755 "$STAGE_DIR/etc/xdg/autostart"
    install -dm755 "$STAGE_DIR/usr/lib/udev/rules.d"

    install -m755 "$BINARY" "$STAGE_DIR/usr/bin/$APP_NAME"
    ln -sf "$APP_NAME" "$STAGE_DIR/usr/bin/linux-clipboard"
    install -m644 "$ICON"   "$STAGE_DIR/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"

    cat > "$STAGE_DIR/usr/share/applications/${APP_NAME}.desktop" << EOF
[Desktop Entry]
Name=Linux Clipboard
Comment=Lightweight, native clipboard history manager
Exec=/usr/bin/${APP_NAME}
Icon=${APP_NAME}
Terminal=false
Type=Application
Categories=Utility;
StartupNotify=false
StartupWMClass=lincb.ople.in
EOF
    cp "$STAGE_DIR/usr/share/applications/${APP_NAME}.desktop" \
       "$STAGE_DIR/etc/xdg/autostart/${APP_NAME}.desktop"
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660"' \
        > "$STAGE_DIR/usr/lib/udev/rules.d/99-lincb-uinput.rules"

    # Write RPM spec
    cat > "$SPEC" << EOF
Name:           ${PKG_NAME}
Version:        ${VERSION}
Release:        1%{?dist}
Summary:        ${DESCRIPTION}
License:        ${LICENSE}
URL:            ${HOMEPAGE}
BuildArch:      x86_64

%global __requires_exclude ^libm\\.so\\.6\\(GLIBC_

Requires:       gtk3
Requires:       glib2
Requires:       libX11
Requires:       libXtst
Requires:       xdotool

%description
lincb.ople.in is a fast, native clipboard history manager for Linux.
It works on X11 and Wayland, supports text, image, and emoji clips,
and provides a keyboard-driven interface inspired by Windows 11.

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a ${STAGE_DIR}/* %{buildroot}/

%post
modprobe uinput 2>/dev/null || true
udevadm control --reload-rules 2>/dev/null || true
udevadm trigger 2>/dev/null || true
gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true

%preun
pkill -f "lincb.ople.in" 2>/dev/null || true

%files
/usr/bin/${APP_NAME}
/usr/bin/linux-clipboard
/usr/share/applications/${APP_NAME}.desktop
/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png
/etc/xdg/autostart/${APP_NAME}.desktop
/usr/lib/udev/rules.d/99-lincb-uinput.rules

%changelog
* $(date '+%a %b %d %Y') ${MAINTAINER} - ${VERSION}-1
- Beta release 0.0.1
EOF

    # Build the .rpm if rpmbuild is available
    if command -v rpmbuild &>/dev/null; then
        rpmbuild --define "_topdir $RPM_DIR" \
                 --define "_builddir $RPM_DIR/BUILD" \
                 --define "_rpmdir $RPM_DIR/RPMS" \
                 --define "_srcrpmdir $RPM_DIR/SRPMS" \
                 --define "_buildrootdir $RPM_DIR/BUILDROOT" \
                 --nodeps \
                 -bb "$SPEC" 2>&1 | tail -8
        # Copy result out
        find "$RPM_DIR/RPMS" -name "*.rpm" -exec cp {} "$RELEASES_DIR/rpm/" \;
        ok "RPM package:  $RELEASES_DIR/rpm/${PKG_NAME}-${VERSION}-1.x86_64.rpm"
    else
        warn "rpmbuild not found — RPM spec written but .rpm not built."
        warn "On Fedora/RHEL: sudo dnf install rpm-build, then:"
        warn "  rpmbuild --define '_topdir $RPM_DIR' -bb $SPEC"
    fi
    ok "RPM spec:      $SPEC"
}

# ─── Step 4: Generic tarball (.tar.gz) ───────────────────────────────────
build_tarball() {
    log "Building generic tarball..."
    local STAGE="$RELEASES_DIR/tarball/build/${APP_NAME}-${VERSION}"
    rm -rf "$STAGE"
    mkdir -p "$STAGE"

    cp "$BINARY"   "$STAGE/$APP_NAME"
    cp "$ICON"     "$STAGE/icon.png"

    cat > "$STAGE/install.sh" << 'INNEREOF'
#!/bin/bash
# lincb.ople.in Generic Installer
set -e
APP="lincb.ople.in"
PREFIX="${PREFIX:-/usr/local}"
BINDIR="$PREFIX/bin"
DATADIR="$PREFIX/share"

echo "Installing $APP to $BINDIR..."
sudo install -Dm755 "$APP" "$BINDIR/$APP"
sudo ln -sf "$APP" "$BINDIR/linux-clipboard"
sudo install -Dm644 icon.png "$DATADIR/icons/hicolor/256x256/apps/${APP}.png"
sudo mkdir -p "$DATADIR/applications" /etc/xdg/autostart

cat | sudo tee "$DATADIR/applications/${APP}.desktop" << EOF
[Desktop Entry]
Name=Linux Clipboard
Comment=Lightweight, native clipboard history manager
Exec=${BINDIR}/${APP}
Icon=${APP}
Terminal=false
Type=Application
Categories=Utility;
StartupNotify=false
StartupWMClass=lincb.ople.in
EOF
sudo cp "$DATADIR/applications/${APP}.desktop" /etc/xdg/autostart/
echo 'KERNEL=="uinput", GROUP="input", MODE="0660"' \
    | sudo tee /usr/lib/udev/rules.d/99-lincb-uinput.rules >/dev/null
sudo modprobe uinput 2>/dev/null || true
sudo udevadm control --reload-rules 2>/dev/null || true
sudo udevadm trigger 2>/dev/null || true
sudo usermod -aG input "$USER" 2>/dev/null || true
gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
echo "✓ Installed! Re-login for group permissions to take effect."
INNEREOF
    chmod +x "$STAGE/install.sh"

    cat > "$STAGE/README.txt" << EOF
lincb.ople.in ${VERSION} – Beta Release
=========================================
A native clipboard history manager for Linux.

INSTALLATION:
  Generic (any distro):  sudo ./install.sh
  Debian/Ubuntu:         sudo dpkg -i lincb-ople-in_${VERSION}_amd64.deb
  Arch Linux:            sudo pacman -U lincb-ople-in-${VERSION}-1-x86_64.pkg.tar.zst
  Fedora/RHEL:           sudo rpm -i lincb-ople-in-${VERSION}-1.x86_64.rpm

USAGE:
  Run: lincb.ople.in
  Or launch via your application menu.

NOTE: You must be in the 'input' group. Run:
  sudo usermod -aG input \$USER
Then log out and back in.

Homepage: https://lincb.ople.in
EOF

    local OUT="$RELEASES_DIR/tarball/${APP_NAME}-${VERSION}-linux-x86_64.tar.gz"
    tar -czf "$OUT" -C "$RELEASES_DIR/tarball/build" "${APP_NAME}-${VERSION}"
    ok "Tarball:       $OUT ($(du -sh "$OUT" | cut -f1))"
}

# ─── Step 5: AppImage ────────────────────────────────────────────────────
build_appimage() {
    log "Building AppImage..."
    local APPDIR="$RELEASES_DIR/appimage/build/${APP_NAME}.AppDir"
    local APPIMAGETOOL="$RELEASES_DIR/appimagetool"
    rm -rf "$APPDIR"
    mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/icons/hicolor/256x256/apps"

    # AppRun — entry point executed when AppImage runs
    cat > "$APPDIR/AppRun" << 'EOF'
#!/bin/bash
HERE="$(dirname "$(readlink -f "${0}")")"  
export PATH="$HERE/usr/bin:$PATH"
exec "$HERE/usr/bin/lincb.ople.in" "$@"
EOF
    chmod +x "$APPDIR/AppRun"

    # Binary & legacy symlink
    install -m755 "$BINARY" "$APPDIR/usr/bin/$APP_NAME"
    ln -sf "$APP_NAME" "$APPDIR/usr/bin/linux-clipboard"

    # Icon (both at root and hicolor path — appimagetool needs root-level)
    install -m644 "$ICON" "$APPDIR/usr/share/icons/hicolor/256x256/apps/${APP_NAME}.png"
    install -m644 "$ICON" "$APPDIR/${APP_NAME}.png"
    # Symlink .DirIcon for appimagetool
    ln -sf "${APP_NAME}.png" "$APPDIR/.DirIcon"

    # Desktop file (must be at AppDir root)
    cat > "$APPDIR/${APP_NAME}.desktop" << EOF
[Desktop Entry]
Name=Linux Clipboard
Comment=Lightweight, native clipboard history manager
Exec=${APP_NAME}
Icon=${APP_NAME}
Terminal=false
Type=Application
Categories=Utility;
StartupNotify=false
StartupWMClass=lincb.ople.in
EOF

    # Run appimagetool
    mkdir -p "$RELEASES_DIR/appimage"
    local OUT="$RELEASES_DIR/appimage/${APP_NAME}-${VERSION}-x86_64.AppImage"

    if [ ! -f "$RELEASES_DIR/appimagetool" ]; then
        log "Downloading appimagetool..."
        wget -q "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" -O "$RELEASES_DIR/appimagetool" || true
        chmod +x "$RELEASES_DIR/appimagetool" || true
    fi

    if [ -f "$RELEASES_DIR/appimagetool" ] && [ ! -d "$RELEASES_DIR/squashfs-root" ]; then
        log "Extracting appimagetool..."
        (cd "$RELEASES_DIR" && ./appimagetool --appimage-extract > /dev/null 2>&1) || true
    fi

    local TOOL_RUN=""
    if [ -x "$RELEASES_DIR/squashfs-root/AppRun" ]; then
        TOOL_RUN="$RELEASES_DIR/squashfs-root/AppRun"
    elif [ -x "$WORK_DIR/squashfs-root/AppRun" ]; then
        TOOL_RUN="$WORK_DIR/squashfs-root/AppRun"
    fi

    if [ -n "$TOOL_RUN" ]; then
        ARCH=x86_64 "$TOOL_RUN" --no-appstream "$APPDIR" "$OUT" 2>&1 | tail -8
        chmod +x "$OUT"
        ok "AppImage:      $OUT ($(du -sh "$OUT" | cut -f1))"
    else
        warn "appimagetool not found — AppImage skipped."
    fi
}

# ─── Step 6: Checksums ───────────────────────────────────────────────────
generate_checksums() {
    log "Generating checksums..."
    local SUM_FILE="$RELEASES_DIR/SHA256SUMS"
    > "$SUM_FILE"

    find "$RELEASES_DIR" \( -name "*.deb" -o -name "*.pkg.tar.zst" \
        -o -name "*.rpm" -o -name "*.tar.gz" -o -name "*.AppImage" \) \
        -not -path "*/build/*" -not -path "*/RPMS/*" -not -path "*/SOURCES/*" -not -path "*/STAGE/*" \
        | sort | while read -r f; do
            sha256sum "$f" | sed "s|$RELEASES_DIR/||" >> "$SUM_FILE"
        done

    ok "Checksums:     $SUM_FILE"
    cat "$SUM_FILE"
}

# ─── Main ────────────────────────────────────────────────────────────────
main() {
    banner
    compile
    echo ""
    build_deb
    build_arch
    build_rpm
    build_tarball
    build_appimage
    echo ""
    generate_checksums
    echo ""
    echo -e "${BOLD}${GREEN}╔══════════════════════════════════════════════════╗${RESET}"
    echo -e "${BOLD}${GREEN}║  All packages built successfully!                ║${RESET}"
    echo -e "${BOLD}${GREEN}╚══════════════════════════════════════════════════╝${RESET}"
    echo ""
    echo -e "${CYAN}Output files:${RESET}"
    find "$RELEASES_DIR" \( -name "*.deb" -o -name "*.pkg.tar.zst" \
        -o -name "*.rpm" -o -name "*.tar.gz" -o -name "*.AppImage" \) \
        -not -path "*/build/*" | sort | while read -r f; do
            echo -e "  ${GREEN}→${RESET} $(basename "$f")  ($(du -sh "$f" | cut -f1))"
        done
    echo ""
}

main "$@"
