// ── Swap storage ──────────────────────────────────────────────────────────────
pub const SWAP_DIR: &str = "/var/lib/dynamic-hibernate";
pub const SWAP_FILE_NAME: &str = "dynamic-hibernate.swapfile";
pub const METADATA_FILE_NAME: &str = ".dynamic-hibernate.metadata";

// ── Kernel power management sysfs ────────────────────────────────────────────
pub const SYS_RESUME: &str = "/sys/power/resume";
pub const SYS_RESUME_OFFSET: &str = "/sys/power/resume_offset";
pub const SYS_IMAGE_SIZE: &str = "/sys/power/image_size";

// ── zswap debugfs (readable only as root) ────────────────────────────────────
pub const ZSWAP_STORED_PAGES: &str = "/sys/kernel/debug/zswap/stored_pages";
pub const ZSWAP_POOL_TOTAL_SIZE: &str = "/sys/kernel/debug/zswap/pool_total_size";
pub const ZSWAP_WRITEBACK: &str = "/sys/kernel/debug/zswap/writeback";

// ── Compressor alignment ─────────────────────────────────────────────────────
// Both must be the same algorithm for the zswap ratio to transfer to hibernate.
// On stock CachyOS (as of 2025): only LZO is compiled into both zswap and
// hibernate without a custom kernel build (CONFIG_HIBERNATION_COMP_LZ4 is not set).
pub const ZSWAP_COMPRESSOR_PATH: &str = "/sys/module/zswap/parameters/compressor";
pub const HIBERNATE_COMPRESSOR_PATH: &str = "/sys/module/hibernate/parameters/compressor";

// ── D-Bus ─────────────────────────────────────────────────────────────────────
pub const DBUS_INTERFACE: &str = "org.dynamic_hibernate.DynamicHibernate";
pub const DBUS_OBJECT_PATH: &str = "/org/dynamic_hibernate/DynamicHibernate";
pub const DBUS_SIGNAL_MEMBER: &str = "ErrorOccurred";

// ── Sizing knobs ─────────────────────────────────────────────────────────────

/// Minimum zswap stored pages before the pool ratio is considered representative.
/// Below this threshold (~200 MB uncompressed) the sample is too sparse to trust.
pub const MIN_ZSWAP_SAMPLE_PAGES: u64 = 51_200;

/// Safety margin applied to the compressed RAM estimate to absorb ratio variance
/// between the biased zswap sample and the full must-save working set.
pub const SIZING_SAFETY_MARGIN: f64 = 0.10;

/// Kernel hibernate image header + page bitmap overhead (kernel internal, ~1 MB).
pub const HIBERNATE_HEADER_OVERHEAD_KB: u64 = 1_024;

// ── External tool names ───────────────────────────────────────────────────────
pub const TOOL_BTRFS: &str = "btrfs";
pub const TOOL_SWAPON: &str = "swapon";
pub const TOOL_SWAPOFF: &str = "swapoff";
pub const TOOL_BOOTCTL: &str = "bootctl";
pub const TOOL_NVIDIA_SMI: &str = "nvidia-smi";
