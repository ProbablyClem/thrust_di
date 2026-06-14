use thrust_macros::service;

/// Both services depend on this abstraction. The bare `dyn UserRepository`
/// fields are resolved to the concrete impl at build time, so the generated
/// component metadata reports the dependency as `PostgresUserRepository`.
pub trait UserRepository {
    fn find_user(&self) -> String;
}

#[service]
pub struct PostgresUserRepository;

impl UserRepository for PostgresUserRepository {
    fn find_user(&self) -> String {
        "alice".to_string()
    }
}

#[service]
pub struct UserService {
    pub repo: dyn UserRepository,
}

impl UserService {
    pub fn current_user(&self) -> String {
        self.repo.find_user()
    }
}

#[service]
pub struct EmailService {
    pub repo: dyn UserRepository,
}

impl EmailService {
    pub fn welcome_message(&self) -> String {
        format!("Welcome, {}!", self.repo.find_user())
    }
}
