pub mod handlers;

use axum::{routing::post, Router};
use crate::state::AppState;

pub fn mcp_router() -> Router<AppState> {
    Router::new().route("/mcp", post(handlers::mcp_handler))
}
