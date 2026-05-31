use std::sync::Arc;
use thrust_macros::{bean, layer};
use tower_http::classify::{ServerErrorsAsFailures, SharedClassifier};
use tower_http::trace::TraceLayer;

use crate::db::DbPool;

#[bean]
pub fn db_pool() -> Arc<DbPool> {
    Arc::new(DbPool::new("postgresql://localhost/myapp"))
}

#[layer]
pub fn request_tracing() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>> {
    TraceLayer::new_for_http()
}
