use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use log::Level;
use reqwest::Client;
use simplelog::format_description;
use time::{OffsetDateTime, UtcOffset};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use crate::log::{LOG_FILE, log_message, update_log_file};
use crate::routes::map as route_map;

pub async fn text(
    msg: Message,
    peer_addr: SocketAddr,
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    path: &Arc<tokio::sync::RwLock<String>>,
    client: &Arc<Client>,
) {
    let text = msg.to_text().unwrap();
    log_message(
        Level::Info,
        &format!("Received message from {}: {}", peer_addr, text),
    );

    if text == "2" {
        write
            .send(Message::text("3"))
            .await
            .expect("Error sending message");
        return;
    }

    let request_path = path.read().await.clone();
    let target_url = match route_map(&request_path) {
        Some(url) => url,
        None => {
            write
                .send(Message::text("{}"))
                .await
                .expect("Error sending message");
            return;
        }
    };

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

    // Check the current date and update the log file if necessary
    let timezone = UtcOffset::from_hms(8, 0, 0).unwrap();
    let now = OffsetDateTime::now_utc().to_offset(timezone);
    let date = now
        .format(&format_description!("[year]-[month]-[day]"))
        .unwrap();
    let log_file_name = format!("logs/server_{}.log", date);

    let log_file_guard = LOG_FILE.lock().unwrap();
    if let Some(ref _log_file) = *log_file_guard {
        let current_log_file_name = log_file_name.clone();
        if log_file_name != current_log_file_name {
            drop(log_file_guard);
            update_log_file();
        }
    }
}
