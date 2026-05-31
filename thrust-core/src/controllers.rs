use std::sync::Arc;

use axum::response::{IntoResponse, Json};
use serde_json::json;
use thrust_macros::{get, post};

use crate::services::{EmailService, UserService};

#[get("/users")]
pub async fn list_users(user_svc: Arc<UserService>) -> impl IntoResponse {
    Json(json!({ "users": [], "msg": user_svc.hello() }))
}

#[post("/users")]
pub async fn create_user(
    user_svc: Arc<UserService>,
    email_svc: Arc<EmailService>,
) -> impl IntoResponse {
    let _ = &email_svc;
    Json(json!({ "created": true, "msg": user_svc.hello() }))
}
