use crate::tui::lang_profiles::{LangProfile, select_profile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    String,
    Comment,
    Number,
    Type,
    Function,
    Operator,
    Punctuation,
    Attribute,
    Lifetime,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: std::string::String,
}

impl Token {
    fn new(kind: TokenKind, text: std::string::String) -> Self {
        Token { kind, text }
    }
}

pub fn tokenize(line: &str, language: Option<&str>) -> Vec<Token> {
    let profile = select_profile(language);
    let chars: Vec<char> = line.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        // Whitespace
        if chars[i].is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(Token::new(
                TokenKind::Plain,
                chars[start..i].iter().collect(),
            ));
            continue;
        }

        // Try line comments
        if let Some(tok) = try_line_comment(&chars, i, profile) {
            i += tok.text.len();
            tokens.push(tok);
            continue;
        }

        // Try block comment
        if let Some(tok) = try_block_comment(&chars, i, profile) {
            i += tok.text.chars().count();
            tokens.push(tok);
            continue;
        }

        // Attribute prefix
        if let Some(attr_ch) = profile.attribute_prefix
            && chars[i] == attr_ch
        {
            if attr_ch == '#' {
                if let Some(tok) = try_rust_attribute(&chars, i) {
                    i += tok.text.chars().count();
                    tokens.push(tok);
                    continue;
                }
            } else if attr_ch == '@'
                && let Some(tok) = try_at_attribute(&chars, i)
            {
                i += tok.text.chars().count();
                tokens.push(tok);
                continue;
            }
        }

        // Lifetime (Rust: 'a, 'b, etc.)
        if profile.lifetime_prefix
            && chars[i] == '\''
            && let Some(tok) = try_lifetime(&chars, i)
        {
            i += tok.text.chars().count();
            tokens.push(tok);
            continue;
        }

        // Strings
        if let Some(tok) = try_string(&chars, i, profile) {
            i += tok.text.chars().count();
            tokens.push(tok);
            continue;
        }

        // Numbers
        if chars[i].is_ascii_digit()
            || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let tok = consume_number(&chars, i);
            i += tok.text.chars().count();
            tokens.push(tok);
            continue;
        }

        // Identifiers / keywords
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let tok = consume_identifier(&chars, i, profile);
            i += tok.text.chars().count();
            tokens.push(tok);
            continue;
        }

        // Operators (multi-char first)
        if let Some(tok) = try_operator(&chars, i) {
            i += tok.text.chars().count();
            tokens.push(tok);
            continue;
        }

        // Punctuation
        if is_punctuation(chars[i]) {
            tokens.push(Token::new(TokenKind::Punctuation, chars[i].to_string()));
            i += 1;
            continue;
        }

        // Fallback
        tokens.push(Token::new(TokenKind::Plain, chars[i].to_string()));
        i += 1;
    }

    tokens
}

fn try_line_comment(chars: &[char], i: usize, profile: &LangProfile) -> Option<Token> {
    for prefix in profile.line_comments {
        let prefix_chars: Vec<char> = prefix.chars().collect();
        if chars[i..].starts_with(&prefix_chars) {
            let text: std::string::String = chars[i..].iter().collect();
            return Some(Token::new(TokenKind::Comment, text));
        }
    }
    None
}

fn try_block_comment(chars: &[char], i: usize, profile: &LangProfile) -> Option<Token> {
    let (open, close) = profile.block_comment?;
    let open_chars: Vec<char> = open.chars().collect();
    let close_chars: Vec<char> = close.chars().collect();

    if !chars[i..].starts_with(&open_chars) {
        return None;
    }

    let mut j = i + open_chars.len();
    while j < chars.len() {
        if chars[j..].starts_with(&close_chars) {
            j += close_chars.len();
            let text: std::string::String = chars[i..j].iter().collect();
            return Some(Token::new(TokenKind::Comment, text));
        }
        j += 1;
    }

    // Unclosed block comment: consume to end
    let text: std::string::String = chars[i..].iter().collect();
    Some(Token::new(TokenKind::Comment, text))
}

fn try_rust_attribute(chars: &[char], i: usize) -> Option<Token> {
    // Expect '#' followed optionally by '!' then '['
    if chars[i] != '#' {
        return None;
    }
    let mut j = i + 1;
    // optional '!'
    if j < chars.len() && chars[j] == '!' {
        j += 1;
    }
    if j >= chars.len() || chars[j] != '[' {
        return None;
    }
    j += 1;
    let mut depth = 1usize;
    while j < chars.len() && depth > 0 {
        match chars[j] {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ => {}
        }
        j += 1;
    }
    let text: std::string::String = chars[i..j].iter().collect();
    Some(Token::new(TokenKind::Attribute, text))
}

fn try_at_attribute(chars: &[char], i: usize) -> Option<Token> {
    if chars[i] != '@' {
        return None;
    }
    let mut j = i + 1;
    while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
        j += 1;
    }
    if j == i + 1 {
        return None; // '@' with no identifier after
    }
    let text: std::string::String = chars[i..j].iter().collect();
    Some(Token::new(TokenKind::Attribute, text))
}

fn try_lifetime(chars: &[char], i: usize) -> Option<Token> {
    // 'identifier (but not a char literal)
    if chars[i] != '\'' {
        return None;
    }
    let j = i + 1;
    if j >= chars.len() || !chars[j].is_alphabetic() {
        return None;
    }
    let mut k = j;
    while k < chars.len() && (chars[k].is_alphanumeric() || chars[k] == '_') {
        k += 1;
    }
    // If followed by '\'' it's a char literal, not a lifetime
    if k < chars.len() && chars[k] == '\'' {
        return None;
    }
    let text: std::string::String = chars[i..k].iter().collect();
    Some(Token::new(TokenKind::Lifetime, text))
}

