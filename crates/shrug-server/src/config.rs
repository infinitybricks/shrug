#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub max_body_bytes: usize,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            max_body_bytes: std::env::var("MAX_BODY_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(256 * 1024),
        }
    }
}
