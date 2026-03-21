# ── Toolchain ─────────────────────────────────────────────────────────────────
CARGO   ?= cargo
INSTALL ?= install

# ── Build profile ─────────────────────────────────────────────────────────────
PROFILE         ?= release
CARGO_BUILD_FLAGS ?=

ifeq ($(PROFILE),release)
	CARGO_BUILD_FLAGS += --release
endif

TARGET_DIR   = target/$(PROFILE)
BIN_MAIN     = $(TARGET_DIR)/dynamic-hibernate
BIN_NOTIFIER = $(TARGET_DIR)/dynamic-hibernate-notifier

# ── Installation prefix / DESTDIR ─────────────────────────────────────────────
# Override with:  make install PREFIX=/usr  or  make install DESTDIR=/pkg/root
PREFIX  ?= /usr
DESTDIR ?=

# ── Destination paths (all respect DESTDIR for packaging) ─────────────────────
LIBEXECDIR      = $(DESTDIR)$(PREFIX)/lib/dynamic-hibernate
SYSTEMD_SYSTEM  = $(DESTDIR)$(PREFIX)/lib/systemd/system
SYSTEMD_USER    = $(DESTDIR)$(PREFIX)/lib/systemd/user
DBUS_POLICY_DIR = $(DESTDIR)$(PREFIX)/share/dbus-1/system.d

# These go under /etc — not under PREFIX — they are machine-local config.
LOGIND_DROP_IN  = $(DESTDIR)/etc/systemd/system/systemd-logind.service.d
SLEEP_CONF_DIR  = $(DESTDIR)/etc/systemd/sleep.conf.d
TMPFILES_DIR    = $(DESTDIR)/etc/tmpfiles.d

# ── Phony targets ─────────────────────────────────────────────────────────────
.PHONY: all build install uninstall clean help

# ── Default ───────────────────────────────────────────────────────────────────
all: build

# ── Build ─────────────────────────────────────────────────────────────────────
build:
	$(CARGO) build $(CARGO_BUILD_FLAGS)

# ── Install ───────────────────────────────────────────────────────────────────
install: build
	# Binaries
	$(INSTALL) -Dm755 $(BIN_MAIN)     $(LIBEXECDIR)/dynamic-hibernate
	$(INSTALL) -Dm755 $(BIN_NOTIFIER) $(LIBEXECDIR)/dynamic-hibernate-notifier

	# systemd system units
	$(INSTALL) -Dm644 systemd/dynamic-hibernate-prepare.service \
		$(SYSTEMD_SYSTEM)/dynamic-hibernate-prepare.service
	$(INSTALL) -Dm644 systemd/dynamic-hibernate-cleanup.service \
		$(SYSTEMD_SYSTEM)/dynamic-hibernate-cleanup.service

	# systemd user unit (notifier — per-user session)
	$(INSTALL) -Dm644 systemd/dynamic-hibernate-notifier.service \
		$(SYSTEMD_USER)/dynamic-hibernate-notifier.service

	# D-Bus system policy
	$(INSTALL) -Dm644 dbus/org.dynamic_hibernate.DynamicHibernate.conf \
		$(DBUS_POLICY_DIR)/org.dynamic_hibernate.DynamicHibernate.conf

	# logind drop-in (bypasses pre-hibernate memory check)
	$(INSTALL) -Dm644 conf/logind.conf.d/00-dynamic-hibernate-logind.conf \
		$(LOGIND_DROP_IN)/00-dynamic-hibernate-logind.conf

	# sleep configuration (s2idle + suspend-then-hibernate)
	$(INSTALL) -Dm644 conf/sleep.conf.d/00-dynamic-hibernate-sleep.conf \
		$(SLEEP_CONF_DIR)/00-dynamic-hibernate-sleep.conf

	# tmpfiles.d (image_size=0, zswap knobs)
	$(INSTALL) -Dm644 conf/tmpfiles.d/dynamic-hibernate.conf \
		$(TMPFILES_DIR)/dynamic-hibernate.conf

	@echo ""
	@echo "Installation complete. Run 'make post-install' to activate."

# ── Post-install (only run on the live system, not during packaging) ──────────
# Not called automatically from install so DESTDIR-based packaging stays clean.
.PHONY: post-install
post-install:
	systemctl daemon-reload
	# Oneshot services triggered by hibernate.target — enable only, never start directly.
	systemctl enable dynamic-hibernate-prepare.service
	systemctl enable dynamic-hibernate-cleanup.service
	# User-session notifier — enable globally for all users.
	systemctl --global enable dynamic-hibernate-notifier.service
	# Apply kernel knobs immediately without reboot.
	systemd-tmpfiles --create /etc/tmpfiles.d/dynamic-hibernate.conf
	# Reload logind to pick up the memory-check bypass drop-in.
	systemctl restart systemd-logind.service
	@echo ""
	@echo "Add to your kernel parameters:"
	@echo "  hibernate.compressor=lzo zswap.compressor=lzo"

# ── Uninstall ─────────────────────────────────────────────────────────────────
uninstall:
	# Disable services first (best-effort — may not be enabled)
	-systemctl disable dynamic-hibernate-prepare.service
	-systemctl disable dynamic-hibernate-cleanup.service
	-systemctl --global disable dynamic-hibernate-notifier.service

	# Remove all installed files
	rm -f $(LIBEXECDIR)/dynamic-hibernate
	rm -f $(LIBEXECDIR)/dynamic-hibernate-notifier
	rm -f $(SYSTEMD_SYSTEM)/dynamic-hibernate-prepare.service
	rm -f $(SYSTEMD_SYSTEM)/dynamic-hibernate-cleanup.service
	rm -f $(SYSTEMD_USER)/dynamic-hibernate-notifier.service
	rm -f $(DBUS_POLICY_DIR)/org.dynamic_hibernate.DynamicHibernate.conf
	rm -f $(LOGIND_DROP_IN)/00-dynamic-hibernate-logind.conf
	rm -f $(SLEEP_CONF_DIR)/00-dynamic-hibernate-sleep.conf
	rm -f $(TMPFILES_DIR)/dynamic-hibernate.conf

	# Remove binary dir if empty
	-rmdir $(LIBEXECDIR)

	systemctl daemon-reload
	@echo "Uninstall complete."

# ── Clean ─────────────────────────────────────────────────────────────────────
clean:
	$(CARGO) clean

# ── Help ──────────────────────────────────────────────────────────────────────
help:
	@echo "Targets:"
	@echo "  make                   Build release binary (default)"
	@echo "  make build             Build (PROFILE=debug for debug build)"
	@echo "  make install           Install all files (supports DESTDIR and PREFIX)"
	@echo "  make post-install      Enable/start services on the live system"
	@echo "  make uninstall         Remove all installed files and disable services"
	@echo "  make clean             Remove cargo build artifacts"
	@echo ""
	@echo "Variables:"
	@echo "  DESTDIR=               Staging root for packaging (default: empty)"
	@echo "  PREFIX=/usr            Installation prefix (default: /usr)"
	@echo "  PROFILE=release        Build profile: release or debug"
