use crate::config::{METADATA_FILE_NAME, SWAP_DIR, SWAP_FILE_NAME, TOOL_SWAPOFF, TOOL_SWAPON};
use crate::error::{ErrorKind, HibernateResult, IntoHibernateError};
use crate::hibernate_err;
use crate::power::{ImageSize, ResumeParams};
use crate::probe::{SwapProbe, ZswapProbe};
use crate::sizing;
use crate::swap::btrfs::BtrfsOps;
use crate::swap::SwapMetadata;
use crate::systemd::BootControl;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io::Write};
use which::which;

/// Orchestrates the full swapfile lifecycle.
pub struct SwapManager {
    swap_dir: PathBuf,
    swap_file: PathBuf,
    meta_file: PathBuf,
}

impl Default for SwapManager {
    fn default() -> Self {
        let dir = PathBuf::from(SWAP_DIR);
        Self {
            swap_file: dir.join(SWAP_FILE_NAME),
            meta_file: dir.join(METADATA_FILE_NAME),
            swap_dir: dir,
        }
    }
}

impl SwapManager {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Public commands ───────────────────────────────────────────────────────

    /// Prepare a swapfile and configure the kernel resume pointers.
    /// Called by `dynamic-hibernate-prepare.service` before hibernation.
    pub fn create(&self) -> HibernateResult<()> {
        // ── 1. Pre-flight ─────────────────────────────────────────────────────
        self.preflight()?;

        // ── 2. Save current resume params ─────────────────────────────────────
        let old_resume = ResumeParams::read_resume().unwrap_or_else(|_| "0:0".into());
        let old_resume_offset = ResumeParams::read_resume_offset().unwrap_or_else(|_| "0".into());

        // ── 3. Zero image_size ────────────────────────────────────────────────
        ImageSize::set_zero()?;

        // ── 4. Compute required size ──────────────────────────────────────────
        let required_kb = sizing::compute_swapfile_kb()?;
        tracing::info!(required_kb, "swapfile target size");

        // ── 5. Reuse existing swap if sufficient ──────────────────────────────
        let swap_probe = SwapProbe;
        if swap_probe.largest_free_kb()? >= required_kb {
            if let Some(path) = swap_probe.largest_free_path()? {
                tracing::info!(path, "existing swap is sufficient — reusing");
                ResumeParams::configure_for_swapfile(Path::new(&path))?;
                return Ok(());
            }
        }

        // ── 6. Create swapfile ────────────────────────────────────────────────
        BtrfsOps::ensure_subvolume(&self.swap_dir)?;
        BtrfsOps::check_free_space(&self.swap_dir, required_kb * 1024)?;

        if self.swap_file.exists() {
            let _ = swapoff(&self.swap_file);
            fs::remove_file(&self.swap_file).hibernate_err(ErrorKind::SwapfileCreationFailed)?;
        }

        BtrfsOps::mkswapfile(&self.swap_file, required_kb)?;

        swapon(&self.swap_file).inspect_err(|_| {
            let _ = fs::remove_file(&self.swap_file);
        })?;

        // ── 7. Configure resume ───────────────────────────────────────────────
        ResumeParams::configure_for_swapfile(&self.swap_file).inspect_err(|_| {
            let _ = swapoff(&self.swap_file);
            let _ = fs::remove_file(&self.swap_file);
        })?;

        // ── 8. Persist metadata ───────────────────────────────────────────────
        SwapMetadata::new(self.swap_file.clone(), old_resume, old_resume_offset)
            .write(&self.meta_file)?;

        // ── 9. Hide bootloader menu ────────────────────────────────────────────
        BootControl::hide_menu();

        tracing::info!("hibernate swap ready");
        Ok(())
    }

