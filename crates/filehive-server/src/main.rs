mod admin;
mod chunk;
mod mcp;
mod middleware;
mod pages;
mod presign;
mod share;
mod state;
mod tasks;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{middleware::from_fn_with_state, Router};
use filehive_config::{ConfigManager, RateLimiter};
use filehive_db::init_pool;
use filehive_storage::FileStorage;
use tower_http::cors::CorsLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,filehive_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting FileHive (Rust edition)...");

    let db_path = std::env::var("DATABASE_URL").unwrap_or_else(|_| "data/filehive.db".to_string());
    if !db_path.contains("://") && db_path != ":memory:" {
        if let Some(parent) = std::path::Path::new(&db_path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
    }

    let pool = init_pool(&db_path).await?;
    info!("Database initialized: {}", db_path);

    let config_mgr = Arc::new(ConfigManager::new(pool.clone()));
    config_mgr.load_from_db().await?;

    let rate_limiter = Arc::new(RateLimiter::new(config_mgr.settings_arc()));

    let settings = config_mgr.get();
    let storage_cfg = state::storage_config_from_settings(&settings);
    let storage = FileStorage::new(&storage_cfg)?;

    let app_state = AppState {
        pool: pool.clone(),
        storage: storage.clone(),
        config: config_mgr.clone(),
        rate_limiter,
    };

    tasks::start_background_tasks(pool, storage).await;

    let app = Router::new()
        .nest("/share", share::share_router())
        .nest("/chunk", chunk::chunk_router())
        .nest("/presign", presign::presign_router())
        .nest("/api/presign", presign::presign_router())
        .nest("/admin", admin::admin_router())
        .merge(mcp::mcp_router())
        .merge(pages::pages_router())
        .layer(from_fn_with_state(
            app_state.clone(),
            middleware::setup_guard_middleware,
        ))
        .layer(CorsLayer::permissive())
        .fallback(pages::handlers::spa_fallback_handler)
        .with_state(app_state);

    let settings = config_mgr.get();
    let host = std::env::var("HOST").unwrap_or_else(|_| settings.server_host.clone());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(settings.server_port);

    let addr_str = format!("{}:{}", host, port);
    let addr: SocketAddr = addr_str.parse().unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 12345)));

    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("FileHive server shutdown gracefully.");
    Ok(())
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
            .expect("failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C (SIGINT), initiating graceful shutdown...");
        },
        _ = terminate => {
            info!("Received SIGTERM, initiating graceful shutdown...");
        },
    }
}
