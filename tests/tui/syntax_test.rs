use viv::tui::syntax::{TokenKind, tokenize};

#[test]
fn tokenize_keyword() {
    let tokens = tokenize("fn", Some("rust"));
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Keyword);
    assert_eq!(tokens[0].text, "fn");
}

#[test]
fn tokenize_string_double_quote() {
    let tokens = tokenize("\"hello\"", Some("rust"));
    let string_tok = tokens.iter().find(|t| t.kind == TokenKind::String);
    assert!(string_tok.is_some(), "should have a String token");
    assert_eq!(string_tok.unwrap().text, "\"hello\"");
}

#[test]
fn tokenize_line_comment_slash() {
    let tokens = tokenize("// comment", Some("rust"));
    assert_eq!(tokens[0].kind, TokenKind::Comment);
    assert_eq!(tokens[0].text, "// comment");
}

#[test]
fn tokenize_number() {
    let tokens = tokenize("42", Some("rust"));
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Number);
    assert_eq!(tokens[0].text, "42");
}

#[test]
fn tokenize_hex_number() {
    let tokens = tokenize("0xFF", Some("rust"));
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Number);
    assert_eq!(tokens[0].text, "0xFF");
}

#[test]
fn tokenize_type_uppercase() {
    let tokens = tokenize("String", Some("rust"));
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::Type);
}

#[test]
fn tokenize_function_call() {
    let tokens = tokenize("foo()", Some("rust"));
    let fn_tok = tokens.iter().find(|t| t.text == "foo");
    assert!(fn_tok.is_some(), "should find 'foo' token");
    assert_eq!(fn_tok.unwrap().kind, TokenKind::Function);
}

#[test]
fn tokenize_operator() {
    let tokens = tokenize("a + b", Some("rust"));
    let op_tok = tokens.iter().find(|t| t.kind == TokenKind::Operator);
    assert!(op_tok.is_some(), "should have an Operator token");
    assert_eq!(op_tok.unwrap().text, "+");
}

#[test]
fn tokenize_rust_lifetime() {
    let tokens = tokenize("'a", Some("rust"));
    assert_eq!(tokens[0].kind, TokenKind::Lifetime);
    assert_eq!(tokens[0].text, "'a");
}

#[test]
fn tokenize_rust_attribute() {
    let tokens = tokenize("#[derive(Debug)]", Some("rust"));
    let attr_tok = tokens.iter().find(|t| t.kind == TokenKind::Attribute);
    assert!(attr_tok.is_some(), "should have an Attribute token");
    assert_eq!(attr_tok.unwrap().text, "#[derive(Debug)]");
}

#[test]
fn tokenize_python_comment() {
    let tokens = tokenize("# comment", Some("python"));
    assert_eq!(tokens[0].kind, TokenKind::Comment);
    assert_eq!(tokens[0].text, "# comment");
}

#[test]
fn tokenize_python_hash_not_comment_in_rust() {
    let tokens = tokenize("#[test]", Some("rust"));
    // In rust, '#[...]' is an attribute, not a comment
    let comment_tok = tokens.iter().find(|t| t.kind == TokenKind::Comment);
    assert!(
        comment_tok.is_none(),
        "#[test] in rust should not be a Comment"
    );
    let attr_tok = tokens.iter().find(|t| t.kind == TokenKind::Attribute);
    assert!(attr_tok.is_some(), "#[test] in rust should be an Attribute");
}

#[test]
fn tokenize_block_comment() {
    let tokens = tokenize("/* block */", Some("rust"));
    assert_eq!(tokens[0].kind, TokenKind::Comment);
    assert_eq!(tokens[0].text, "/* block */");
}

#[test]
fn tokenize_python_triple_quote() {
    let tokens = tokenize("\"\"\"docstring\"\"\"", Some("python"));
    let string_tok = tokens.iter().find(|t| t.kind == TokenKind::String);
    assert!(
        string_tok.is_some(),
        "should have a String token for triple-quoted string"
    );
    assert_eq!(string_tok.unwrap().text, "\"\"\"docstring\"\"\"");
}

#[test]
fn tokenize_js_template_literal() {
    let tokens = tokenize("`hello`", Some("javascript"));
    let string_tok = tokens.iter().find(|t| t.kind == TokenKind::String);
    assert!(
        string_tok.is_some(),
        "should have a String token for template literal"
    );
    assert_eq!(string_tok.unwrap().text, "`hello`");
}
