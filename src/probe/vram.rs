use crate::config::TOOL_NVIDIA_SMI;
use std::process::Command;
use which::which;

/// Probes used NVIDIA VRAM via `nvidia-smi`.
///
/// ## Why VRAM is separate
///
/// The NVIDIA kernel driver saves in-use VRAM allocations to the hibernate
/// image alongside system RAM. This data goes through the **nvidia driver's
/// own save/restore path**, not through the kernel's LZO/LZ4 compressor.
/// It must therefore be added to the swapfile size **uncompressed** and
/// must not be divided by the zswap compression ratio.
///
/// ## Failure behaviour
///
/// Returns `0` on any failure:
/// - `nvidia-smi` not found in `PATH` (supergfxd integrated mode, D3cold)
/// - Non-zero exit (GPU suspended, no NVIDIA GPU present)
/// - Parse error
///
/// Zero is always safe: if the GPU is off, no VRAM is saved.
pub struct VramProbe;

impl VramProbe {
    /// Returns used VRAM in kilobytes, or `0` on any failure.
    pub fn used_kb(&self) -> u64 {
        // Fast path: tool not present means GPU is off or not NVIDIA.
        if which(TOOL_NVIDIA_SMI).is_err() {
            tracing::debug!("nvidia-smi not in PATH — VRAM contribution: 0 KB");
            return 0;
        }

        let output = Command::new(TOOL_NVIDIA_SMI)
            .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let raw = String::from_utf8_lossy(&out.stdout);
                let kb = raw
                    .lines()
                    .next()
                    .and_then(|l| l.trim().parse::<u64>().ok())
                    .map(|mib| mib * 1024)
                    .unwrap_or(0);

                tracing::debug!(vram_kb = kb, "VRAM probe");
                kb
            }
            Ok(out) => {
                // Non-zero: GPU is suspended or not initialised — safe to treat as 0.
                tracing::debug!(
                    exit_code = ?out.status.code(),
                    "nvidia-smi non-zero exit — VRAM: 0 KB"
                );
                0
            }
            Err(e) => {
                tracing::debug!(error = %e, "nvidia-smi exec failed — VRAM: 0 KB");
                0
            }
        }
    }
}
