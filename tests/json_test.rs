use viv::json::{JsonValue, Number};

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
    assert_eq!(JsonValue::parse("42").unwrap(), JsonValue::Number(Number::Int(42)));
}

#[test]
fn test_parse_negative_integer() {
    assert_eq!(JsonValue::parse("-7").unwrap(), JsonValue::Number(Number::Int(-7)));
}

#[test]
fn test_parse_float() {
    assert_eq!(JsonValue::parse("-3.14").unwrap(), JsonValue::Number(Number::Float(-3.14)));
}

#[test]
fn test_parse_exponent() {
    assert_eq!(JsonValue::parse("1e3").unwrap(), JsonValue::Number(Number::Float(1000.0)));
}

#[test]
fn test_parse_negative_exponent() {
    assert_eq!(JsonValue::parse("2.5e-2").unwrap(), JsonValue::Number(Number::Float(0.025)));
}

#[test]
fn test_parse_zero() {
    assert_eq!(JsonValue::parse("0").unwrap(), JsonValue::Number(Number::Int(0)));
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
            JsonValue::Number(Number::Int(1)),
            JsonValue::Number(Number::Int(2)),
            JsonValue::Number(Number::Int(3)),
        ])
    );
}

#[test]
fn test_parse_array_with_whitespace() {
    assert_eq!(
        JsonValue::parse("[ 1 , 2 ]").unwrap(),
        JsonValue::Array(vec![JsonValue::Number(Number::Int(1)), JsonValue::Number(Number::Int(2))])
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
            JsonValue::Number(Number::Int(3)),
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
            ("a".to_string(), JsonValue::Number(Number::Int(1))),
            ("b".to_string(), JsonValue::Number(Number::Int(2))),
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
            JsonValue::Array(vec![JsonValue::Number(Number::Int(1)), JsonValue::Number(Number::Int(2))]),
            JsonValue::Array(vec![JsonValue::Number(Number::Int(3)), JsonValue::Number(Number::Int(4))]),
        ])
    );
}

#[test]
fn test_parse_nested_object() {
    assert_eq!(
        JsonValue::parse(r#"{"outer":{"inner":42}}"#).unwrap(),
        JsonValue::Object(vec![(
            "outer".to_string(),
            JsonValue::Object(vec![("inner".to_string(), JsonValue::Number(Number::Int(42)))])
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
                JsonValue::Number(Number::Int(1)),
                JsonValue::Number(Number::Int(2)),
                JsonValue::Number(Number::Int(3)),
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
    assert_eq!(JsonValue::Number(Number::Int(42)).to_string(), "42");
}

#[test]
fn test_display_float() {
    assert_eq!(JsonValue::Number(Number::Float(3.14)).to_string(), "3.14");
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
        JsonValue::Array(vec![JsonValue::Number(Number::Int(1)), JsonValue::Number(Number::Int(2))]).to_string(),
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
    for v in [JsonValue::Number(Number::Int(42)), JsonValue::Number(Number::Float(-3.14)), JsonValue::Number(Number::Int(0))] {
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
        JsonValue::Number(Number::Int(1)),
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
        ("version".to_string(), JsonValue::Number(Number::Int(3))),
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
    assert_eq!(JsonValue::Number(Number::Int(1)).as_str(), None);
}

#[test]
fn test_as_f64() {
    assert_eq!(JsonValue::Number(Number::Float(3.14)).as_f64(), Some(3.14));
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
    let arr = vec![JsonValue::Number(Number::Int(1))];
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
