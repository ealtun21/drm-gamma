BIN     = drm-gamma
PREFIX ?= /usr/local
CONF    = /etc/default/drm-gamma.conf
UNIT    = /etc/systemd/system

VERSION ?= 0.0.0
ARCH    ?= $(shell dpkg --print-architecture 2>/dev/null || echo amd64)
DEB_DIR  = build-deb/$(BIN)_$(VERSION)_$(ARCH)

.PHONY: build test install install-notifier uninstall deb clean
.DEFAULT_GOAL := build

build:
	cargo build --release

test:
	cargo test --release

install: build
	install -D -m 755 target/release/$(BIN) $(PREFIX)/bin/$(BIN)
	# Never clobber an existing (calibrated) config.
	[ -e $(CONF) ] || install -D -m 644 drm-gamma.conf $(CONF)
	sed 's|/usr/local/bin|$(PREFIX)/bin|g' scripts/drm-gamma.service > $(UNIT)/drm-gamma.service
	systemctl daemon-reload
	@echo "Edit $(CONF), then: sudo systemctl enable --now drm-gamma"

install-notifier:
	install -D -m 755 scripts/drm-gamma-notify.sh   $(PREFIX)/bin/drm-gamma-notify.sh
	install -D -m 755 scripts/drm-gamma-notifier.sh $(PREFIX)/bin/drm-gamma-notifier.sh
	sed -i 's|/usr/local/bin|$(PREFIX)/bin|g' $(PREFIX)/bin/drm-gamma-notifier.sh
	sed 's|/usr/local/bin|$(PREFIX)/bin|g' scripts/drm-gamma-notifier.service > $(UNIT)/drm-gamma-notifier.service
	systemctl daemon-reload

uninstall:
	-systemctl disable --now drm-gamma drm-gamma-notifier 2>/dev/null
	rm -f $(PREFIX)/bin/$(BIN) $(PREFIX)/bin/drm-gamma-notify.sh $(PREFIX)/bin/drm-gamma-notifier.sh
	rm -f $(UNIT)/drm-gamma.service $(UNIT)/drm-gamma-notifier.service
	systemctl daemon-reload
	@echo "Config left in place: $(CONF)"

deb: build
	rm -rf $(DEB_DIR)
	install -D -m 755 target/release/$(BIN) $(DEB_DIR)/usr/bin/$(BIN)
	install -D -m 755 scripts/drm-gamma-notify.sh   $(DEB_DIR)/usr/bin/drm-gamma-notify.sh
	install -D -m 755 scripts/drm-gamma-notifier.sh $(DEB_DIR)/usr/bin/drm-gamma-notifier.sh
	install -D -m 644 drm-gamma.conf $(DEB_DIR)$(CONF)
	install -D -m 644 scripts/drm-gamma.service          $(DEB_DIR)/usr/lib/systemd/system/drm-gamma.service
	install -D -m 644 scripts/drm-gamma-notifier.service $(DEB_DIR)/usr/lib/systemd/system/drm-gamma-notifier.service
	install -D -m 644 README.md $(DEB_DIR)/usr/share/doc/$(BIN)/README.md
	sed -i 's|/usr/local/bin|/usr/bin|g' \
		$(DEB_DIR)/usr/lib/systemd/system/*.service $(DEB_DIR)/usr/bin/drm-gamma-notifier.sh
	mkdir -p $(DEB_DIR)/DEBIAN
	printf '%s\n' \
		"Package: $(BIN)" "Version: $(VERSION)" "Architecture: $(ARCH)" \
		"Maintainer: ealtun21 <ealtun21@users.noreply.github.com>" \
		"Depends: libc6, libgcc-s1" "Recommends: libnotify-bin" \
		"Section: utils" "Priority: optional" \
		"Homepage: https://github.com/ealtun21/drm-gamma" \
		"Description: Persistent per-channel gamma and color temperature via DRM" \
		" Sets RGB gamma / color temperature directly on DRM CRTCs, independent of" \
		" the X11 or Wayland compositor. Daemon re-applies it on TTY switch." \
		> $(DEB_DIR)/DEBIAN/control
	echo "$(CONF)" > $(DEB_DIR)/DEBIAN/conffiles
	printf '#!/bin/sh\nset -e\n[ "$$1" = configure ] && systemctl daemon-reload || true\n' > $(DEB_DIR)/DEBIAN/postinst
	printf '#!/bin/sh\nset -e\nif [ "$$1" = remove ] || [ "$$1" = purge ]; then\n  systemctl disable --now drm-gamma drm-gamma-notifier 2>/dev/null || true\nfi\n' > $(DEB_DIR)/DEBIAN/prerm
	chmod 755 $(DEB_DIR)/DEBIAN/postinst $(DEB_DIR)/DEBIAN/prerm
	dpkg-deb --build --root-owner-group $(DEB_DIR) build-deb/

clean:
	cargo clean
	rm -rf build-deb
