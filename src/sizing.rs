//! Swapfile size estimation.
//!
//! ## Formula
//!
//! ```text
//! compressed_ram_kb = must_save_kb / zswap_ratio
//! safety_kb         = compressed_ram_kb × SIZING_SAFETY_MARGIN   (10 %)
//! swapfile_kb       = compressed_ram_kb + safety_kb + vram_kb + HIBERNATE_HEADER_OVERHEAD_KB
//! ```
//!
//! Where:
//! - `must_save_kb` — non-reclaimable RAM from `/proc/meminfo` (see `MemProbe`)
//! - `zswap_ratio`  — live ratio from zswap debugfs pool (1.0 if unusable)
//! - `vram_kb`      — used NVIDIA VRAM (0 if GPU off; added **uncompressed**)

use crate::config::{HIBERNATE_HEADER_OVERHEAD_KB, SIZING_SAFETY_MARGIN};
use crate::error::HibernateResult;
use crate::probe::{MemProbe, VramProbe, ZswapProbe};

/// Compute the minimum required swapfile size in kilobytes.
pub fn compute_swapfile_kb() -> HibernateResult<u64> {
    let must_save_kb = MemProbe.must_save_kb()?;
    let ratio = ZswapProbe.compression_ratio(); // always >= 1.0, never fails
    let vram_kb = VramProbe.used_kb(); // always 0 on failure

    let compressed_ram_kb = (must_save_kb as f64 / ratio) as u64;
    let safety_kb = (compressed_ram_kb as f64 * SIZING_SAFETY_MARGIN) as u64;

    let total_kb = compressed_ram_kb
        .saturating_add(safety_kb)
        .saturating_add(vram_kb)
        .saturating_add(HIBERNATE_HEADER_OVERHEAD_KB);

    tracing::info!(
        must_save_kb,
        ratio,
        compressed_ram_kb,
        safety_kb,
        vram_kb,
        overhead_kb = HIBERNATE_HEADER_OVERHEAD_KB,
        total_kb,
        "swapfile size computed"
    );

    Ok(total_kb)
}
