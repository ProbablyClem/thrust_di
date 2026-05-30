use thrust_macros::service;

#[service]
pub struct GreetingService;

impl GreetingService {
    pub fn greet(&self, name: &str) -> String {
        format!("Hello, {name}!")
    }
}
