use crate::error::{ErrorKind, HibernateResult, IntoHibernateError};
use std::fs::File;
use std::io::{BufRead, BufReader};

// ── Data types ────────────────────────────────────────────────────────────────

/// A single entry from `/proc/swaps` (all sizes in kilobytes).
#[derive(Debug, Clone)]
pub struct SwapEntry {
    pub path: String,
    pub size_kb: u64,
    pub used_kb: u64,
}

impl SwapEntry {
    pub fn free_kb(&self) -> u64 {
        self.size_kb.saturating_sub(self.used_kb)
    }

    pub fn is_zram(&self) -> bool {
        self.path.contains("/dev/zram")
    }
}

// ── Probe ─────────────────────────────────────────────────────────────────────

/// Parses `/proc/swaps`, filtering out zram devices.
pub struct SwapProbe;

impl SwapProbe {
    /// All active non-zram swap entries.
    pub fn entries(&self) -> HibernateResult<Vec<SwapEntry>> {
        let f = File::open("/proc/swaps")
            .hibernate_err_ctx(ErrorKind::ProbeFailed, "cannot open /proc/swaps")?;

        let entries = BufReader::new(f)
            .lines()
            .skip(1) // skip header: "Filename Type Size Used Priority"
            .filter_map(|l| l.ok())
            .filter_map(parse_swap_line)
            .filter(|e| !e.is_zram())
            .collect();

        Ok(entries)
    }

    /// Kilobytes of free space in the largest non-zram swap entry.
    pub fn largest_free_kb(&self) -> HibernateResult<u64> {
        Ok(self
            .entries()?
            .iter()
            .map(SwapEntry::free_kb)
            .max()
            .unwrap_or(0))
    }

    /// Path of the non-zram swap entry with the most free space.
    pub fn largest_free_path(&self) -> HibernateResult<Option<String>> {
        Ok(self
            .entries()?
            .into_iter()
            .max_by_key(SwapEntry::free_kb)
            .map(|e| e.path))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_swap_line(line: String) -> Option<SwapEntry> {
    // /proc/swaps columns: Filename  Type  Size  Used  Priority
    let mut cols = line.split_whitespace();
    let path = cols.next()?.to_owned();
    let _type = cols.next()?; // skip
    let size_kb = cols.next()?.parse().ok()?;
    let used_kb = cols.next()?.parse().ok()?;
    Some(SwapEntry {
        path,
        size_kb,
        used_kb,
    })
}
