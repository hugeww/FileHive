pub mod handlers;
pub mod setup;

use axum::{
    routing::get,
    Router,
};
use crate::state::AppState;

pub fn pages_router() -> Router<AppState> {
    Router::new()
        .route("/setup", get(setup::setup_get_handler).post(setup::setup_post_handler))
        .route("/setup/", get(setup::setup_get_handler).post(setup::setup_post_handler))
        .route("/", get(handlers::index_handler).post(handlers::post_config_handler))
        .route("/api/v1/config", get(handlers::get_public_config_handler))
        .route("/health", get(handlers::health_handler))
        .route("/robots.txt", get(handlers::robots_handler))
        .route("/assets/{*asset_path}", get(handlers::theme_asset_handler))
}
