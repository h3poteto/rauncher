DESTDIR ?=

.PHONY: all
all: build

.PHONY: build-x11
build-x11:
	cargo build --release --features x11

.PHONY: build-wayland
build-wayland:
	cargo build --release --features wayland --no-default-features

.PHONY: install
install:
	install -Dm755 target/release/rauncher $(DESTDIR)/usr/bin/rauncher
	install -Dm644 data/rauncher.desktop $(DESTDIR)/usr/share/applications/rauncher.desktop
	install -Dm644 data/icons/hicolor/128x128/apps/rauncher.png $(DESTDIR)/usr/share/icons/hicolor/128x128/apps/rauncher.png
	gtk-update-icon-cache -f /usr/share/icons/hicolor

.PHONY: uninstall
uninstall:
	rm -f $(DESTDIR)/usr/bin/rauncher
	rm -f $(DESTDIR)/usr/share/applications/rauncher.desktop
	rm -f $(DESTDIR)/usr/share/icons/hicolor/128x128/apps/rauncher.png
