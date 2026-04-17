use std::fmt;
use crate::Error;

pub trait ToJson {
    fn to_json(&self) -> String;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Number::Int(n) => Some(*n),
            Number::Float(n) => {
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    Some(*n as i64)
                } else {
                    None
                }
            }
        }
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            Number::Int(n) => *n as f64,
            Number::Float(n) => *n,
        }
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Number::Int(n) => write!(f, "{}", n),
            Number::Float(n) => write!(f, "{}", n),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(Number),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn parse(input: &str) -> crate::Result<JsonValue> {
        let mut parser = Parser::new(input);
        let value = parser.parse_value()?;
        parser.skip_whitespace();
        if parser.pos < parser.chars.len() {
            return Err(Error::Json(format!("Unexpected trailing characters at position {}", parser.pos)));
        }
        Ok(value)
    }

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        if let JsonValue::Object(pairs) = self {
            for (k, v) in pairs {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn as_str(&self) -> Option<&str> {
        if let JsonValue::Str(s) = self {
            Some(s.as_str())
        } else {
            None
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        if let JsonValue::Number(n) = self {
            Some(n.as_f64())
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        if let JsonValue::Number(n) = self {
            n.as_i64()
        } else {
            None
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let JsonValue::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
        if let JsonValue::Array(arr) = self {
            Some(arr)
        } else {
            None
        }
    }

    pub fn as_object(&self) -> Option<&Vec<(String, JsonValue)>> {
        if let JsonValue::Object(pairs) = self {
            Some(pairs)
        } else {
            None
        }
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonValue::Null => write!(f, "null"),
            JsonValue::Bool(b) => write!(f, "{}", b),
            JsonValue::Number(n) => write!(f, "{}", n),
            JsonValue::Str(s) => {
                write!(f, "\"")?;
                for ch in s.chars() {
                    match ch {
                        '"' => write!(f, "\\\"")?,
                        '\\' => write!(f, "\\\\")?,
                        '\n' => write!(f, "\\n")?,
                        '\r' => write!(f, "\\r")?,
                        '\t' => write!(f, "\\t")?,
                        '\x08' => write!(f, "\\b")?,
                        '\x0C' => write!(f, "\\f")?,
                        c => write!(f, "{}", c)?,
                    }
                }
                write!(f, "\"")
            }
            JsonValue::Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            JsonValue::Object(pairs) => {
                write!(f, "{{")?;
                for (i, (key, val)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    let key_val = JsonValue::Str(key.clone());
                    write!(f, "{}:{}", key_val, val)?;
                }
                write!(f, "}}")
            }
        }
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Parser {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn consume(&mut self) -> Option<char> {
        if self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), Error> {
        match self.consume() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(Error::Json(format!("Expected '{}' but got '{}' at position {}", expected, ch, self.pos - 1))),
            None => Err(Error::Json(format!("Expected '{}' but got end of input", expected))),
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, Error> {
        self.skip_whitespace();
        match self.peek() {
            Some('n') => self.parse_null(),
            Some('t') | Some('f') => self.parse_bool(),
            Some('"') => Ok(JsonValue::Str(self.parse_string()?)),
            Some('[') => self.parse_array(),
            Some('{') => self.parse_object(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(Error::Json(format!("Unexpected character '{}' at position {}", c, self.pos))),
            None => Err(Error::Json("Unexpected end of input".to_string())),
        }
    }

    fn parse_null(&mut self) -> Result<JsonValue, Error> {
        for expected in ['n', 'u', 'l', 'l'] {
            self.expect(expected)?;
        }
        Ok(JsonValue::Null)
    }

    fn parse_bool(&mut self) -> Result<JsonValue, Error> {
        if self.peek() == Some('t') {
            for expected in ['t', 'r', 'u', 'e'] {
                self.expect(expected)?;
            }
            Ok(JsonValue::Bool(true))
        } else {
            for expected in ['f', 'a', 'l', 's', 'e'] {
                self.expect(expected)?;
            }
            Ok(JsonValue::Bool(false))
        }
    }

    fn parse_string(&mut self) -> Result<String, Error> {
        self.expect('"')?;
        let mut result = String::new();
        loop {
            match self.consume() {
                Some('"') => break,
                Some('\\') => {
                    match self.consume() {
                        Some('"') => result.push('"'),
                        Some('\\') => result.push('\\'),
                        Some('/') => result.push('/'),
                        Some('b') => result.push('\x08'),
                        Some('f') => result.push('\x0C'),
                        Some('n') => result.push('\n'),
                        Some('r') => result.push('\r'),
                        Some('t') => result.push('\t'),
                        Some('u') => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                match self.consume() {
                                    Some(h) if h.is_ascii_hexdigit() => hex.push(h),
                                    Some(c) => return Err(Error::Json(format!("Invalid hex digit '{}' in unicode escape", c))),
                                    None => return Err(Error::Json("Unexpected end in unicode escape".to_string())),
                                }
                            }
                            let code_point = u32::from_str_radix(&hex, 16)
                                .map_err(|e| Error::Json(format!("Invalid unicode escape: {}", e)))?;
                            let ch = char::from_u32(code_point)
                                .ok_or_else(|| Error::Json(format!("Invalid unicode code point: {}", code_point)))?;
                            result.push(ch);
                        }
                        Some(c) => return Err(Error::Json(format!("Invalid escape sequence '\\{}' at position {}", c, self.pos - 1))),
                        None => return Err(Error::Json("Unexpected end of input in string escape".to_string())),
                    }
                }
                Some(c) => result.push(c),
                None => return Err(Error::Json("Unexpected end of input in string".to_string())),
            }
        }
        Ok(result)
    }

    fn parse_number(&mut self) -> Result<JsonValue, Error> {
        let start = self.pos;
        let mut is_float = false;
        // optional minus
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        // integer part
        match self.peek() {
            Some('0') => { self.pos += 1; }
            Some(c) if c.is_ascii_digit() => {
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            _ => return Err(Error::Json(format!("Invalid number at position {}", self.pos))),
        }
        // optional fractional part
        if self.peek() == Some('.') {
            is_float = true;
            self.pos += 1;
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return Err(Error::Json(format!("Expected digit after '.' at position {}", self.pos)));
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        // optional exponent
        if matches!(self.peek(), Some('e') | Some('E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.pos += 1;
            }
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return Err(Error::Json(format!("Expected digit in exponent at position {}", self.pos)));
            }
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let num_str: String = self.chars[start..self.pos].iter().collect();
        if is_float {
            let n: f64 = num_str.parse().map_err(|e| Error::Json(format!("Invalid number '{}': {}", num_str, e)))?;
            Ok(JsonValue::Number(Number::Float(n)))
        } else {
            let n: i64 = num_str.parse().map_err(|e| Error::Json(format!("Invalid number '{}': {}", num_str, e)))?;
            Ok(JsonValue::Number(Number::Int(n)))
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, Error> {
        self.expect('[')?;
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let val = self.parse_value()?;
            items.push(val);
            self.skip_whitespace();
            match self.peek() {
                Some(',') => { self.pos += 1; }
                Some(']') => { self.pos += 1; break; }
                Some(c) => return Err(Error::Json(format!("Expected ',' or ']' in array, got '{}' at position {}", c, self.pos))),
                None => return Err(Error::Json("Unexpected end of input in array".to_string())),
            }
        }
        Ok(JsonValue::Array(items))
    }

    fn parse_object(&mut self) -> Result<JsonValue, Error> {
        self.expect('{')?;
        self.skip_whitespace();
        let mut pairs = Vec::new();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(':')?;
            let val = self.parse_value()?;
            pairs.push((key, val));
            self.skip_whitespace();
            match self.peek() {
                Some(',') => { self.pos += 1; }
                Some('}') => { self.pos += 1; break; }
                Some(c) => return Err(Error::Json(format!("Expected ',' or '}}' in object, got '{}' at position {}", c, self.pos))),
                None => return Err(Error::Json("Unexpected end of input in object".to_string())),
            }
        }
        Ok(JsonValue::Object(pairs))
    }
}
