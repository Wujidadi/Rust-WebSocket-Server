use std::sync::Arc;

use futures_util::StreamExt;
use log::Level;
use reqwest::Client;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

use crate::config::Config;
use crate::log::log_message;
use crate::message::text as handle_text_message;

pub async fn handle(stream: TcpStream, config: Arc<Config>) {
    let peer_addr = stream.peer_addr().unwrap();
    log_message(Level::Info, &format!("Client connected: {}", peer_addr));

    let path = Arc::new(tokio::sync::RwLock::new(String::new()));

    let ws_stream = accept_hdr_async(stream, |req: &Request, res: Response| {
        let request_path = req.uri().path().to_string();
        let path = path.clone();
        tokio::spawn(async move {
            *path.write().await = request_path;
        });
        Ok(res)
    })
    .await
    .expect("Error during WebSocket handshake");

    let (mut write, mut read) = ws_stream.split();
    let client = Arc::new(Client::new());

    loop {
        let msg = timeout(config.timeout(), read.next()).await;

        match msg {
            Ok(Some(Ok(msg))) => {
                if msg.is_text() {
                    handle_text_message(msg, peer_addr, &mut write, &path, &client).await;
                }
            }
            Ok(Some(Err(e))) => {
                eprintln!("Error reading message from {}: {:?}", peer_addr, e);
                break;
            }
            Ok(None) => {
                println!("Client {} disconnected", peer_addr);
                log_message(Level::Info, &format!("Client {} disconnected", peer_addr));
                break;
            }
            Err(_) => {
                println!("Client {} timed out due to inactivity", peer_addr);
                log_message(
                    Level::Info,
                    &format!("Client {} timed out due to inactivity", peer_addr),
                );
                break;
            }
        }
    }
}
