mod api;
mod test_manager;

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::api::{cancel_test, get_test_status, list_tests, start_test, test_sse_handler};
use crate::test_manager::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rperf3_web_ui=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create shared application state
    let state = Arc::new(AppState::new());

    // Build API routes
    let api_routes = Router::new()
        .route("/tests", post(start_test))
        .route("/tests", get(list_tests))
        .route("/tests/:test_id", get(get_test_status))
        .route("/tests/:test_id", delete(cancel_test))
        .route("/tests/:test_id/sse", get(test_sse_handler))
        .with_state(state);

    // Build main app
    let app = Router::new()
        .nest("/api", api_routes)
        .nest_service("/", ServeDir::new("frontend/dist"))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = "0.0.0.0:3000";
    info!("Starting rperf3 Web UI server on http://{}", addr);
    info!("API endpoints available at http://{}/api", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
