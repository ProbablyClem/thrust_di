use std::sync::Arc;

use axum::response::{IntoResponse, Json};
use serde_json::json;
use thrust_macros::{get, post};

use crate::services::TodoService;

#[get("/todos")]
pub async fn list_todos(svc: Arc<TodoService>) -> impl IntoResponse {
    Json(json!({ "todos": svc.find_all() }))
}

#[post("/todos")]
pub async fn create_todo(svc: Arc<TodoService>) -> impl IntoResponse {
    let _ = &svc;
    Json(json!({ "created": true }))
}
