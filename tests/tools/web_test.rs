use viv::tools::web::html_to_markdown;

#[test]
fn html_to_markdown_headings() {
    let result = html_to_markdown("<h1>Title</h1>");
    assert!(result.contains("# Title"), "Got: {}", result);

    let result = html_to_markdown("<h2>Sub</h2>");
    assert!(result.contains("## Sub"), "Got: {}", result);

    let result = html_to_markdown("<h3>Deep</h3>");
    assert!(result.contains("### Deep"), "Got: {}", result);
}

#[test]
fn html_to_markdown_links() {
    let result = html_to_markdown(r#"<a href="https://example.com">Click</a>"#);
    assert!(
        result.contains("[Click](https://example.com)"),
        "Got: {}",
        result
    );
}

#[test]
fn html_to_markdown_emphasis() {
    assert!(html_to_markdown("<strong>bold</strong>").contains("**bold**"));
    assert!(html_to_markdown("<b>bold</b>").contains("**bold**"));
    assert!(html_to_markdown("<em>italic</em>").contains("*italic*"));
    assert!(html_to_markdown("<i>italic</i>").contains("*italic*"));
}

#[test]
fn html_to_markdown_lists() {
    let html = "<ul><li>one</li><li>two</li></ul>";
    let md = html_to_markdown(html);
    assert!(md.contains("- one"), "Got: {}", md);
    assert!(md.contains("- two"), "Got: {}", md);
}

#[test]
fn html_to_markdown_code() {
    assert!(html_to_markdown("<code>x + 1</code>").contains("`x + 1`"));
}

#[test]
fn html_to_markdown_pre() {
    let md = html_to_markdown("<pre>fn main() {\n    println!(\"hi\");\n}</pre>");
    assert!(md.contains("```"), "Got: {}", md);
    assert!(md.contains("fn main()"), "Got: {}", md);
}

#[test]
fn html_to_markdown_paragraphs() {
    let md = html_to_markdown("<p>First</p><p>Second</p>");
    assert!(
        md.contains("First") && md.contains("Second"),
        "Got: {}",
        md
    );
}

#[test]
fn html_to_markdown_strips_script() {
    let md = html_to_markdown("<p>Hello</p><script>alert('xss')</script><p>World</p>");
    assert!(
        !md.contains("alert"),
        "Script content should be stripped: {}",
        md
    );
    assert!(md.contains("Hello") && md.contains("World"));
}

#[test]
fn html_to_markdown_strips_style() {
    let md = html_to_markdown("<p>Hello</p><style>body { color: red; }</style><p>World</p>");
    assert!(
        !md.contains("color"),
        "Style content should be stripped: {}",
        md
    );
    assert!(md.contains("Hello") && md.contains("World"));
}

#[test]
fn html_to_markdown_entities() {
    assert!(html_to_markdown("&amp; &lt; &gt;").contains("& < >"));
    assert!(html_to_markdown("&quot;hello&quot;").contains("\"hello\""));
    assert!(html_to_markdown("a&nbsp;b").contains("a b"));
}

#[test]
fn html_to_markdown_br() {
    let md = html_to_markdown("line1<br>line2");
    assert!(md.contains("line1\nline2"), "Got: {:?}", md);
}

#[test]
fn html_to_markdown_whitespace_collapse() {
    let md = html_to_markdown("hello    world");
    assert!(md.contains("hello world"), "Got: {:?}", md);
}

#[test]
fn html_to_markdown_mixed() {
    let html = r#"<h1>Title</h1><p>Some <strong>bold</strong> and <em>italic</em> text with <a href="https://example.com">a link</a>.</p><ul><li>Item 1</li><li>Item 2</li></ul>"#;
    let md = html_to_markdown(html);
    assert!(md.contains("# Title"), "Missing heading: {}", md);
    assert!(md.contains("**bold**"), "Missing bold: {}", md);
    assert!(md.contains("*italic*"), "Missing italic: {}", md);
    assert!(
        md.contains("[a link](https://example.com)"),
        "Missing link: {}",
        md
    );
    assert!(md.contains("- Item 1"), "Missing list: {}", md);
}
