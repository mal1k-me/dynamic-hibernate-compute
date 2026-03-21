use clap::{Parser, Subcommand};
use dynamic_hibernate::dbus::emitter::emit_error_signal_blocking;
use dynamic_hibernate::swap::SwapManager;
use nix::unistd::Uid;
use tracing_subscriber::{prelude::*, EnvFilter};

#[derive(Parser)]
#[command(name = "dynamic-hibernate", version, about = "Dynamic hibernation swap manager")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Prepare a swapfile and configure the kernel resume pointers.
    Create,
    /// Remove the swapfile and restore kernel state after resume.
    Cleanup,
    /// Print current hibernate state and size estimate.
    Status,
}

fn main() {
    init_logging();

    if let Err(e) = run() {
        emit_error_signal_blocking(&e.to_signal());
        tracing::error!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> dynamic_hibernate::HibernateResult<()> {
    use dynamic_hibernate::error::{ErrorKind, HibernateError};

    if !Uid::effective().is_root() {
        return Err(HibernateError::new(
            ErrorKind::RequiresRoot,
            anyhow::anyhow!("dynamic-hibernate must be run as root"),
        ));
    }

    let cli = Cli::parse();
    let manager = SwapManager::new();

    match cli.command {
        Cmd::Create => manager.create(),
        Cmd::Cleanup => manager.cleanup(),
        Cmd::Status => manager.status(),
    }
}

fn init_logging() {
    let journald = tracing_journald::layer().ok();
    let stderr = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);

    tracing_subscriber::registry()
        .with(
            EnvFilter::from_env("DYNAMIC_HIBERNATE_LOG").add_directive(
                if std::env::var("JOURNAL_STREAM").is_ok() {
                    "info".parse().unwrap()
                } else {
                    "debug".parse().unwrap()
                },
            ),
        )
        .with(journald)
        .with(stderr)
        .init();
}
