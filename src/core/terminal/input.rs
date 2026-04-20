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
    CtrlChar(char), // Ctrl+A through Ctrl+Z
    Escape,
    ShiftEnter,
    Unknown(Vec<u8>),
}

/// Represents a parsed mouse event from raw terminal input bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum MouseEvent {
    WheelUp,
    WheelDown,
    LeftPress,
    LeftRelease,
}

/// Represents a parsed input event from raw terminal input bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

/// Parses raw byte sequences from the terminal into `InputEvent`s.
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

    /// Parse a decimal number from a byte slice, returning `None` if any byte
    /// is not an ASCII digit.
    fn parse_u8(s: &[u8]) -> Option<u8> {
        let mut val: u8 = 0;
        for &b in s {
            if b < b'0' || b > b'9' {
                return None;
            }
            val = val.saturating_mul(10) + (b - b'0');
        }
        Some(val)
    }

    /// Attempt to parse and consume the next input event from the buffer.
    /// Returns `None` if the buffer is empty or contains only an incomplete
    /// sequence that might still be extended.
    pub fn next_event(&mut self) -> Option<InputEvent> {
        if self.buf.is_empty() {
            return None;
        }

        let first = self.buf[0];

        match first {
            // Control characters
            3 => {
                self.buf.drain(..1);
                Some(InputEvent::Key(KeyEvent::CtrlC))
            }
            4 => {
                self.buf.drain(..1);
                Some(InputEvent::Key(KeyEvent::CtrlD))
            }
            // Ctrl+A (1) through Ctrl+C (3), Ctrl+E through Ctrl+Z (5-26)
            // Skip 13 (Ctrl+M = Enter) — handled separately
            1..=3 | 5..=12 | 14..=26 => {
                let ch = (first - 1 + b'a') as char;
                self.buf.drain(..1);
                Some(InputEvent::Key(KeyEvent::CtrlChar(ch)))
            }
            13 => {
                self.buf.drain(..1);
                Some(InputEvent::Key(KeyEvent::Enter))
            }
            127 => {
                self.buf.drain(..1);
                Some(InputEvent::Key(KeyEvent::Backspace))
            }

            // Escape sequences
            0x1b => {
                if self.buf.len() == 1 {
                    // Lone ESC — consume and return Escape
                    self.buf.drain(..1);
                    return Some(InputEvent::Key(KeyEvent::Escape));
                }

                if self.buf[1] == b'[' {
                    // CSI sequence: ESC [ ...
                    match self.buf.get(2) {
                        None => {
                            // Incomplete — wait for more data; but since we
                            // treat the buffer as fully available we return
                            // Escape and leave the rest for the next call.
                            self.buf.drain(..1);
                            return Some(InputEvent::Key(KeyEvent::Escape));
                        }
                        Some(&b'A') => {
                            self.buf.drain(..3);
                            return Some(InputEvent::Key(KeyEvent::Up));
                        }
                        Some(&b'B') => {
                            self.buf.drain(..3);
                            return Some(InputEvent::Key(KeyEvent::Down));
                        }
                        Some(&b'C') => {
                            self.buf.drain(..3);
                            return Some(InputEvent::Key(KeyEvent::Right));
                        }
                        Some(&b'D') => {
                            self.buf.drain(..3);
                            return Some(InputEvent::Key(KeyEvent::Left));
                        }
                        Some(&b'H') => {
                            self.buf.drain(..3);
                            return Some(InputEvent::Key(KeyEvent::Home));
                        }
                        Some(&b'F') => {
                            self.buf.drain(..3);
                            return Some(InputEvent::Key(KeyEvent::End));
                        }
                        Some(&b'3') => {
                            // Delete: ESC [ 3 ~
                            if self.buf.get(3) == Some(&b'~') {
                                self.buf.drain(..4);
                                return Some(InputEvent::Key(KeyEvent::Delete));
                            }
                            // Unknown sequence — consume what we have
                            let consumed: Vec<u8> =
                                self.buf.drain(..self.buf.len().min(4)).collect();
                            return Some(InputEvent::Key(KeyEvent::Unknown(consumed)));
                        }
                        Some(_) => {
                            // Try URXVT mouse mode (1015): ESC [ M b x y
                            // b = button + 32 (64=wheel up, 65=wheel down), x = col + 33, y = row + 33
                            if self.buf.get(2) == Some(&b'M') && self.buf.len() >= 6 {
                                let btn = self.buf[3].saturating_sub(32);
                                self.buf.drain(..6);
                                match btn {
                                    64 => return Some(InputEvent::Mouse(MouseEvent::WheelUp)),
                                    65 => return Some(InputEvent::Mouse(MouseEvent::WheelDown)),
                                    _ => {} // other mouse events: consume and skip
                                }
                            }
                            // Try SGR mouse: ESC [ < N ; X ; Y M (press) or m (release)
                            if self.buf.get(2) == Some(&b'<')
                                && self.buf.len() >= 6
                            {
                                // Find the M or m marker
                                let body = &self.buf[3..];
                                if let Some(pos) = body.iter().position(|&b| b == b'M' || b == b'm')
                                {
                                    let params = &body[..pos];
                                    // Expect "N;X;Y" — three numbers separated by ';'
                                    let parts: Vec<&[u8]> =
                                        params.split(|&b| b == b';').collect();
                                    if parts.len() == 3 {
                                        let n = Self::parse_u8(parts[0]);
                                        let _y = Self::parse_u8(parts[2]);
                                        let is_release = body[pos] == b'm';
                                        self.buf.drain(..3 + pos + 1);
                                        match (n, is_release) {
                                            (Some(0), false) => {
                                                return Some(InputEvent::Mouse(MouseEvent::LeftPress));
                                            }
                                            (Some(0), true) => {
                                                return Some(InputEvent::Mouse(MouseEvent::LeftRelease));
                                            }
                                            (Some(64), _) => {
                                                return Some(InputEvent::Mouse(MouseEvent::WheelUp));
                                            }
                                            (Some(65), _) => {
                                                return Some(InputEvent::Mouse(MouseEvent::WheelDown));
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            // Unknown CSI sequence — consume ESC [ and the byte
                            let consumed: Vec<u8> = self.buf.drain(..3).collect();
                            return Some(InputEvent::Key(KeyEvent::Unknown(consumed)));
                        }
                    }
                }

                // ESC followed by something that is not '[' — return Escape, leave rest
                self.buf.drain(..1);
                Some(InputEvent::Key(KeyEvent::Escape))
            }

            // Printable ASCII: 32 (space) through 126 (~)
            32..=126 => {
                let ch = first as char;
                self.buf.drain(..1);
                Some(InputEvent::Key(KeyEvent::Char(ch)))
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
                    return Some(InputEvent::Key(KeyEvent::Unknown(b)));
                };

                if self.buf.len() < seq_len {
                    // Incomplete multibyte sequence — wait; treat current buf as unknown
                    let consumed: Vec<u8> = self.buf.drain(..).collect();
                    return Some(InputEvent::Key(KeyEvent::Unknown(consumed)));
                }

                let bytes: Vec<u8> = self.buf.drain(..seq_len).collect();
                match std::str::from_utf8(&bytes) {
                    Ok(s) => {
                        if let Some(ch) = s.chars().next() {
                            Some(InputEvent::Key(KeyEvent::Char(ch)))
                        } else {
                            Some(InputEvent::Key(KeyEvent::Unknown(bytes)))
                        }
                    }
                    Err(_) => Some(InputEvent::Key(KeyEvent::Unknown(bytes))),
                }
            }

            // Everything else (0x00–0x1F control chars not handled above, etc.)
            _ => {
                let b = self.buf.drain(..1).collect();
                Some(InputEvent::Key(KeyEvent::Unknown(b)))
            }
        }
    }
}

impl Default for InputParser {
    fn default() -> Self {
        Self::new()
    }
}
