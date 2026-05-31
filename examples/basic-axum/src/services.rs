use std::sync::Arc;
use thrust_macros::service;

#[service]
pub struct TodoRepository;

#[service]
pub struct TodoService {
    pub repo: Arc<TodoRepository>,
}

impl TodoService {
    pub fn find_all(&self) -> Vec<&'static str> {
        vec!["buy milk", "write tests"]
    }
}
