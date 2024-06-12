pub fn map(path: &str) -> Option<&str> {
    match path {
        "/sms/message" => Some("http://127.0.0.1:8080/BankCardTransfer/GetAllSysParameter"),
        "/wechat/message" => Some("http://127.0.0.1:8080/BankCardTransfer/GetNextTask"),
        _ => None,
    }
}