    /// Remove the swapfile and restore kernel state after resume.
    /// Called by `dynamic-hibernate-cleanup.service`.
    pub fn cleanup(&self) -> HibernateResult<()> {
        if !self.meta_file.exists() {
            tracing::info!("no metadata found — nothing to clean up");
            return Ok(());
        }

        let meta = SwapMetadata::read(&self.meta_file)?;

        if meta.swap_file.exists() {
            let _ = swapoff(&meta.swap_file);
            fs::remove_file(&meta.swap_file)
                .hibernate_err_ctx(ErrorKind::CleanupFailed, "remove swapfile")?;
            tracing::info!(path = %meta.swap_file.display(), "swapfile removed");
        }

        ResumeParams::restore(&meta.old_resume, &meta.old_resume_offset)?;
        ImageSize::set_zero()?;

        fs::remove_file(&self.meta_file)
            .hibernate_err_ctx(ErrorKind::CleanupFailed, "remove metadata")?;

        BootControl::restore_menu();
        tracing::info!("cleanup complete");
        Ok(())
    }

    /// Print current hibernate state and size estimate.
    pub fn status(&self) -> HibernateResult<()> {
        let mut out = std::io::stdout().lock();

        writeln!(out, "=== dynamic-hibernate status ===").ok();

        let entries = SwapProbe.entries()?;
        if entries.is_empty() {
            writeln!(out, "Active swap : none").ok();
        } else {
            for e in &entries {
                writeln!(
                    out,
                    "Swap        : {} — {}/{} KB free",
                    e.path,
                    e.free_kb(),
                    e.size_kb
                )
                    .ok();
            }
        }

        if self.meta_file.exists() {
            match SwapMetadata::read(&self.meta_file) {
                Ok(m) => {
                    writeln!(out, "Swapfile    : {}", m.swap_file.display()).ok();
                    writeln!(out, "Created at  : {} (unix)", m.created_at).ok();
                    writeln!(
                        out,
                        "Old resume  : {} @ {}",
                        m.old_resume, m.old_resume_offset
                    )
                        .ok();
                }
                Err(e) => {
                    writeln!(out, "Metadata    : corrupt — {}", e).ok();
                }
            };
        } else {
            writeln!(out, "Metadata    : none").ok();
        }

        match sizing::compute_swapfile_kb() {
            Ok(kb) => writeln!(
                out,
                "Estimate    : {} KB  ({:.1} MB)",
                kb,
                kb as f64 / 1024.0
            )
                .ok(),
            Err(e) => writeln!(out, "Estimate    : unavailable — {}", e).ok(),
        };

        let z = ZswapProbe.zswap_compressor().unwrap_or_else(|| "?".into());
        let h = ZswapProbe
            .hibernate_compressor()
            .unwrap_or_else(|| "?".into());
        let aligned = if z == h {
            "✓ aligned"
        } else {
            "✗ MISMATCH — ratio unusable"
        };
        writeln!(out, "Compressors : zswap={z}  hibernate={h}  {aligned}").ok();

        Ok(())
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn preflight(&self) -> HibernateResult<()> {
        for tool in &[TOOL_SWAPON, TOOL_SWAPOFF] {
            which(tool).map(|_| ()).hibernate_err_ctx(
                ErrorKind::ToolNotFound,
                &format!("{tool} not found in PATH"),
            )?;
        }
        BtrfsOps::check_available()?;

        if !ZswapProbe.compressors_aligned() {
            tracing::warn!(
                "zswap and hibernate compressors differ — swapfile will use \
                 uncompressed size estimate (safe, but larger)"
            );
        }

        Ok(())
    }
}

// ── swapon / swapoff wrappers ─────────────────────────────────────────────────

fn swapon(path: &Path) -> HibernateResult<()> {
    let status = Command::new(TOOL_SWAPON)
        .arg(path)
        .status()
        .hibernate_err_ctx(ErrorKind::SwapfileActivationFailed, "exec swapon")?;

    if !status.success() {
        return Err(hibernate_err!(
            ErrorKind::SwapfileActivationFailed,
            "swapon {} exited with {}",
            path.display(),
            status
        ));
    }
    tracing::debug!(path = %path.display(), "swapon ok");
    Ok(())
}

fn swapoff(path: &Path) -> HibernateResult<()> {
    match Command::new(TOOL_SWAPOFF).arg(path).status() {
        Ok(s) if s.success() => {
            tracing::debug!(path = %path.display(), "swapoff ok");
        }
        Ok(s) => tracing::warn!(path = %path.display(), "swapoff non-zero: {}", s),
        Err(e) => tracing::warn!(path = %path.display(), "swapoff exec failed: {}", e),
    }
    Ok(())
}
