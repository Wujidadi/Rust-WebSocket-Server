use std::env;

use tokio::time::Duration;

pub struct Config {
    pub host: String,
    pub port: String,
    pub timeout: Duration,
}

impl Config {
    pub fn new() -> Self {
        dotenv::dotenv().ok();

        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT").unwrap_or_else(|_| "6005".to_string());
        let timeout = env::var("TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or_else(|| Duration::from_secs(50_000_000));

        Config { host, port, timeout }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}
