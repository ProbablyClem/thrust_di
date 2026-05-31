use std::sync::Arc;
use thrust_macros::service;

use crate::db::DbPool;

#[service]
pub struct UserService {
    pub pool: Arc<DbPool>,
}

impl UserService {
    pub fn list_users(&self) -> Vec<String> {
        vec![
            format!("Alice (via {})", self.pool.url),
            "Bob".to_string(),
        ]
    }
}
