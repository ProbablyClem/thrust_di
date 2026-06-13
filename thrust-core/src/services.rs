use std::sync::Arc;
use thrust_macros::service;

/// Abstraction the services depend on, rather than a concrete repository.
/// `Send + Sync` is required because the implementation is stored as
/// `Arc<dyn UserRepository>` inside the (thread-shared) container.
pub trait UserRepository: Send + Sync {
    fn find_user(&self) -> String;
}

#[service]
pub struct PostgresUserRepository;

impl UserRepository for PostgresUserRepository {
    fn find_user(&self) -> String {
        "user from postgres".to_string()
    }
}

#[service]
pub struct UserService {
    pub repo: Arc<dyn UserRepository>,
}

#[service]
pub struct EmailService {
    pub repo: Arc<dyn UserRepository>,
}

impl UserService {
    pub fn hello(&self) -> String {
        format!("hello from UserService — {}", self.repo.find_user())
    }
}
