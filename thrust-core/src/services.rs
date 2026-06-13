use thrust_macros::{interface, service};

/// Abstraction the services depend on, rather than a concrete repository.
/// `#[interface]` adds the `Send + Sync` bounds required to store the
/// implementation as `Arc<dyn UserRepository>` in the shared container.
#[interface]
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