fn try_string(chars: &[char], i: usize, profile: &LangProfile) -> Option<Token> {
    // Raw string (Rust r"...")
    if let Some(raw_prefix) = profile.raw_string {
        let raw_chars: Vec<char> = raw_prefix.chars().collect();
        if !raw_chars.is_empty() && chars[i..].starts_with(&raw_chars) {
            let quote_char = match raw_chars.last() {
                Some(c) => *c,
                None => return None,
            };
            let mut j = i + raw_chars.len();
            while j < chars.len() && chars[j] != quote_char {
                j += 1;
            }
            if j < chars.len() {
                j += 1; // consume closing quote
            }
            let text: std::string::String = chars[i..j].iter().collect();
            return Some(Token::new(TokenKind::String, text));
        }
    }

    // Template literal (JS `...`)
    if profile.template_literal && chars[i] == '`' {
        let mut j = i + 1;
        while j < chars.len() && chars[j] != '`' {
            if chars[j] == '\\' {
                j += 1; // skip escaped char
            }
            j += 1;
        }
        if j < chars.len() {
            j += 1; // consume closing backtick
        }
        let text: std::string::String = chars[i..j].iter().collect();
        return Some(Token::new(TokenKind::String, text));
    }

    // Check if current char is a string quote
    if !profile.string_quotes.contains(&chars[i]) {
        return None;
    }

    let quote = chars[i];

    // Triple quote (Python)
    if profile.triple_quote {
        let triple = [quote, quote, quote];
        if chars[i..].starts_with(&triple) {
            let mut j = i + 3;
            loop {
                if j + 2 < chars.len() && chars[j..j + 3] == triple {
                    j += 3;
                    break;
                }
                if j >= chars.len() {
                    break;
                }
                j += 1;
            }
            let text: std::string::String = chars[i..j].iter().collect();
            return Some(Token::new(TokenKind::String, text));
        }
    }

    // Regular string
    let mut j = i + 1;
    while j < chars.len() && chars[j] != quote {
        if chars[j] == '\\' {
            j += 1; // skip escaped char
        }
        j += 1;
    }
    if j < chars.len() {
        j += 1; // consume closing quote
    }
    let text: std::string::String = chars[i..j].iter().collect();
    Some(Token::new(TokenKind::String, text))
}

fn consume_number(chars: &[char], i: usize) -> Token {
    let mut j = i;

    // Hex, binary, octal
    if chars[j] == '0' && j + 1 < chars.len() {
        match chars[j + 1] {
            'x' | 'X' => {
                j += 2;
                while j < chars.len() && (chars[j].is_ascii_hexdigit() || chars[j] == '_') {
                    j += 1;
                }
                let text: std::string::String = chars[i..j].iter().collect();
                return Token::new(TokenKind::Number, text);
            }
            'b' | 'B' => {
                j += 2;
                while j < chars.len() && (chars[j] == '0' || chars[j] == '1' || chars[j] == '_') {
                    j += 1;
                }
                let text: std::string::String = chars[i..j].iter().collect();
                return Token::new(TokenKind::Number, text);
            }
            'o' | 'O' => {
                j += 2;
                while j < chars.len() && (('0'..='7').contains(&chars[j]) || chars[j] == '_') {
                    j += 1;
                }
                let text: std::string::String = chars[i..j].iter().collect();
                return Token::new(TokenKind::Number, text);
            }
            _ => {}
        }
    }

    // Integer or float
    while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '_') {
        j += 1;
    }
    if j < chars.len() && chars[j] == '.' {
        j += 1;
        while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '_') {
            j += 1;
        }
    }
    // Exponent
    if j < chars.len() && (chars[j] == 'e' || chars[j] == 'E') {
        j += 1;
        if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
            j += 1;
        }
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
    }
    // Suffix (like u32, f64, etc.)
    while j < chars.len() && chars[j].is_alphanumeric() {
        j += 1;
    }

    let text: std::string::String = chars[i..j].iter().collect();
    Token::new(TokenKind::Number, text)
}

fn consume_identifier(chars: &[char], i: usize, profile: &LangProfile) -> Token {
    let mut j = i;
    while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
        j += 1;
    }
    let word: std::string::String = chars[i..j].iter().collect();

    // Check if followed by '(' → Function
    let next_non_space = chars[j..]
        .iter()
        .position(|c| !c.is_whitespace())
        .map(|k| j + k);
    if let Some(next) = next_non_space
        && chars[next] == '('
    {
        return Token::new(TokenKind::Function, word);
    }

    // Keyword check
    if profile.keywords.contains(&word.as_str()) {
        return Token::new(TokenKind::Keyword, word);
    }

    // Type (starts with uppercase)
    if profile.type_starts_upper
        && let Some(first) = word.chars().next()
        && first.is_uppercase()
    {
        return Token::new(TokenKind::Type, word);
    }

    Token::new(TokenKind::Plain, word)
}

fn try_operator(chars: &[char], i: usize) -> Option<Token> {
    // Multi-char operators (longest match first)
    let multi = [
        "->", "=>", "::", "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "+=", "-=", "*=", "/=",
        "%=", "&=", "|=", "^=", "..",
    ];
    for op in &multi {
        let op_chars: Vec<char> = op.chars().collect();
        if chars[i..].starts_with(&op_chars) {
            return Some(Token::new(TokenKind::Operator, op.to_string()));
        }
    }

    // Single-char operators
    let single = [
        '=', '+', '-', '*', '/', '<', '>', '!', '&', '|', '^', '%', '~', ':',
    ];
    if single.contains(&chars[i]) {
        return Some(Token::new(TokenKind::Operator, chars[i].to_string()));
    }

    None
}

fn is_punctuation(ch: char) -> bool {
    matches!(ch, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | '.')
}
