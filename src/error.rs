use std::fmt;

// ── Error categories (transmitted over D-Bus as u32) ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorKind {
    RequiresRoot = 0,
    DiskSpaceInsufficient = 1,
    SwapfileActivationFailed = 2,
    SwapfileCreationFailed = 3,
    ResumeConfigFailed = 4,
    MetadataWriteFailed = 5,
    CleanupFailed = 6,
    DeviceLookupFailed = 7,
    CompressorMismatch = 8,
    SubvolumePreparationFailed = 9,
    ProbeFailed = 10,
    ToolNotFound = 11,
    Unknown = 12,
}

impl ErrorKind {
    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

// ── Primary error type ────────────────────────────────────────────────────────

/// A hibernate-domain error carrying a typed category and an anyhow chain.
#[derive(Debug)]
pub struct HibernateError {
    pub kind: ErrorKind,
    pub inner: anyhow::Error,
}

impl HibernateError {
    pub fn new(kind: ErrorKind, inner: impl Into<anyhow::Error>) -> Self {
        Self {
            kind,
            inner: inner.into(),
        }
    }

    /// Convert into the payload that gets broadcast over D-Bus.
    pub fn to_signal(&self) -> ErrorSignal {
        ErrorSignal {
            kind: self.kind,
            message: self.inner.to_string(),
        }
    }
}

impl fmt::Display for HibernateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.kind, self.inner)
    }
}

impl std::error::Error for HibernateError {}

// ── D-Bus signal payload ──────────────────────────────────────────────────────

/// Serializable payload sent to the user-session notifier via D-Bus.
#[derive(Debug, Clone)]
pub struct ErrorSignal {
    pub kind: ErrorKind,
    pub message: String,
}

impl ErrorSignal {
    /// Returns `(kind_code, message_str)` ready to be written as a D-Bus body.
    pub fn as_dbus_body(&self) -> (u32, &str) {
        (self.kind.as_u32(), self.message.as_str())
    }
}

// ── Convenience ──────────────────────────────────────────────────────────────

pub type HibernateResult<T> = Result<T, HibernateError>;

#[macro_export]
macro_rules! hibernate_err {
    ($kind:expr, $($arg:tt)*) => {
        $crate::error::HibernateError::new($kind, anyhow::anyhow!($($arg)*))
    };
}

/// Extension trait to attach an `ErrorKind` to any `Result<T, E: Into<anyhow::Error>>`.
pub trait IntoHibernateError<T> {
    fn hibernate_err(self, kind: ErrorKind) -> HibernateResult<T>;
    fn hibernate_err_ctx(self, kind: ErrorKind, ctx: &str) -> HibernateResult<T>;
}

impl<T, E: Into<anyhow::Error>> IntoHibernateError<T> for Result<T, E> {
    fn hibernate_err(self, kind: ErrorKind) -> HibernateResult<T> {
        self.map_err(|e| HibernateError::new(kind, e))
    }

    fn hibernate_err_ctx(self, kind: ErrorKind, ctx: &str) -> HibernateResult<T> {
        self.map_err(|e| {
            let enriched: anyhow::Error = e.into().context(ctx.to_owned());
            HibernateError::new(kind, enriched)
        })
    }
}
