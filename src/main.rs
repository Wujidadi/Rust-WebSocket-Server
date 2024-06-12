use std::fs::create_dir_all;
use std::sync::Arc;

use simplelog::*;
use tokio::{net::TcpListener, signal};

use config::Config;
use connect::handle as handle_connect;
use log::{log_message, update_log_file};

mod config;
mod connect;
mod log;
mod message;
mod routes;

struct ServerGuard;

impl Drop for ServerGuard {
    fn drop(&mut self) {
        log_message(Level::Info, "Server is shutting down...");
    }
}

#[tokio::main]
async fn main() {
    create_dir_all("logs").unwrap();
    update_log_file();

    let config = Arc::new(Config::new());

    log_message(Level::Info, "WebSocket server is starting...");
    let listener = TcpListener::bind(config.address()).await.unwrap();
    println!("WebSocket server running on ws://{}", config.address());

    let server_guard = ServerGuard;

    let server = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let config = config.clone();
            tokio::spawn(async move {
                handle_connect(stream, config).await;
            });
        }
    });

    tokio::select! {
        _ = signal::ctrl_c() => {
            log_message(Level::Info, "Received Ctrl-C signal");
        }
        result = server => {
            if let Err(e) = result {
                log_message(Level::Error, &format!("Server task failed: {:?}", e));
            }
        }
    }

    drop(server_guard);
}
