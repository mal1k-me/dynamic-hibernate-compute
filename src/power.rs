use crate::config::{SYS_IMAGE_SIZE, SYS_RESUME, SYS_RESUME_OFFSET};
use crate::error::{ErrorKind, HibernateResult, IntoHibernateError};
use crate::hibernate_err;
use std::path::Path;
use std::{fs, process::Command};

// ── Resume parameters ─────────────────────────────────────────────────────────

pub struct ResumeParams;

impl ResumeParams {
    pub fn read_resume() -> HibernateResult<String> {
        sysfs_read(SYS_RESUME, ErrorKind::ResumeConfigFailed)
    }

    pub fn read_resume_offset() -> HibernateResult<String> {
        sysfs_read(SYS_RESUME_OFFSET, ErrorKind::ResumeConfigFailed)
    }

    /// Compute resume parameters from a swapfile, then write them atomically.
    /// Rolls back `/sys/power/resume` if writing the offset fails.
    pub fn configure_for_swapfile(swap_file: &Path) -> HibernateResult<()> {
        let device = find_backing_device(swap_file)?;
        let offset = btrfs_swap_offset(swap_file)?;

        fs::write(SYS_RESUME, &device)
            .hibernate_err_ctx(ErrorKind::ResumeConfigFailed, "write /sys/power/resume")?;

        if let Err(e) = fs::write(SYS_RESUME_OFFSET, &offset).hibernate_err_ctx(
            ErrorKind::ResumeConfigFailed,
            "write /sys/power/resume_offset",
        ) {
            if let Err(rb) = fs::write(SYS_RESUME, "0:0") {
                tracing::warn!("resume rollback failed: {}", rb);
            }
            return Err(e);
        }

        tracing::info!(device, offset, "resume parameters written");
        Ok(())
    }

    pub fn restore(resume: &str, resume_offset: &str) -> HibernateResult<()> {
        fs::write(SYS_RESUME, resume)
            .hibernate_err_ctx(ErrorKind::CleanupFailed, "restore /sys/power/resume")?;
        fs::write(SYS_RESUME_OFFSET, resume_offset)
            .hibernate_err_ctx(ErrorKind::CleanupFailed, "restore /sys/power/resume_offset")?;
        tracing::info!(resume, resume_offset, "resume parameters restored");
        Ok(())
    }

    pub fn clear() -> HibernateResult<()> {
        Self::restore("0:0", "0")
    }
}

// ── Image size ────────────────────────────────────────────────────────────────

pub struct ImageSize;

impl ImageSize {
    /// Set to `0`: instructs the kernel to drop every reclaimable page before
    /// taking the hibernation snapshot.
    pub fn set_zero() -> HibernateResult<()> {
        fs::write(SYS_IMAGE_SIZE, "0\n")
            .hibernate_err_ctx(ErrorKind::ResumeConfigFailed, "write /sys/power/image_size")
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn find_backing_device(target: &Path) -> HibernateResult<String> {
    let abs = target
        .canonicalize()
        .hibernate_err_ctx(ErrorKind::DeviceLookupFailed, "canonicalize swap path")?;

    let mounts = fs::read_to_string("/proc/mounts")
        .hibernate_err_ctx(ErrorKind::DeviceLookupFailed, "read /proc/mounts")?;

    let best = mounts
        .lines()
        .filter_map(parse_mount_line)
        .filter(|(_, mp)| abs.starts_with(mp.as_str()))
        .max_by_key(|(_, mp)| mp.len())
        .map(|(src, _)| src);

    let source = best.ok_or_else(|| {
        hibernate_err!(
            ErrorKind::DeviceLookupFailed,
            "no mount covers {}",
            abs.display()
        )
    })?;

    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(&source)
        .hibernate_err_ctx(ErrorKind::DeviceLookupFailed, "stat backing device")?;

    let rdev = meta.rdev();
    let major = ((rdev >> 8) & 0xfff) | ((rdev >> 32) & !0xfff_u64);
    let minor = (rdev & 0xff) | ((rdev >> 12) & !0xff_u64);

    Ok(format!("{major}:{minor}"))
}

fn btrfs_swap_offset(swap_file: &Path) -> HibernateResult<String> {
    let out = Command::new("btrfs")
        .args(["inspect-internal", "map-swapfile", "-r"])
        .arg(swap_file)
        .output()
        .hibernate_err_ctx(ErrorKind::ResumeConfigFailed, "exec btrfs map-swapfile")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(hibernate_err!(
            ErrorKind::ResumeConfigFailed,
            "btrfs map-swapfile failed: {}",
            stderr.trim()
        ));
    }

    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn parse_mount_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.split_whitespace();
    let source = parts.next()?.to_owned();
    let mp_raw = parts.next()?;
    Some((source, decode_octal_escapes(mp_raw)))
}

fn decode_octal_escapes(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2].is_ascii_digit()
            && bytes[i + 3].is_ascii_digit()
        {
            if let Ok(s) = std::str::from_utf8(&bytes[i + 1..i + 4]) {
                if let Ok(val) = u8::from_str_radix(s, 8) {
                    out.push(val);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

fn sysfs_read(path: &str, kind: ErrorKind) -> HibernateResult<String> {
    fs::read_to_string(path)
        .map(|s| s.trim().to_owned())
        .hibernate_err_ctx(kind, path)
}
