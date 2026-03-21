//! Kernel-API probes.
//!
//! Each submodule provides a zero-size unit struct whose methods read a
//! specific kernel interface. All methods are infallible by design where
//! the absence of data has a safe default (e.g., VRAM returns 0 when the GPU
//! is off) and fallible otherwise (e.g., MemProbe returns an error when
//! /proc/meminfo is unreadable, which is always a hard fault).

pub mod mem;
pub mod swap;
pub mod vram;
pub mod zswap;

pub use mem::MemProbe;
pub use swap::{SwapEntry, SwapProbe};
pub use vram::VramProbe;
pub use zswap::ZswapProbe;
