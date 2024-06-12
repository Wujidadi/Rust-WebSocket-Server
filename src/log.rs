use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

use lazy_static::lazy_static;
use log::Level;
use simplelog::format_description;
use time::{format_description::{self, FormatItem}, OffsetDateTime, UtcOffset};

lazy_static! {
    static ref LOG_TIME_FORMAT: Vec<FormatItem<'static>> = {
        let log_time_format = "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:6]";
        format_description::parse(log_time_format).unwrap()
    };
    pub static ref LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
}

pub fn update_log_file() {
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

pub fn log_message(level: Level, message: &str) {
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
