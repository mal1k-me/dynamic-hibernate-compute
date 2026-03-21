use crate::config::{DBUS_INTERFACE, DBUS_SIGNAL_MEMBER};
use futures_util::StreamExt;
use zbus::{Connection, MatchRule};

/// Listen for `ErrorOccurred` signals on the system bus and forward them as
/// desktop notifications. Runs as an infinite async loop.
pub async fn run() -> anyhow::Result<()> {
    let conn = Connection::system().await?;

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(DBUS_INTERFACE)?
        .member(DBUS_SIGNAL_MEMBER)?
        .build();

    let mut stream = zbus::MessageStream::for_match_rule(rule, &conn, Some(32)).await?;

    tracing::info!("listening for hibernate error signals");

    loop {
        match stream.next().await {
            Some(Ok(msg)) => match msg.body().deserialize::<(u32, String)>() {
                Ok((kind, message)) => {
                    tracing::debug!(kind, message = %message, "signal received");
                    dispatch_notification(kind, &message).await;
                }
                Err(e) => tracing::warn!(error = %e, "failed to deserialise signal body"),
            },
            Some(Err(e)) => tracing::warn!(error = %e, "signal stream error"),
            None => {
                tracing::warn!("signal stream ended — reconnecting");
                break;
            }
        }
    }

    Ok(())
}

async fn dispatch_notification(kind: u32, message: &str) {
    let title = title_for_kind(kind);
    let body = if message.trim().is_empty() {
        "Check 'journalctl -u dynamic-hibernate-prepare' for details.".to_owned()
    } else {
        message.to_owned()
    };

    tracing::info!(title, body = %body, "sending desktop notification");

    let result = notify_rust::Notification::new()
        .summary(title)
        .body(&body)
        .icon("system-hibernate-symbolic")
        .urgency(notify_rust::Urgency::Critical)
        .show();

    if let Err(e) = result {
        tracing::warn!(error = %e, "failed to show desktop notification");
    }
}

fn title_for_kind(kind: u32) -> &'static str {
    match kind {
        0 => "Hibernate: Root Privileges Required",
        1 => "Hibernate: Insufficient Disk Space",
        2 => "Hibernate: Swapfile Activation Failed",
        3 => "Hibernate: Swapfile Creation Failed",
        4 => "Hibernate: Resume Configuration Failed",
        5 => "Hibernate: Metadata Write Failed",
        6 => "Hibernate: Cleanup Failed",
        7 => "Hibernate: Device Lookup Failed",
        8 => "Hibernate: Compressor Mismatch Detected",
        9 => "Hibernate: Storage Preparation Failed",
        10 => "Hibernate: Kernel Probe Failed",
        11 => "Hibernate: Required Tool Not Found",
        _ => "Hibernate: Unexpected Error",
    }
}
