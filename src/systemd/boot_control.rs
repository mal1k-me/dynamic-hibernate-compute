use crate::config::TOOL_BOOTCTL;
use serde::Deserialize;
use std::process::{Command, Stdio};
use which::which;

/// Manages systemd-boot visibility around hibernate/resume cycles.
/// All methods are best-effort and non-fatal.
pub struct BootControl;

impl BootControl {
    pub fn hide_menu() {
        if !Self::is_present() {
            return;
        }
        Self::pin_current_entry();
        if let Err(e) = Self::run_bootctl(&["set-timeout", "0"]) {
            tracing::warn!("bootctl set-timeout 0 failed: {}", e);
        }
    }

    pub fn restore_menu() {
        if !Self::is_present() {
            return;
        }
        if let Err(e) = Self::run_bootctl(&["set-timeout", ""]) {
            tracing::warn!("bootctl set-timeout restore failed: {}", e);
        }
    }

    fn is_present() -> bool {
        if which(TOOL_BOOTCTL).is_err() {
            return false;
        }
        Command::new(TOOL_BOOTCTL)
            .arg("is-installed")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn pin_current_entry() {
        match Self::current_entry_id() {
            Some(id) => {
                if let Err(e) = Self::run_bootctl(&["set-oneshot", &id]) {
                    tracing::warn!("bootctl set-oneshot failed: {}", e);
                }
            }
            None => tracing::debug!("no selected boot entry found — skipping pin"),
        }
    }

    fn current_entry_id() -> Option<String> {
        #[derive(Deserialize)]
        struct Entry {
            id: Option<String>,
            #[serde(rename = "isSelected")]
            is_selected: Option<bool>,
        }

        let out = Command::new(TOOL_BOOTCTL)
            .args(["list", "--json=short", "--no-pager"])
            .output()
            .ok()?;

        if !out.status.success() {
            return None;
        }

        serde_json::from_slice::<Vec<Entry>>(&out.stdout)
            .ok()?
            .into_iter()
            .find(|e| e.is_selected.unwrap_or(false))
            .and_then(|e| e.id)
    }

    fn run_bootctl(args: &[&str]) -> anyhow::Result<()> {
        let status = Command::new(TOOL_BOOTCTL).args(args).status()?;

        anyhow::ensure!(
            status.success(),
            "bootctl {} exited with {}",
            args.join(" "),
            status
        );
        Ok(())
    }
}
