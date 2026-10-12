pub mod handlers;

use axum::{
    routing::{delete, get, post},
    Router,
};
use crate::state::AppState;

pub fn chunk_router() -> Router<AppState> {
    Router::new()
        .route("/upload/init/", post(handlers::chunk_init_handler))
        .route("/upload/", post(handlers::chunk_upload_handler))
        .route(
            "/upload/chunk/{upload_id}/{chunk_index}",
            post(handlers::chunk_upload_path_handler),
        )
        .route(
            "/upload/status/{upload_id}",
            get(handlers::chunk_status_handler),
        )
        .route("/upload/complete/", post(handlers::chunk_complete_handler))
        .route(
            "/upload/complete/{upload_id}",
            post(handlers::chunk_complete_path_handler),
        )
        .route("/upload/{upload_id}", delete(handlers::chunk_abort_handler))
}
