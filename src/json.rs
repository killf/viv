use std::fmt;

#[derive(Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn parse(input: &str) -> Result<JsonValue, String> {
        let mut parser = Parser::new(input);
        let value = parser.parse_value()?;
        parser.skip_whitespace();
        if parser.pos < parser.chars.len() {
            return Err(format!("Unexpected trailing characters at position {}", parser.pos));
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
            Some(*n)
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
            JsonValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
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

    fn expect(&mut self, expected: char) -> Result<(), String> {
        match self.consume() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(format!("Expected '{}' but got '{}' at position {}", expected, ch, self.pos - 1)),
            None => Err(format!("Expected '{}' but got end of input", expected)),
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        match self.peek() {
            Some('n') => self.parse_null(),
            Some('t') | Some('f') => self.parse_bool(),
            Some('"') => Ok(JsonValue::Str(self.parse_string()?)),
            Some('[') => self.parse_array(),
            Some('{') => self.parse_object(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!("Unexpected character '{}' at position {}", c, self.pos)),
            None => Err("Unexpected end of input".to_string()),
        }
    }

    fn parse_null(&mut self) -> Result<JsonValue, String> {
        for expected in ['n', 'u', 'l', 'l'] {
            self.expect(expected)?;
        }
        Ok(JsonValue::Null)
    }

    fn parse_bool(&mut self) -> Result<JsonValue, String> {
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

    fn parse_string(&mut self) -> Result<String, String> {
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
                                    Some(c) => return Err(format!("Invalid hex digit '{}' in unicode escape", c)),
                                    None => return Err("Unexpected end in unicode escape".to_string()),
                                }
                            }
                            let code_point = u32::from_str_radix(&hex, 16)
                                .map_err(|e| format!("Invalid unicode escape: {}", e))?;
                            let ch = char::from_u32(code_point)
                                .ok_or_else(|| format!("Invalid unicode code point: {}", code_point))?;
                            result.push(ch);
                        }
                        Some(c) => return Err(format!("Invalid escape sequence '\\{}' at position {}", c, self.pos - 1)),
                        None => return Err("Unexpected end of input in string escape".to_string()),
                    }
                }
                Some(c) => result.push(c),
                None => return Err("Unexpected end of input in string".to_string()),
            }
        }
        Ok(result)
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.pos;
        // optional minus
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        // integer part
        match self.peek() {
            Some('0') => { self.pos += 1; }
            Some(c) if c.is_ascii_digit() => {
                while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            _ => return Err(format!("Invalid number at position {}", self.pos)),
        }
        // optional fractional part
        if self.peek() == Some('.') {
            self.pos += 1;
            if !self.peek().map_or(false, |c| c.is_ascii_digit()) {
                return Err(format!("Expected digit after '.' at position {}", self.pos));
            }
            while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        // optional exponent
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.pos += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.pos += 1;
            }
            if !self.peek().map_or(false, |c| c.is_ascii_digit()) {
                return Err(format!("Expected digit in exponent at position {}", self.pos));
            }
            while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let num_str: String = self.chars[start..self.pos].iter().collect();
        let n: f64 = num_str.parse().map_err(|e| format!("Invalid number '{}': {}", num_str, e))?;
        Ok(JsonValue::Number(n))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
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
                Some(c) => return Err(format!("Expected ',' or ']' in array, got '{}' at position {}", c, self.pos)),
                None => return Err("Unexpected end of input in array".to_string()),
            }
        }
        Ok(JsonValue::Array(items))
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
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
                Some(c) => return Err(format!("Expected ',' or '}}' in object, got '{}' at position {}", c, self.pos)),
                None => return Err("Unexpected end of input in object".to_string()),
            }
        }
        Ok(JsonValue::Object(pairs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Null ---
    #[test]
    fn test_parse_null() {
        assert_eq!(JsonValue::parse("null").unwrap(), JsonValue::Null);
    }

    #[test]
    fn test_parse_null_with_whitespace() {
        assert_eq!(JsonValue::parse("  null  ").unwrap(), JsonValue::Null);
    }

    // --- Booleans ---
    #[test]
    fn test_parse_true() {
        assert_eq!(JsonValue::parse("true").unwrap(), JsonValue::Bool(true));
    }

    #[test]
    fn test_parse_false() {
        assert_eq!(JsonValue::parse("false").unwrap(), JsonValue::Bool(false));
    }

    // --- Numbers ---
    #[test]
    fn test_parse_integer() {
        assert_eq!(JsonValue::parse("42").unwrap(), JsonValue::Number(42.0));
    }

    #[test]
    fn test_parse_negative_integer() {
        assert_eq!(JsonValue::parse("-7").unwrap(), JsonValue::Number(-7.0));
    }

    #[test]
    fn test_parse_float() {
        assert_eq!(JsonValue::parse("-3.14").unwrap(), JsonValue::Number(-3.14));
    }

    #[test]
    fn test_parse_exponent() {
        assert_eq!(JsonValue::parse("1e3").unwrap(), JsonValue::Number(1000.0));
    }

    #[test]
    fn test_parse_negative_exponent() {
        assert_eq!(JsonValue::parse("2.5e-2").unwrap(), JsonValue::Number(0.025));
    }

    #[test]
    fn test_parse_zero() {
        assert_eq!(JsonValue::parse("0").unwrap(), JsonValue::Number(0.0));
    }

    // --- Strings ---
    #[test]
    fn test_parse_simple_string() {
        assert_eq!(JsonValue::parse(r#""hello""#).unwrap(), JsonValue::Str("hello".to_string()));
    }

    #[test]
    fn test_parse_empty_string() {
        assert_eq!(JsonValue::parse(r#""""#).unwrap(), JsonValue::Str("".to_string()));
    }

    #[test]
    fn test_parse_string_with_escaped_quote() {
        assert_eq!(JsonValue::parse(r#""say \"hi\"""#).unwrap(), JsonValue::Str(r#"say "hi""#.to_string()));
    }

    #[test]
    fn test_parse_string_with_escaped_backslash() {
        assert_eq!(JsonValue::parse(r#""a\\b""#).unwrap(), JsonValue::Str("a\\b".to_string()));
    }

    #[test]
    fn test_parse_string_with_escaped_newline() {
        assert_eq!(JsonValue::parse(r#""line1\nline2""#).unwrap(), JsonValue::Str("line1\nline2".to_string()));
    }

    #[test]
    fn test_parse_string_with_escaped_tab() {
        assert_eq!(JsonValue::parse(r#""col1\tcol2""#).unwrap(), JsonValue::Str("col1\tcol2".to_string()));
    }

    #[test]
    fn test_parse_string_with_escaped_slash() {
        assert_eq!(JsonValue::parse(r#""a\/b""#).unwrap(), JsonValue::Str("a/b".to_string()));
    }

    #[test]
    fn test_parse_string_with_unicode_escape() {
        // \u0041 = 'A'
        assert_eq!(JsonValue::parse(r#""\u0041""#).unwrap(), JsonValue::Str("A".to_string()));
    }

    #[test]
    fn test_parse_string_with_unicode_escape_emoji_range() {
        // \u03B1 = Greek small letter alpha
        assert_eq!(JsonValue::parse(r#""\u03B1""#).unwrap(), JsonValue::Str("α".to_string()));
    }

    // --- Arrays ---
    #[test]
    fn test_parse_empty_array() {
        assert_eq!(JsonValue::parse("[]").unwrap(), JsonValue::Array(vec![]));
    }

    #[test]
    fn test_parse_array_of_numbers() {
        assert_eq!(
            JsonValue::parse("[1,2,3]").unwrap(),
            JsonValue::Array(vec![
                JsonValue::Number(1.0),
                JsonValue::Number(2.0),
                JsonValue::Number(3.0),
            ])
        );
    }

    #[test]
    fn test_parse_array_with_whitespace() {
        assert_eq!(
            JsonValue::parse("[ 1 , 2 ]").unwrap(),
            JsonValue::Array(vec![JsonValue::Number(1.0), JsonValue::Number(2.0)])
        );
    }

    #[test]
    fn test_parse_array_mixed_types() {
        assert_eq!(
            JsonValue::parse(r#"[null, true, "hi", 3]"#).unwrap(),
            JsonValue::Array(vec![
                JsonValue::Null,
                JsonValue::Bool(true),
                JsonValue::Str("hi".to_string()),
                JsonValue::Number(3.0),
            ])
        );
    }

    // --- Objects ---
    #[test]
    fn test_parse_empty_object() {
        assert_eq!(JsonValue::parse("{}").unwrap(), JsonValue::Object(vec![]));
    }

    #[test]
    fn test_parse_simple_object() {
        assert_eq!(
            JsonValue::parse(r#"{"key":"value"}"#).unwrap(),
            JsonValue::Object(vec![("key".to_string(), JsonValue::Str("value".to_string()))])
        );
    }

    #[test]
    fn test_parse_object_multiple_keys() {
        assert_eq!(
            JsonValue::parse(r#"{"a":1,"b":2}"#).unwrap(),
            JsonValue::Object(vec![
                ("a".to_string(), JsonValue::Number(1.0)),
                ("b".to_string(), JsonValue::Number(2.0)),
            ])
        );
    }

    #[test]
    fn test_parse_object_with_whitespace() {
        assert_eq!(
            JsonValue::parse(r#"{ "x" : true }"#).unwrap(),
            JsonValue::Object(vec![("x".to_string(), JsonValue::Bool(true))])
        );
    }

    // --- Nested structures ---
    #[test]
    fn test_parse_nested_array() {
        assert_eq!(
            JsonValue::parse("[[1,2],[3,4]]").unwrap(),
            JsonValue::Array(vec![
                JsonValue::Array(vec![JsonValue::Number(1.0), JsonValue::Number(2.0)]),
                JsonValue::Array(vec![JsonValue::Number(3.0), JsonValue::Number(4.0)]),
            ])
        );
    }

    #[test]
    fn test_parse_nested_object() {
        assert_eq!(
            JsonValue::parse(r#"{"outer":{"inner":42}}"#).unwrap(),
            JsonValue::Object(vec![(
                "outer".to_string(),
                JsonValue::Object(vec![("inner".to_string(), JsonValue::Number(42.0))])
            )])
        );
    }

    #[test]
    fn test_parse_object_with_array_value() {
        assert_eq!(
            JsonValue::parse(r#"{"items":[1,2,3]}"#).unwrap(),
            JsonValue::Object(vec![(
                "items".to_string(),
                JsonValue::Array(vec![
                    JsonValue::Number(1.0),
                    JsonValue::Number(2.0),
                    JsonValue::Number(3.0),
                ])
            )])
        );
    }

    // --- Serialize / Display ---
    #[test]
    fn test_display_null() {
        assert_eq!(JsonValue::Null.to_string(), "null");
    }

    #[test]
    fn test_display_bool_true() {
        assert_eq!(JsonValue::Bool(true).to_string(), "true");
    }

    #[test]
    fn test_display_bool_false() {
        assert_eq!(JsonValue::Bool(false).to_string(), "false");
    }

    #[test]
    fn test_display_integer() {
        assert_eq!(JsonValue::Number(42.0).to_string(), "42");
    }

    #[test]
    fn test_display_float() {
        assert_eq!(JsonValue::Number(3.14).to_string(), "3.14");
    }

    #[test]
    fn test_display_string() {
        assert_eq!(JsonValue::Str("hello".to_string()).to_string(), r#""hello""#);
    }

    #[test]
    fn test_display_string_with_quote() {
        assert_eq!(JsonValue::Str(r#"say "hi""#.to_string()).to_string(), r#""say \"hi\"""#);
    }

    #[test]
    fn test_display_string_with_newline() {
        assert_eq!(JsonValue::Str("a\nb".to_string()).to_string(), r#""a\nb""#);
    }

    #[test]
    fn test_display_array() {
        assert_eq!(
            JsonValue::Array(vec![JsonValue::Number(1.0), JsonValue::Number(2.0)]).to_string(),
            "[1,2]"
        );
    }

    #[test]
    fn test_display_object() {
        assert_eq!(
            JsonValue::Object(vec![("k".to_string(), JsonValue::Str("v".to_string()))]).to_string(),
            r#"{"k":"v"}"#
        );
    }

    // --- Roundtrip ---
    #[test]
    fn test_roundtrip_null() {
        let v = JsonValue::Null;
        assert_eq!(JsonValue::parse(&v.to_string()).unwrap(), v);
    }

    #[test]
    fn test_roundtrip_bool() {
        for v in [JsonValue::Bool(true), JsonValue::Bool(false)] {
            assert_eq!(JsonValue::parse(&v.to_string()).unwrap(), v);
        }
    }

    #[test]
    fn test_roundtrip_number() {
        for v in [JsonValue::Number(42.0), JsonValue::Number(-3.14), JsonValue::Number(0.0)] {
            assert_eq!(JsonValue::parse(&v.to_string()).unwrap(), v);
        }
    }

    #[test]
    fn test_roundtrip_string() {
        for s in ["hello", "say \"hi\"", "line1\nline2", "tab\there"] {
            let v = JsonValue::Str(s.to_string());
            assert_eq!(JsonValue::parse(&v.to_string()).unwrap(), v);
        }
    }

    #[test]
    fn test_roundtrip_array() {
        let v = JsonValue::Array(vec![
            JsonValue::Number(1.0),
            JsonValue::Str("two".to_string()),
            JsonValue::Bool(false),
            JsonValue::Null,
        ]);
        assert_eq!(JsonValue::parse(&v.to_string()).unwrap(), v);
    }

    #[test]
    fn test_roundtrip_complex_object() {
        let v = JsonValue::Object(vec![
            ("name".to_string(), JsonValue::Str("Claude".to_string())),
            ("version".to_string(), JsonValue::Number(3.0)),
            ("active".to_string(), JsonValue::Bool(true)),
            ("tags".to_string(), JsonValue::Array(vec![
                JsonValue::Str("ai".to_string()),
                JsonValue::Str("assistant".to_string()),
            ])),
        ]);
        assert_eq!(JsonValue::parse(&v.to_string()).unwrap(), v);
    }

    // --- Accessor methods ---
    #[test]
    fn test_get_existing_key() {
        let v = JsonValue::parse(r#"{"model":"claude-3","temperature":1}"#).unwrap();
        assert_eq!(v.get("model"), Some(&JsonValue::Str("claude-3".to_string())));
    }

    #[test]
    fn test_get_missing_key() {
        let v = JsonValue::parse(r#"{"a":1}"#).unwrap();
        assert_eq!(v.get("b"), None);
    }

    #[test]
    fn test_get_on_non_object() {
        assert_eq!(JsonValue::Null.get("key"), None);
        assert_eq!(JsonValue::Bool(true).get("key"), None);
    }

    #[test]
    fn test_as_str() {
        assert_eq!(JsonValue::Str("hello".to_string()).as_str(), Some("hello"));
        assert_eq!(JsonValue::Null.as_str(), None);
        assert_eq!(JsonValue::Number(1.0).as_str(), None);
    }

    #[test]
    fn test_as_f64() {
        assert_eq!(JsonValue::Number(3.14).as_f64(), Some(3.14));
        assert_eq!(JsonValue::Null.as_f64(), None);
        assert_eq!(JsonValue::Str("1".to_string()).as_f64(), None);
    }

    #[test]
    fn test_as_bool() {
        assert_eq!(JsonValue::Bool(true).as_bool(), Some(true));
        assert_eq!(JsonValue::Bool(false).as_bool(), Some(false));
        assert_eq!(JsonValue::Null.as_bool(), None);
    }

    #[test]
    fn test_as_array() {
        let arr = vec![JsonValue::Number(1.0)];
        assert!(JsonValue::Array(arr).as_array().is_some());
        assert_eq!(JsonValue::Null.as_array(), None);
    }

    #[test]
    fn test_as_object() {
        let obj = vec![("k".to_string(), JsonValue::Null)];
        assert!(JsonValue::Object(obj).as_object().is_some());
        assert_eq!(JsonValue::Null.as_object(), None);
    }

    #[test]
    fn test_accessor_chain() {
        let v = JsonValue::parse(r#"{"user":{"name":"Alice","age":30}}"#).unwrap();
        let name = v.get("user")
            .and_then(|u| u.get("name"))
            .and_then(|n| n.as_str());
        assert_eq!(name, Some("Alice"));
    }

    // --- Error cases ---
    #[test]
    fn test_parse_invalid_json() {
        assert!(JsonValue::parse("not json").is_err());
    }

    #[test]
    fn test_parse_unclosed_array() {
        assert!(JsonValue::parse("[1,2").is_err());
    }

    #[test]
    fn test_parse_unclosed_object() {
        assert!(JsonValue::parse(r#"{"a":1"#).is_err());
    }

    #[test]
    fn test_parse_trailing_garbage() {
        assert!(JsonValue::parse("null extra").is_err());
    }
}
