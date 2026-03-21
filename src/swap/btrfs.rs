use crate::config::TOOL_BTRFS;
use crate::error::{ErrorKind, HibernateResult, IntoHibernateError};
use crate::hibernate_err;
use nix::sys::statvfs::statvfs;
use std::fs;
use std::path::Path;
use std::process::Command;
use which::which;

/// Btrfs-specific operations required by the swapfile lifecycle.
pub struct BtrfsOps;

impl BtrfsOps {
    /// Verify `btrfs` tool is available.
    pub fn check_available() -> HibernateResult<()> {
        which(TOOL_BTRFS).map(|_| ()).hibernate_err_ctx(
            ErrorKind::ToolNotFound,
            "btrfs-progs not found in PATH — is btrfs-progs installed?",
        )
    }

    /// Ensure `target` is a btrfs subvolume.
    ///
    /// - Already a subvolume → noop.
    /// - Exists but not a subvolume → remove and create (refuses if subdirs present).
    /// - Doesn't exist → create subvolume.
    pub fn ensure_subvolume(target: &Path) -> HibernateResult<()> {
        if target.exists() {
            if Self::is_subvolume(target)? {
                return Ok(());
            }

            if target.is_dir() {
                let has_subdirs = fs::read_dir(target)
                    .hibernate_err(ErrorKind::SubvolumePreparationFailed)?
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().is_dir());

                if has_subdirs {
                    return Err(hibernate_err!(
                        ErrorKind::SubvolumePreparationFailed,
                        "refusing to remove non-empty non-subvolume dir: {}",
                        target.display()
                    ));
                }

                fs::remove_dir_all(target).hibernate_err(ErrorKind::SubvolumePreparationFailed)?;
            } else {
                fs::remove_file(target).hibernate_err(ErrorKind::SubvolumePreparationFailed)?;
            }
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).hibernate_err(ErrorKind::SubvolumePreparationFailed)?;
        }

        Self::create_subvolume(target)
    }

    /// Verify the filesystem has enough free space via `statvfs(2)`.
    pub fn check_free_space(dir: &Path, required_bytes: u64) -> HibernateResult<()> {
        let stat =
            statvfs(dir).hibernate_err_ctx(ErrorKind::DiskSpaceInsufficient, "statvfs failed")?;

        let available = stat.blocks_available() * stat.block_size();

        if available < required_bytes {
            return Err(hibernate_err!(
                ErrorKind::DiskSpaceInsufficient,
                "not enough space in {} — need {} KB, have {} KB",
                dir.display(),
                required_bytes / 1024,
                available / 1024
            ));
        }

        tracing::debug!(
            dir = %dir.display(),
            required_kb = required_bytes / 1024,
            available_kb = available / 1024,
            "disk space check passed"
        );

        Ok(())
    }

    /// Create a btrfs swapfile with `nodatacow`/`nodatasum`/`--uuid clear`.
    /// Requires btrfs-progs >= 6.1.
    pub fn mkswapfile(path: &Path, size_kb: u64) -> HibernateResult<()> {
        let size_mb = (size_kb / 1024) + 1;
        tracing::info!(path = %path.display(), size_mb, "creating btrfs swapfile");

        let out = Command::new(TOOL_BTRFS)
            .args([
                "filesystem",
                "mkswapfile",
                "--size",
                &format!("{size_mb}m"),
                "--uuid",
                "clear",
            ])
            .arg(path)
            .output()
            .hibernate_err_ctx(ErrorKind::SwapfileCreationFailed, "exec btrfs mkswapfile")?;

        if !out.status.success() {
            let _ = fs::remove_file(path);
            return Err(hibernate_err!(
                ErrorKind::SwapfileCreationFailed,
                "btrfs mkswapfile failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }

        Ok(())
    }

    // ── private ──────────────────────────────────────────────────────────────

    fn is_subvolume(path: &Path) -> HibernateResult<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let out = Command::new(TOOL_BTRFS)
            .args(["subvolume", "show"])
            .arg(path)
            .output()
            .hibernate_err(ErrorKind::SubvolumePreparationFailed)?;

        match out.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(hibernate_err!(
                ErrorKind::SubvolumePreparationFailed,
                "btrfs subvolume show unexpected exit: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )),
        }
    }

    fn create_subvolume(target: &Path) -> HibernateResult<()> {
        let out = Command::new(TOOL_BTRFS)
            .args(["subvolume", "create"])
            .arg(target)
            .output()
            .hibernate_err(ErrorKind::SubvolumePreparationFailed)?;

        if !out.status.success() {
            return Err(hibernate_err!(
                ErrorKind::SubvolumePreparationFailed,
                "btrfs subvolume create failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }

        tracing::debug!(path = %target.display(), "btrfs subvolume created");
        Ok(())
    }
}
