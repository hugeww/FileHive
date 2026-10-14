pub mod handlers;

use axum::{
    routing::{get, post},
    Router,
};
use crate::state::AppState;

pub fn share_router() -> Router<AppState> {
    Router::new()
        .route("/text/", post(handlers::share_text_handler))
        .route("/file/", post(handlers::share_file_handler))
        .route(
            "/select/",
            get(handlers::select_get_handler).post(handlers::select_post_handler),
        )
        .route("/download", get(handlers::download_handler))
        .route(
            "/metadata/",
            get(handlers::metadata_get_handler).post(handlers::metadata_post_handler),
        )
}
