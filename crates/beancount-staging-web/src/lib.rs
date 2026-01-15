mod api;
mod state;
mod static_files;

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

use state::AppState;

pub async fn run(journal: Vec<PathBuf>, staging: Vec<PathBuf>, port: u16) -> anyhow::Result<()> {
    // Initialize tracing if not already initialized
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "beancount_staging_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();

    // Initialize application state
    let state = AppState::new(journal, staging)?;

    // Build router with API routes first, then fallback to embedded static files
    let app = Router::new()
        .route("/api/init", get(api::init_handler))
        .route("/api/transaction/{index}", get(api::get_transaction))
        .route("/api/transaction/{index}/account", post(api::save_account))
        .route(
            "/api/transaction/{index}/commit",
            post(api::commit_transaction),
        )
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
