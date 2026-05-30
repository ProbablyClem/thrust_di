use std::sync::Arc;
use thrust_macros::service;

#[service]
pub struct UserRepository;

#[service]
pub struct UserService {
    pub repo: Arc<UserRepository>,
}

#[service]
pub struct EmailService {
    pub repo: Arc<UserRepository>,
}

impl UserService {
    pub fn hello(&self) -> &str {
        "hello from UserService"
    }
}
