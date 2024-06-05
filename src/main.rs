use tokio::net::TcpListener;
use tokio_tungstenite::{accept_hdr_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use reqwest::Client;
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:6005".to_string();
    let listener = TcpListener::bind(&addr).await.unwrap();
    println!("WebSocket server running on ws://{}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_connection(stream));
    }
}

async fn handle_connection(stream: tokio::net::TcpStream) {
    let path = Arc::new(tokio::sync::RwLock::new(String::new()));

    let ws_stream = accept_hdr_async(stream, |req: &Request, res: Response| {
        let request_path = req.uri().path().to_string();
        let path = path.clone();
        tokio::spawn(async move {
            *path.write().await = request_path;
        });
        Ok(res)
    }).await.expect("Error during WebSocket handshake");

    let (mut write, mut read) = ws_stream.split();
    let client = Arc::new(Client::new());

    while let Some(msg) = read.next().await {
        let msg = msg.expect("Error reading message");

        if msg.is_text() {
            let request_path = path.read().await.clone();
            let target_url = match map_websocket_to_http(&request_path) {
                Some(url) => url,
                None => {
                    write.send(Message::text("{}")).await.expect("Error sending message");
                    return;
                }
            };

            let client = client.clone();
            let response = client.post(target_url)
                .body(msg.to_string())
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let resp_text = resp.text().await.unwrap_or_else(|_| "Error reading response".to_string());
                    write.send(Message::text(resp_text)).await.expect("Error sending message");
                }
                Err(_) => {
                    write.send(Message::text("Failed to process request")).await.expect("Error sending message");
                }
            }
        }
    }
}

fn map_websocket_to_http(path: &str) -> Option<&str> {
    match path {
        "/sms/message" => Some("http://127.0.0.1:8080/BankCardTransfer/GetAllSysParameter"),
        "/wechat/message" => Some("http://127.0.0.1:8080/BankCardTransfer/GetNextTask"),
        _ => None,
    }
}
