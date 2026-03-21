use crate::config::{
    HIBERNATE_COMPRESSOR_PATH, MIN_ZSWAP_SAMPLE_PAGES, ZSWAP_COMPRESSOR_PATH,
    ZSWAP_POOL_TOTAL_SIZE, ZSWAP_STORED_PAGES,
};
use std::fs;

/// Probes the live zswap debugfs pool for a real compression ratio.
///
/// ## Strategy
///
/// zswap compresses anonymous pages evicted under memory pressure in real time,
/// using the same kernel crypto infrastructure as hibernate. If both zswap and
/// hibernate are configured to use the **same compressor** (e.g. `lzo`), the
/// observed pool ratio is a real sample of how well your current memory content
/// compresses — not a theoretical estimate.
///
/// ## Fallback behaviour
///
/// Returns `1.0` (no compression benefit assumed, full uncompressed size) when:
/// - Compressors are misaligned (zswap ≠ hibernate)
/// - Pool sample is too small (< 200 MB — not representative)
/// - debugfs paths are unreadable (not root, or zswap inactive)
/// - Any numeric parse failure
pub struct ZswapProbe;

impl ZswapProbe {
    /// Returns the live compression ratio (uncompressed / compressed).
    /// Always >= 1.0. Returns 1.0 on any failure or misalignment.
    pub fn compression_ratio(&self) -> f64 {
        if !self.compressors_aligned() {
            return 1.0;
        }

        let stored = match self.stored_pages() {
            Some(v) if v >= MIN_ZSWAP_SAMPLE_PAGES => v,
            Some(v) => {
                tracing::debug!(
                    stored_pages = v,
                    min = MIN_ZSWAP_SAMPLE_PAGES,
                    "zswap pool too sparse — ratio fallback to 1.0"
                );
                return 1.0;
            }
            None => return 1.0,
        };

        let pool = match self.pool_bytes() {
            Some(v) if v > 0 => v,
            _ => return 1.0,
        };

        let ratio = (stored as f64 * 4096.0) / pool as f64;

        tracing::debug!(
            stored_pages = stored,
            pool_bytes = pool,
            ratio,
            "zswap ratio sampled"
        );

        ratio.max(1.0)
    }

    /// Returns `true` only when both compressors are readable and identical.
    pub fn compressors_aligned(&self) -> bool {
        match (self.zswap_compressor(), self.hibernate_compressor()) {
            (Some(z), Some(h)) => {
                let ok = z == h;
                if !ok {
                    tracing::warn!(
                        zswap = %z,
                        hibernate = %h,
                        "compressor mismatch — zswap ratio unusable for hibernate sizing"
                    );
                }
                ok
            }
            _ => {
                tracing::debug!("compressor paths unreadable — assuming misaligned");
                false
            }
        }
    }

    pub fn zswap_compressor(&self) -> Option<String> {
        Self::read_trimmed(ZSWAP_COMPRESSOR_PATH)
    }

    pub fn hibernate_compressor(&self) -> Option<String> {
        Self::read_trimmed(HIBERNATE_COMPRESSOR_PATH)
    }

    fn stored_pages(&self) -> Option<u64> {
        Self::read_u64(ZSWAP_STORED_PAGES)
    }

    fn pool_bytes(&self) -> Option<u64> {
        Self::read_u64(ZSWAP_POOL_TOTAL_SIZE)
    }

    fn read_u64(path: &str) -> Option<u64> {
        fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    fn read_trimmed(path: &str) -> Option<String> {
        Some(fs::read_to_string(path).ok()?.trim().to_lowercase())
    }
}
