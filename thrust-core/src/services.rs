use thrust_macros::service;

/// Abstraction the services depend on, rather than a concrete repository.
/// A bare `dyn UserRepository` field on a `#[service]` is resolved to the
/// concrete impl at build time and compiled as `Arc<PostgresUserRepository>`
/// (static dispatch), so the trait needs no `Send + Sync` bound.
pub trait UserRepository {
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
    pub repo: dyn UserRepository,
}

#[service]
pub struct EmailService {
    pub repo: dyn UserRepository,
}

impl UserService {
    pub fn hello(&self) -> String {
        format!("hello from UserService — {}", self.repo.find_user())
    }
}
