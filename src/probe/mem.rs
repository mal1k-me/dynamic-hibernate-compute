use crate::error::{ErrorKind, HibernateResult, IntoHibernateError};
use procfs::Current;
use procfs::Meminfo;

/// Probes `/proc/meminfo` for the non-reclaimable RAM footprint.
///
/// ## Formula
///
/// ```text
/// reclaimable  = Buffers + max(Cached − Dirty, 0) + SReclaimable
/// must_save_kb = MemTotal − MemFree − reclaimable
/// ```
///
/// Under `image_size=0` the kernel drops every reclaimable page before
/// taking the hibernation snapshot. What remains — `must_save_kb` — is
/// exactly what must be written to the swapfile. This implicitly captures:
///
/// - `AnonPages`    — anonymous memory with no file backing
/// - `Shmem`        — shared memory / tmpfs
/// - `SUnreclaim`   — non-reclaimable slab
/// - `PageTables`   — process page-table structures
/// - `KernelStack`  — per-thread kernel stacks
/// - `Unevictable`  — mlock'd pages (GPU driver buffers, etc.)
/// - `Percpu`       — per-CPU allocations (missed by additive formulas)
/// - Dirty file pages — cannot be dropped, must be saved
/// - Kernel runtime data not reflected in any single additive field
///
/// ## When to call this
///
/// For the tightest estimate, call at the s2idle→hibernate transition when
/// processes are frozen and the working set is completely static.
pub struct MemProbe;

impl MemProbe {
    /// Returns the non-reclaimable RAM footprint in kilobytes.
    pub fn must_save_kb(&self) -> HibernateResult<u64> {
        let info = Meminfo::current()
            .hibernate_err_ctx(ErrorKind::ProbeFailed, "failed to read /proc/meminfo")?;

        let total = info.mem_total;
        let free = info.mem_free;
        let buffers = info.buffers;
        let cached = info.cached;
        let dirty = info.dirty; // u64, always present
        let s_reclaimable = info.s_reclaimable.unwrap_or(0); // Option<u64>, kernel >= 2.6.19

        let clean_cache = cached.saturating_sub(dirty);
        let reclaimable = buffers + clean_cache + s_reclaimable;
        let must_save = total.saturating_sub(free).saturating_sub(reclaimable);

        tracing::debug!(
            total_kb = total,
            free_kb = free,
            buffers_kb = buffers,
            cached_kb = cached,
            dirty_kb = dirty,
            sreclaimable_kb = s_reclaimable,
            reclaimable_kb = reclaimable,
            must_save_kb = must_save,
            "meminfo probe complete"
        );

        Ok(must_save)
    }
}
