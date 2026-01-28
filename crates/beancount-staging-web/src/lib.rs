mod api;
mod state;
mod static_files;
mod watcher;

use anyhow::Result;
use axum::{
    Router,
    routing::{get, post},
};
use beancount_staging::reconcile::StagingSource;
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
};
use tokio::{net::TcpListener, task::spawn_blocking};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

use state::{AppState, FileChangeEvent};
use watcher::FileWatcher;

// also change the clap default
pub const DEFAULT_PORT: u16 = 8472;

#[derive(Debug, Clone)]
pub enum ListenerType {
    Tcp(u16),
    UnixSocket(PathBuf),
}

pub async fn run(
    journal: Vec<PathBuf>,
    staging_source: StagingSource,
    listener_type: ListenerType,
) -> anyhow::Result<()> {
    let app = router(journal, staging_source)?;

    match listener_type {
        ListenerType::Tcp(port) => {
            let address = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
            let listener = try_listen_tcp(address).await?;
            tracing::info!("Server listening on http://{}", address);

            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
        #[cfg(unix)]
        ListenerType::UnixSocket(socket_path) => {
            let listener = try_listen_unix(&socket_path).await?;
            tracing::info!("Server listening on unix socket: {}", socket_path.display());

            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
        #[cfg(not(unix))]
        ListenerType::UnixSocket(_) => {
            anyhow::bail!("Unix sockets are not supported on this platform");
        }
    }

    Ok(())
}

pub fn router(journal: Vec<PathBuf>, staging_source: StagingSource) -> Result<Router> {
    // Initialize tracing if not already initialized
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "beancount_staging=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();

    // Initialize application state first
    let (file_change_tx, _rx) = tokio::sync::broadcast::channel(100);
    let state = AppState::new(journal, staging_source, file_change_tx.clone())?;

    spawn_blocking({
        let state = state.clone();
        move || {
            if let Err(e) = state.inner.lock().unwrap().retrain() {
                tracing::error!("Error trying to retrain: {e}");
            }
        }
    });

    let _watcher = {
        let state_ = state.lock().unwrap();
        let relevant_files = {
            state_
                .reconcile_state
                .journal_sourceset
                .iter()
                .chain(state_.reconcile_state.staging_sourceset.iter())
                .map(AsRef::as_ref)
        };
        let state_for_watcher = state.clone();
        FileWatcher::new(relevant_files, move || {
            if let Err(e) = state_for_watcher.reload() {
                tracing::error!("Failed to reload state: {}", e);
            } else {
                tracing::info!("State reloaded successfully");
            }

            // notify clients via SSE
            let subscriber_count = state_for_watcher.file_change_tx.receiver_count();
            match state_for_watcher.file_change_tx.send(FileChangeEvent) {
                Ok(_) => {
                    tracing::info!(
                        "Sent file change event to {} SSE clients",
                        subscriber_count - 1
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to send SSE event: {}", e);
                }
            }
        })?
    };

    // Build router with API routes first, then fallback to embedded static files
    let app = Router::new()
        .route("/api/init", get(api::init_handler))
        .route("/api/transaction/{index}", get(api::get_transaction))
        .route(
            "/api/transaction/{index}/commit",
            post(api::commit_transaction),
        )
        .route("/api/file-changes", get(api::file_changes_stream))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .fallback(static_files::static_handler);

    Ok(app)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn try_listen_tcp(address: SocketAddrV4) -> Result<TcpListener> {
    let listener = TcpListener::bind(address).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            anyhow::anyhow!(
                "Port {} is already in use. Please stop the existing server or choose a different port.",
                address.port()
            )
        } else {
            anyhow::Error::from(e)
        }
    })?;
    Ok(listener)
}

#[cfg(unix)]
async fn try_listen_unix(socket_path: &PathBuf) -> Result<tokio::net::UnixListener> {
    use std::fs;

    // Remove existing socket file if it exists
    if socket_path.exists() {
        fs::remove_file(socket_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to remove existing socket file at {}: {}",
                socket_path.display(),
                e
            )
        })?;
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = socket_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create directory for socket at {}: {}",
                parent.display(),
                e
            )
        })?;
    }

    let listener = tokio::net::UnixListener::bind(socket_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to bind to Unix socket at {}: {}",
            socket_path.display(),
            e
        )
    })?;

    Ok(listener)
}
