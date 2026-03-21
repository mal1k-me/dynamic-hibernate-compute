use crate::error::{ErrorKind, HibernateResult, IntoHibernateError};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapMetadata {
    pub swap_file: PathBuf,
    pub old_resume: String,
    pub old_resume_offset: String,
    pub created_at: i64,
}

impl SwapMetadata {
    pub fn new(swap_file: PathBuf, old_resume: String, old_resume_offset: String) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Self {
            swap_file,
            old_resume,
            old_resume_offset,
            created_at,
        }
    }

    /// Write to disk as JSON, mode `0o600`.
    pub fn write(&self, path: &Path) -> HibernateResult<()> {
        let json =
            serde_json::to_string_pretty(self).hibernate_err(ErrorKind::MetadataWriteFailed)?;

        fs::write(path, json)
            .hibernate_err_ctx(ErrorKind::MetadataWriteFailed, "write metadata")?;

        let mut perms = fs::metadata(path)
            .hibernate_err(ErrorKind::MetadataWriteFailed)?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms).hibernate_err(ErrorKind::MetadataWriteFailed)?;

        tracing::debug!(path = %path.display(), "metadata written");
        Ok(())
    }

    /// Read and deserialise from disk.
    pub fn read(path: &Path) -> HibernateResult<Self> {
        let f = File::open(path).hibernate_err_ctx(ErrorKind::CleanupFailed, "open metadata")?;
        serde_json::from_reader(f)
            .hibernate_err_ctx(ErrorKind::CleanupFailed, "parse metadata JSON")
    }
}
