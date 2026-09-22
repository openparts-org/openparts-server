use openparts_server::{routes, store};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let data_dir = std::env::var("OPENPARTS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../openparts-data"));
    tracing::info!(data_dir = %data_dir.display(), "loading openparts-data");
    let loaded_store = Arc::new(store::discover_and_load(&data_dir)?);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let app = routes::build_router(loaded_store);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "openparts-server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
