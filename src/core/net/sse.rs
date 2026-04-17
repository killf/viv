#[derive(Debug, Clone, PartialEq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

pub struct SseParser {
    buf: String,
}

impl SseParser {
    pub fn new() -> Self {
        SseParser { buf: String::new() }
    }

    pub fn feed(&mut self, data: &str) {
        self.buf.push_str(data);
    }

    pub fn drain(&mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();

        while let Some(pos) = self.buf.find("\n\n") {
            let block = self.buf[..pos].to_string();
            self.buf = self.buf[pos + 2..].to_string();

            let mut event_type: Option<String> = None;
            let mut data_parts: Vec<String> = Vec::new();

            for line in block.lines() {
                if line.starts_with(':') {
                    // comment — ignore
                } else if let Some(val) = line.strip_prefix("event: ") {
                    event_type = Some(val.to_string());
                } else if let Some(val) = line.strip_prefix("data: ") {
                    data_parts.push(val.to_string());
                }
            }

            if !data_parts.is_empty() {
                events.push(SseEvent {
                    event: event_type,
                    data: data_parts.join("\n"),
                });
            }
        }

        events
    }
}
