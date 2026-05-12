use chrono::Local;

#[derive(Clone, Debug)]
pub struct LogEvent {
    pub timestamp: String,
    pub message: String,
}

impl LogEvent {
    pub fn line(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
        }
    }
}
