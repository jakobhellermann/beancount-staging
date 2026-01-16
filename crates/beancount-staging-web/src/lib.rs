mod api;
mod state;
mod static_files;
mod watcher;

use axum::{
    Router,
    routing::{get, post},
};
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

use state::{AppState, FileChangeEvent};
use watcher::FileWatcher;

pub async fn run(journal: Vec<PathBuf>, staging: Vec<PathBuf>, port: u16) -> anyhow::Result<()> {
    // Initialize tracing if not already initialized
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "beancount_staging_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();

    // Initialize application state first
    let (file_change_tx, _rx) = tokio::sync::broadcast::channel(100);
    let state = AppState::new(journal.clone(), staging.clone(), file_change_tx.clone())?;

    let _watcher = {
        let state_ = state.inner.lock().unwrap();
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

    // Start server
    let listen = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    let listener = tokio::net::TcpListener::bind(listen).await?;
    tracing::info!("Server listening on http://{}", listen);

    axum::serve(listener, app).await?;

    Ok(())
}
