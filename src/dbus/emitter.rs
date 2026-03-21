use crate::config::{DBUS_INTERFACE, DBUS_OBJECT_PATH, DBUS_SIGNAL_MEMBER};
use crate::error::ErrorSignal;

/// Emit an `ErrorOccurred` signal on the system D-Bus (best-effort, non-fatal).
pub async fn emit_error_signal(signal: &ErrorSignal) {
    if let Err(e) = try_emit(signal).await {
        tracing::warn!(error = %e, "D-Bus signal emission failed (non-fatal)");
    }
}

/// Synchronous wrapper for use in the main binary's error handler.
pub fn emit_error_signal_blocking(signal: &ErrorSignal) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();

    match rt {
        Ok(rt) => rt.block_on(emit_error_signal(signal)),
        Err(e) => tracing::warn!("tokio runtime creation failed: {}", e),
    }
}

async fn try_emit(signal: &ErrorSignal) -> anyhow::Result<()> {
    use zbus::Connection;

    let (kind, message) = signal.as_dbus_body();
    let conn = Connection::system().await?;

    let msg = zbus::message::Message::signal(DBUS_OBJECT_PATH, DBUS_INTERFACE, DBUS_SIGNAL_MEMBER)?
        .build(&(kind, message))?;

    conn.send(&msg).await?;

    // Brief flush window before the oneshot connection is dropped.
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    Ok(())
}
