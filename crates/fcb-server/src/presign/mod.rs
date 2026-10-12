pub mod handlers;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use crate::state::AppState;

pub fn presign_router() -> Router<AppState> {
    Router::new()
        .route("/upload/init", post(handlers::presign_init_handler))
        .route(
            "/upload/proxy/{upload_id}",
            put(handlers::presign_proxy_upload_handler),
        )
        .route(
            "/upload/confirm/{upload_id}",
            post(handlers::presign_confirm_handler),
        )
        .route(
            "/upload/status/{upload_id}",
            get(handlers::presign_status_handler),
        )
        .route("/upload/{upload_id}", delete(handlers::presign_cancel_handler))
}
