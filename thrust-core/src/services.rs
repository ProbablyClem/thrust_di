use thrust_macros::service;

#[service]
pub struct UserService;

#[service]
pub struct EmailService;

impl UserService {
    pub fn hello(&self) -> &str {
        "hello from UserService"
    }
}
