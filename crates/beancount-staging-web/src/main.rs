mod api;
mod state;
mod static_files;

use axum::{
    Router,
    routing::{get, post},
};
use clap::Parser;
use std::path::PathBuf;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

use state::AppState;

#[derive(Parser)]
#[command(name = "beancount-staging-web")]
#[command(about = "Web server for interactive beancount staging")]
struct Args {
    /// Journal file paths
    #[arg(short, long, required = true)]
    journal: Vec<PathBuf>,

    /// Staging file paths
    #[arg(short, long, required = true)]
    staging: Vec<PathBuf>,

    /// Port to listen on
    #[arg(short, long, default_value = "8472")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "beancount_staging_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    // Initialize application state
    let state = AppState::new(args.journal, args.staging)?;

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
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", args.port)).await?;
    tracing::info!("Server listening on http://127.0.0.1:{}", args.port);

    axum::serve(listener, app).await?;

    Ok(())
}
