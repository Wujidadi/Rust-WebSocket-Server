use std::fs::{create_dir_all, OpenOptions};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use simplelog::*;
use time::{OffsetDateTime, UtcOffset};
use time::macros::format_description;
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use tokio_tungstenite::{accept_hdr_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

#[tokio::main]
async fn main() {
    create_dir_all("logs").unwrap();

    let timezone = UtcOffset::from_hms(8, 0, 0).unwrap();
    let config = ConfigBuilder::new()
        .set_time_format_custom(format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:6]"
        ))
        .set_time_offset(timezone)
        .build();

    let now = OffsetDateTime::now_utc().to_offset(timezone);
    let date = now
        .format(&format_description!("[year]-[month]-[day]"))
        .unwrap();
    let log_file_name = format!("logs/server_{}.log", date);
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_name)
        .unwrap();

    CombinedLogger::init(vec![WriteLogger::new(LevelFilter::Info, config, log_file)]).unwrap();

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
    })
        .await
        .expect("Error during WebSocket handshake");

    let (mut write, mut read) = ws_stream.split();
    let client = Arc::new(Client::new());

    loop {
        let msg = timeout(Duration::from_secs(50), read.next()).await;

        match msg {
            Ok(Some(Ok(msg))) => {
                if msg.is_text() {
                    let text = msg.to_text().unwrap();
                    log::info!("Received message: {}", text);

                    if text == "2" {
                        write
                            .send(Message::text("3"))
                            .await
                            .expect("Error sending message");
                        continue;
                    }

                    let request_path = path.read().await.clone();
                    let target_url = match map_websocket_to_http(&request_path) {
                        Some(url) => url,
                        None => {
                            write
                                .send(Message::text("{}"))
                                .await
                                .expect("Error sending message");
                            continue;
                        }
                    };

                    let client = client.clone();
                    let response = client.post(target_url).body(msg.to_string()).send().await;

                    match response {
                        Ok(resp) => {
                            let resp_text = resp
                                .text()
                                .await
                                .unwrap_or_else(|_| "Error reading response".to_string());
                            write
                                .send(Message::text(resp_text))
                                .await
                                .expect("Error sending message");
                        }
                        Err(_) => {
                            write
                                .send(Message::text("Failed to process request"))
                                .await
                                .expect("Error sending message");
                        }
                    }
                }
            }
            Ok(Some(Err(e))) => {
                eprintln!("Error reading message: {:?}", e);
                break;
            }
            Ok(None) => {
                println!("Client disconnected");
                break;
            }
            Err(_) => {
                println!("Client timed out due to inactivity");
                break;
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
