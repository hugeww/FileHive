pub mod auth;
pub mod handlers;

use axum::{
    routing::{delete, get, patch, post},
    Router,
};
use crate::state::AppState;

pub fn admin_router() -> Router<AppState> {
    Router::new()
        .route("/login", post(handlers::login_handler))
        .route("/verify", get(handlers::verify_handler))
        .route("/logout", post(handlers::logout_handler))
        .route("/dashboard", get(handlers::dashboard_handler))
        .route("/file/list", get(handlers::file_list_handler))
        .route(
            "/file/detail",
            get(handlers::file_detail_handler).post(handlers::file_detail_handler),
        )
        .route(
            "/file/policy-action",
            post(handlers::file_policy_action_handler).patch(handlers::file_policy_action_handler),
        )
        .route(
            "/file/batch-policy-action",
            post(handlers::file_batch_policy_action_handler)
                .patch(handlers::file_batch_policy_action_handler),
        )
        .route(
            "/file/metadata",
            post(handlers::file_metadata_handler).patch(handlers::file_metadata_handler),
        )
        .route("/file/delete", delete(handlers::delete_file_handler))
        .route(
            "/file/batch-delete",
            delete(handlers::batch_delete_handler).post(handlers::batch_delete_handler),
        )
        .route(
            "/file/view-presets",
            get(handlers::file_view_presets_get_handler)
                .post(handlers::file_view_presets_save_handler)
                .patch(handlers::file_view_presets_save_handler)
                .delete(handlers::file_view_presets_delete_handler),
        )
        .route(
            "/file/view-presets/delete",
            post(handlers::file_view_presets_delete_handler),
        )
        .route("/config/get", get(handlers::config_get_handler))
        .route("/config/update", patch(handlers::config_update_handler))
        .route("/local/lists", get(handlers::local_lists_handler))
        .route("/local/share", post(handlers::local_share_handler))
        .route(
            "/local/delete",
            delete(handlers::local_delete_handler).post(handlers::local_delete_handler),
        )
}
