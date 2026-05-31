use std::sync::Arc;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use thrust_macros::get;

use crate::services::UserService;

#[get("/users")]
pub async fn list_users(svc: Arc<UserService>) -> impl IntoResponse {
    Json(json!({ "users": svc.list_users() }))
}
