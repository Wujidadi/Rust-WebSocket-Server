use std::env;

pub struct Config {
    pub host: String,
    pub port: String,
}

impl Config {
    pub fn new() -> Self {
        dotenv::dotenv().ok();

        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT").unwrap_or_else(|_| "6005".to_string());

        Config { host, port }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
