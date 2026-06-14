use thrust_macros::service;

/// Abstraction the services depend on, rather than a concrete repository.
/// A bare `dyn UserRepository` field on a `#[service]` is compiled to
/// `Arc<dyn UserRepository + Send + Sync>` (dynamic dispatch). The concrete impl
/// is still resolved at build time for container wiring, but the field stays a
/// trait object — identical in tests and production, so it can be mocked. The
/// `Send + Sync` bound is added at the field use-site, so the trait stays clean.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A hand-written mock of the `UserRepository` dependency. The `#[service]`
    /// macro compiles `UserService::repo` as `Arc<dyn UserRepository + Send +
    /// Sync>` (a trait object) in every build, so we can construct `UserService`
    /// with this mock instead of the concrete `PostgresUserRepository`.
    struct MockUserRepository;
    impl UserRepository for MockUserRepository {
        fn find_user(&self) -> String {
            "mock user".to_string()
        }
    }

    #[test]
    fn hello_uses_injected_repo() {
        let service = UserService {
            repo: Arc::new(MockUserRepository),
        };
        assert_eq!(service.hello(), "hello from UserService — mock user");
    }
}
