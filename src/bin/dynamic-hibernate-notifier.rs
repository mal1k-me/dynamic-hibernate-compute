use dynamic_hibernate::dbus::notifier;
use tracing_subscriber::{prelude::*, EnvFilter};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    init_logging();

    loop {
        if let Err(e) = notifier::run().await {
            tracing::warn!(error = %e, "notifier cycle failed — retrying in 2 s");
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

fn init_logging() {
    tracing_subscriber::registry()
        .with(
            EnvFilter::from_env("DYNAMIC_HIBERNATE_LOG")
                .add_directive("info".parse().unwrap()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
}
