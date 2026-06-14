use thrust_macros::bean;

use crate::ServerConfig;

/// Configure the HTTP server in code. Declaring a `#[bean]` that produces a
/// `ServerConfig` makes thrust's generated `run()` use it instead of the
/// defaults. thrust wraps it in `Arc` for you.
#[bean]
pub fn server_config() -> ServerConfig {
    ServerConfig {
        port: 3000,
        ..Default::default()
    }
}
