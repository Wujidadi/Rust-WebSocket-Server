use std::{
    fs::{create_dir_all, OpenOptions},
    io::Write,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures_util::{
    SinkExt,
    stream::SplitSink,
    StreamExt,
};
use lazy_static::lazy_static;
use reqwest::Client;
use simplelog::*;
use time::{
    format_description::{self, FormatItem},
    OffsetDateTime,
    UtcOffset,
};
use tokio::{
    net::{TcpListener, TcpStream},
    signal,
    time::{Duration, timeout},
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::handshake::server::{Request, Response},
    tungstenite::protocol::Message,
    WebSocketStream,
};

lazy_static! {
    static ref LOG_TIME_FORMAT: Vec<FormatItem<'static>> = {
        let log_time_format = "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:6]";
        format_description::parse(log_time_format).unwrap()
    };
    static ref LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
}

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

    log_message(Level::Info, "WebSocket server is starting...");

    let addr = "127.0.0.1:6005".to_string();
    let listener = TcpListener::bind(&addr).await.unwrap();
    println!("WebSocket server running on ws://{}", addr);

    let server_guard = ServerGuard;

    let server = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(handle_connection(stream));
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

fn update_log_file() {
    let timezone = UtcOffset::from_hms(8, 0, 0).unwrap();
    let now = OffsetDateTime::now_utc().to_offset(timezone);
    let log_date = now
        .format(&format_description!("[year]-[month]-[day]"))
        .unwrap();
    let log_file_name = format!("logs/server_{}.log", log_date);
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_name)
        .unwrap();

    let mut log_file_guard = LOG_FILE.lock().unwrap();
    *log_file_guard = Some(log_file);
}

fn log_message(level: Level, message: &str) {
    update_log_file();

    let mut log_file_guard = LOG_FILE.lock().unwrap();
    if let Some(ref mut log_file) = *log_file_guard {
        let timezone = UtcOffset::from_hms(8, 0, 0).unwrap();
        let now = OffsetDateTime::now_utc().to_offset(timezone);
        let timestamp = now
            .format(&LOG_TIME_FORMAT)
            .unwrap();
        writeln!(log_file, "[{}] {}: {}", timestamp, level, message).unwrap();
    }
}

async fn handle_connection(stream: TcpStream) {
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
        let msg = timeout(Duration::from_secs(50), read.next()).await;

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
                log_message(Level::Info, &format!("Client {} timed out due to inactivity", peer_addr));
                break;
            }
        }
    }
}

async fn handle_text_message(
    msg: Message,
    peer_addr: SocketAddr,
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    path: &Arc<tokio::sync::RwLock<String>>,
    client: &Arc<Client>,
) {
    let text = msg.to_text().unwrap();
    log_message(Level::Info, &format!("Received message from {}: {}", peer_addr, text));

    if text == "2" {
        write.send(Message::text("3")).await.expect("Error sending message");
        return;
    }

    let request_path = path.read().await.clone();
    let target_url = match map_websocket_to_http(&request_path) {
        Some(url) => url,
        None => {
            write.send(Message::text("{}")).await.expect("Error sending message");
            return;
        }
    };

    let response = client.post(target_url).body(msg.to_string()).send().await;

    match response {
        Ok(resp) => {
            let resp_text = resp.text().await.unwrap_or_else(|_| "Error reading response".to_string());
            write.send(Message::text(resp_text)).await.expect("Error sending message");
        }
        Err(_) => {
            write.send(Message::text("Failed to process request")).await.expect("Error sending message");
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

fn map_websocket_to_http(path: &str) -> Option<&str> {
    match path {
        "/sms/message" => Some("http://127.0.0.1:8080/BankCardTransfer/GetAllSysParameter"),
        "/wechat/message" => Some("http://127.0.0.1:8080/BankCardTransfer/GetNextTask"),
        _ => None,
    }
}
