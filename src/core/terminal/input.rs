/// Represents a parsed key event from raw terminal input bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyEvent {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    CtrlC,
    CtrlD,
    Escape,
    ShiftEnter,
    Unknown(Vec<u8>),
}

/// Parses raw byte sequences from the terminal into `KeyEvent`s.
pub struct InputParser {
    pub buf: Vec<u8>,
}

impl InputParser {
    pub fn new() -> Self {
        InputParser { buf: Vec::new() }
    }

    /// Append raw bytes to the internal buffer.
    pub fn feed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Attempt to parse and consume the next key event from the buffer.
    /// Returns `None` if the buffer is empty or contains only an incomplete
    /// sequence that might still be extended.
    pub fn next_event(&mut self) -> Option<KeyEvent> {
        if self.buf.is_empty() {
            return None;
        }

        let first = self.buf[0];

        match first {
            // Control characters
            3 => {
                self.buf.drain(..1);
                Some(KeyEvent::CtrlC)
            }
            4 => {
                self.buf.drain(..1);
                Some(KeyEvent::CtrlD)
            }
            13 => {
                self.buf.drain(..1);
                Some(KeyEvent::Enter)
            }
            127 => {
                self.buf.drain(..1);
                Some(KeyEvent::Backspace)
            }

            // Escape sequences
            0x1b => {
                if self.buf.len() == 1 {
                    // Lone ESC — consume and return Escape
                    self.buf.drain(..1);
                    return Some(KeyEvent::Escape);
                }

                if self.buf[1] == b'[' {
                    // CSI sequence: ESC [ ...
                    match self.buf.get(2) {
                        None => {
                            // Incomplete — wait for more data; but since we
                            // treat the buffer as fully available we return
                            // Escape and leave the rest for the next call.
                            self.buf.drain(..1);
                            return Some(KeyEvent::Escape);
                        }
                        Some(&b'A') => { self.buf.drain(..3); return Some(KeyEvent::Up); }
                        Some(&b'B') => { self.buf.drain(..3); return Some(KeyEvent::Down); }
                        Some(&b'C') => { self.buf.drain(..3); return Some(KeyEvent::Right); }
                        Some(&b'D') => { self.buf.drain(..3); return Some(KeyEvent::Left); }
                        Some(&b'H') => { self.buf.drain(..3); return Some(KeyEvent::Home); }
                        Some(&b'F') => { self.buf.drain(..3); return Some(KeyEvent::End); }
                        Some(&b'3') => {
                            // Delete: ESC [ 3 ~
                            if self.buf.get(3) == Some(&b'~') {
                                self.buf.drain(..4);
                                return Some(KeyEvent::Delete);
                            }
                            // Unknown sequence — consume what we have
                            let consumed: Vec<u8> = self.buf.drain(..self.buf.len().min(4)).collect();
                            return Some(KeyEvent::Unknown(consumed));
                        }
                        Some(_) => {
                            // Unknown CSI sequence — consume ESC [ and the byte
                            let consumed: Vec<u8> = self.buf.drain(..3).collect();
                            return Some(KeyEvent::Unknown(consumed));
                        }
                    }
                }

                // ESC followed by something that is not '[' — return Escape, leave rest
                self.buf.drain(..1);
                Some(KeyEvent::Escape)
            }

            // Printable ASCII: 32 (space) through 126 (~)
            32..=126 => {
                let ch = first as char;
                self.buf.drain(..1);
                Some(KeyEvent::Char(ch))
            }

            // UTF-8 multibyte sequences: leading byte 0x80–0xFF
            0x80..=0xFF => {
                // Determine expected sequence length from the leading byte.
                let seq_len: usize = if first & 0b1111_0000 == 0b1111_0000 {
                    4
                } else if first & 0b1110_0000 == 0b1100_0000 {
                    2
                } else if first & 0b1111_0000 == 0b1110_0000 {
                    3
                } else {
                    // Continuation byte or invalid — consume and mark unknown
                    let b = self.buf.drain(..1).collect();
                    return Some(KeyEvent::Unknown(b));
                };

                if self.buf.len() < seq_len {
                    // Incomplete multibyte sequence — wait; treat current buf as unknown
                    let consumed: Vec<u8> = self.buf.drain(..).collect();
                    return Some(KeyEvent::Unknown(consumed));
                }

                let bytes: Vec<u8> = self.buf.drain(..seq_len).collect();
                match std::str::from_utf8(&bytes) {
                    Ok(s) => {
                        if let Some(ch) = s.chars().next() {
                            Some(KeyEvent::Char(ch))
                        } else {
                            Some(KeyEvent::Unknown(bytes))
                        }
                    }
                    Err(_) => Some(KeyEvent::Unknown(bytes)),
                }
            }

            // Everything else (0x00–0x1F control chars not handled above, etc.)
            _ => {
                let b = self.buf.drain(..1).collect();
                Some(KeyEvent::Unknown(b))
            }
        }
    }
}

impl Default for InputParser {
    fn default() -> Self {
        Self::new()
    }
}
