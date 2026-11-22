#![forbid(unsafe_code)]

use shrug::{config::Config, logging, routes};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init();
    let cfg = Config::from_env();

    let state = routes::AppState {
        ready: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        max_body_bytes: cfg.max_body_bytes,
    };

    tracing::info!(bind_addr = %cfg.bind_addr, "shrug listening (no model, no auth, no database)");
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    axum::serve(listener, routes::router(state)).await?;
    Ok(())
}
