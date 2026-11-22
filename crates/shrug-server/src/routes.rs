use axum::extract::DefaultBodyLimit;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub ready: Arc<AtomicBool>,
    pub max_body_bytes: usize,
}

impl AppState {
    /// A ready server with a 256 KiB body limit.
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(true)),
            max_body_bytes: 256 * 1024,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn router(state: AppState) -> Router {
    let limit = state.max_body_bytes;
    Router::new()
        .route("/livez", get(livez))
        .route("/readyz", get(readyz))
        .route("/v1/decisions", post(crate::handlers::decisions))
        .route("/v1/systemone", post(crate::handlers::systemone))
        .layer(DefaultBodyLimit::max(limit))
        .with_state(state)
}

async fn livez() -> StatusCode {
    StatusCode::OK
}

async fn readyz(State(s): State<AppState>) -> StatusCode {
    if s.ready.load(Ordering::SeqCst) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
