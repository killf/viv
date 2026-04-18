use viv::tui::lang_profiles::select_profile;

#[test]
fn select_rust_profile() {
    let p = select_profile(Some("rust"));
    assert_eq!(p.name, "rust");
    assert!(
        p.keywords.contains(&"fn"),
        "rust profile should have 'fn' keyword"
    );
    assert!(
        p.lifetime_prefix,
        "rust profile should have lifetime_prefix = true"
    );
}

#[test]
fn select_rs_alias() {
    let p = select_profile(Some("rs"));
    assert_eq!(p.name, "rust");
}

#[test]
fn select_python_profile() {
    let p = select_profile(Some("python"));
    assert_eq!(p.name, "python");
    assert!(
        p.keywords.contains(&"def"),
        "python profile should have 'def' keyword"
    );
    assert!(
        p.triple_quote,
        "python profile should have triple_quote = true"
    );
    assert_eq!(
        p.line_comments,
        &["#"],
        "python profile should use '#' for line comments"
    );
}

#[test]
fn select_js_profile() {
    let p = select_profile(Some("javascript"));
    assert_eq!(p.name, "javascript");
    assert!(
        p.template_literal,
        "js profile should have template_literal = true"
    );
}

#[test]
fn select_typescript_alias() {
    let p = select_profile(Some("typescript"));
    assert_eq!(p.name, "javascript");
}

#[test]
fn select_go_profile() {
    let p = select_profile(Some("go"));
    assert_eq!(p.name, "go");
    assert!(
        p.type_starts_upper,
        "go profile should have type_starts_upper = true"
    );
}

#[test]
fn select_shell_profile() {
    let p = select_profile(Some("bash"));
    assert_eq!(p.name, "shell");
    assert!(
        p.keywords.contains(&"fi"),
        "shell profile should have 'fi' keyword"
    );
}

#[test]
fn select_json_profile() {
    let p = select_profile(Some("json"));
    assert_eq!(p.name, "json");
    assert!(
        p.line_comments.is_empty(),
        "json profile should have no line comments"
    );
}

#[test]
fn select_unknown_returns_generic() {
    let p = select_profile(Some("unknownlang"));
    assert_eq!(p.name, "generic");
}

#[test]
fn select_none_returns_generic() {
    let p = select_profile(None);
    assert_eq!(p.name, "generic");
}
