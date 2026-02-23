mod app_manager;
mod accessibility;
mod keyboard;
mod mcp_server;

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use mcp_server::AppManagerServer;

const BIND_ADDRESS: &str = "127.0.0.1:8000";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cancellation_token = tokio_util::sync::CancellationToken::new();

    let service = StreamableHttpService::new(
        || Ok(AppManagerServer::new()),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token: cancellation_token.child_token(),
            ..Default::default()
        },
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;
    tracing::info!("MCP HTTP server listening on http://{}/mcp", BIND_ADDRESS);

    let _ = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.unwrap();
            cancellation_token.cancel();
        })
        .await;

    Ok(())
}
