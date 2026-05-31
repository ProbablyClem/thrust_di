pub struct DbPool {
    pub url: String,
}

impl DbPool {
    pub fn new(url: &str) -> Self {
        Self { url: url.to_string() }
    }
}
