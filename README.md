# dynamic-hibernate

Dynamic hibernation swap manager for Linux — Btrfs + s2idle + zswap-assisted sizing.

## What it does

Creates a correctly-sized btrfs swapfile immediately before hibernation,
configures the kernel resume pointers, and removes the file after resume.
No permanent swap partition needed. Swapfile size is computed from live
kernel state, not a fixed fraction of RAM.

## Sizing strategy

```
swapfile_kb = (must_save_kb / zswap_ratio × 1.10) + vram_kb + 1024
```

Where:

| Term           | Source                      | Notes                                                                      |
|----------------|-----------------------------|----------------------------------------------------------------------------|
| `must_save_kb` | `/proc/meminfo` subtraction | Non-reclaimable pages only; read frozen at s2idle transition               |
| `zswap_ratio`  | `/sys/kernel/debug/zswap/`  | Live LZO ratio; falls back to 1.0 if misaligned or sparse                  |
| `vram_kb`      | `nvidia-smi`                | NVIDIA VRAM saved by driver, bypasses kernel compressor                    |
| `1.10`         | Safety margin               | Absorbs ratio variance between zswap cold-page sample and full working set |
| `1024 KB`      | Kernel overhead             | Hibernate image header + page bitmap                                       |

`image_size=0` is written before every hibernate to instruct the kernel to
maximally drop reclaimable pages before taking the snapshot.

## Platform target

- Arch Linux / CachyOS
- Btrfs root filesystem
- Intel Alder Lake / Tiger Lake (s2idle / S0ix, no S3)
- NVIDIA Optimus (supergfxd) — MX 550 or equivalent

## Requirements

| Package             | Purpose                               |
|---------------------|---------------------------------------|
| `btrfs-progs ≥ 6.1` | `btrfs filesystem mkswapfile`         |
| `util-linux`        | `swapon` / `swapoff`                  |
| `systemd ≥ 253`     | `systemd-hibernate-clear.service`     |
| `nvidia-utils`      | `nvidia-smi` (optional — VRAM sizing) |

## Kernel parameters

Add to your bootloader config:

```
hibernate.compressor=lzo
zswap.compressor=lzo
```

Both must use the same algorithm for the zswap ratio to transfer to the
hibernate write path. LZO is the only option compiled into stock CachyOS
for both zswap and hibernate without a custom kernel build.

## Installation

```bash
# Build
cargo build --release

# Binaries
install -Dm755 target/release/dynamic-hibernate          /usr/lib/dynamic-hibernate/dynamic-hibernate
install -Dm755 target/release/dynamic-hibernate-notifier /usr/lib/dynamic-hibernate/dynamic-hibernate-notifier

# systemd system units
install -Dm644 systemd/dynamic-hibernate-prepare.service /usr/lib/systemd/system/
install -Dm644 systemd/dynamic-hibernate-cleanup.service /usr/lib/systemd/system/

# systemd user unit (notifier)
install -Dm644 systemd/dynamic-hibernate-notifier.service /usr/lib/systemd/user/

# logind memory-check bypass
install -Dm644 conf/logind.conf.d/00-dynamic-hibernate-logind.conf \
    /etc/systemd/system/systemd-logind.service.d/

# Sleep configuration (s2idle + suspend-then-hibernate)
install -Dm644 conf/sleep.conf.d/00-dynamic-hibernate-sleep.conf \
    /etc/systemd/sleep.conf.d/

# Kernel knobs (image_size=0, zswap=lzo/5%/no-writeback)
install -Dm644 conf/tmpfiles.d/dynamic-hibernate.conf \
    /etc/tmpfiles.d/

# D-Bus system policy
install -Dm644 dbus/org.dynamic_hibernate.DynamicHibernate.conf \
    /usr/share/dbus-1/system.d/

# Enable system services
systemctl daemon-reload
systemctl enable dynamic-hibernate-prepare.service
systemctl enable dynamic-hibernate-cleanup.service

# Enable user notifier
systemctl --user enable dynamic-hibernate-notifier.service

# Apply tmpfiles immediately (no reboot needed for first-time setup)
systemd-tmpfiles --create /etc/tmpfiles.d/dynamic-hibernate.conf

# Reload logind to pick up the memory-check bypass
systemctl restart systemd-logind.service
```

## Kernel parameters

In `/etc/kernel/cmdline` or your bootloader:

```
hibernate.compressor=lzo zswap.compressor=lzo
```

Regenerate your unified kernel image or GRUB config as appropriate.

## initramfs resume hook

For UEFI systems, systemd-hibernate-resume reads the `HibernateLocation`
EFI variable automatically — no `resume=` kernel parameter needed.

For BIOS or if you prefer explicit configuration:

```
resume=UUID=<uuid-of-btrfs-partition>
```

The `resume_offset` is written dynamically by the prepare service.

## Troubleshooting

```bash
# Live status and size estimate
sudo dynamic-hibernate status

# Verbose prepare run (dry test without actually hibernating)
sudo DYNAMIC_HIBERNATE_LOG=debug dynamic-hibernate create

# Check compressor alignment
cat /sys/module/zswap/parameters/compressor
cat /sys/module/hibernate/parameters/compressor
# Both must print: lzo

# Check zswap pool sample size (need ≥ 51200 pages for ratio to be used)
sudo cat /sys/kernel/debug/zswap/stored_pages
sudo cat /sys/kernel/debug/zswap/pool_total_size

# Service logs
journalctl -u dynamic-hibernate-prepare.service
journalctl -u dynamic-hibernate-cleanup.service
```

## License

GPL-2.0-only OR GPL-3.0-only OR LicenseRef-KDE-Accepted-GPL
