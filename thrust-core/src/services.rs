use thrust_macros::service;

#[service]
pub struct UserRepository;

#[service]
pub struct UserService {
    pub repo: UserRepository,
}

#[service]
pub struct EmailService {
    pub repo: UserRepository,
}

impl UserService {
    pub fn hello(&self) -> &str {
        "hello from UserService"
    }
}
