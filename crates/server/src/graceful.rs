use tokio_util::sync::CancellationToken;
use tracing::info;

/// Wait for a shutdown signal (Ctrl+C or SIGTERM) and cancel the token.
pub async fn wait_for_shutdown_signal(shutdown: CancellationToken) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("received Ctrl+C, initiating shutdown");
        }
        _ = terminate => {
            info!("received SIGTERM, initiating shutdown");
        }
    }

    shutdown.cancel();
}
