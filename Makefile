# lincb.ople.in Makefile
# Automates compiling, installing, and configuring permissions

PREFIX ?= /usr/local
BINDIR := $(PREFIX)/bin
DATADIR := $(PREFIX)/share
DESTDIR ?=

APP_NAME := lincb.ople.in
CARGO_BIN := lincb-ople-in
DESKTOP_FILE := lincb.ople.in.desktop

.PHONY: all build install uninstall clean install-rules add-user-group

all: build

build:
	cargo build --release
	cp target/release/$(CARGO_BIN) target/release/$(APP_NAME)

install-rules:
	@echo "Configuring udev permissions for /dev/uinput..."
	echo 'KERNEL=="uinput", GROUP="input", MODE="0660"' | sudo tee /etc/udev/rules.d/99-uinput-clipboard.rules
	sudo modprobe uinput || true
	sudo udevadm control --reload-rules || true
	sudo udevadm trigger || true
	@echo "✓ udev rules installed successfully."

add-user-group:
	@echo "Adding active user to input group..."
	@if [ -n "$$SUDO_USER" ]; then \
		sudo usermod -aG input $$SUDO_USER; \
		echo "✓ Added $$SUDO_USER to input group"; \
		else \
		sudo usermod -aG input $$USER; \
		echo "✓ Added $$USER to input group"; \
	fi
	@echo "⚠️ NOTE: You must log out and log back in for group changes to take effect."

install:
	@echo "Installing $(APP_NAME) binary..."
	install -Dm755 target/release/$(APP_NAME) $(DESTDIR)$(BINDIR)/$(APP_NAME)

	@echo "Installing desktop icon..."
	@mkdir -p $(DESTDIR)$(DATADIR)/icons/hicolor/256x256/apps
	install -Dm644 icon.png $(DESTDIR)$(DATADIR)/icons/hicolor/256x256/apps/$(APP_NAME).png

	@echo "Installing desktop launcher..."
	@mkdir -p $(DESTDIR)$(DATADIR)/applications
	echo '[Desktop Entry]' > $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'Name=Linux Clipboard' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'Comment=Lightweight, native clipboard history manager' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'Exec=$(BINDIR)/$(APP_NAME)' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'Icon=$(APP_NAME)' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'Terminal=false' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'Type=Application' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'Categories=Utility;Application;' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'StartupNotify=false' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	echo 'StartupWMClass=lincb.ople.in' >> $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	chmod 644 $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)

	@# Configure autostart
	@mkdir -p $(DESTDIR)/etc/xdg/autostart
	cp $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE) $(DESTDIR)/etc/xdg/autostart/$(DESKTOP_FILE)

	@echo "✓ Installed successfully! You can run it via launcher or by typing '$(APP_NAME)'."

uninstall:
	@echo "Removing $(APP_NAME)..."
	rm -f $(DESTDIR)$(BINDIR)/$(APP_NAME)
	rm -f $(DESTDIR)$(DATADIR)/applications/$(DESKTOP_FILE)
	rm -f $(DESTDIR)/etc/xdg/autostart/$(DESKTOP_FILE)
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/256x256/apps/$(APP_NAME).png
	rm -f /etc/udev/rules.d/99-uinput-clipboard.rules
	@echo "✓ Uninstalled successfully."

clean:
	cargo clean
