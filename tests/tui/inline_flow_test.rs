use viv::core::terminal::backend::TestBackend;
use viv::core::terminal::size::TermSize;
use viv::tui::content::{InlineSpan, MarkdownNode};
use viv::tui::input::InputMode;
use viv::tui::live_region::{BlockState, LiveBlock, LiveRegion};
use viv::tui::status::StatusContext;

#[test]
fn scripted_flow_produces_scrollback_and_live_region() {
    let mut region = LiveRegion::new(TermSize { cols: 60, rows: 20 });
    let mut backend = TestBackend::new(60, 20);

    // 1. User message commit.
    region.commit_text(&mut backend, "> hello viv").unwrap();

    // 2. Streaming assistant markdown: two closed paragraphs.
    for para in ["Sure, here's what I can do.", "Let me check the file."] {
        region.push_live_block(LiveBlock::Markdown {
            nodes: vec![MarkdownNode::Paragraph {
                spans: vec![InlineSpan::Text(para.into())],
            }],
            state: BlockState::Committing,
        });
    }

    // 3. Frame: commits both + paints input/status.
    let ctx = StatusContext {
        cwd: "~/p".into(),
        branch: Some("main".into()),
        model: "claude-sonnet-4-6".into(),
        input_tokens: 123,
        output_tokens: 456,
        spinner_frame: None,
        spinner_verb: String::new(),
    };
    let cur = region
        .frame(&mut backend, "", 0, InputMode::Chat, &ctx)
        .unwrap();

    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(out.contains("hello viv"), "stdout missing user line");
    assert!(out.contains("Sure, here's what I can do."), "stdout missing para 1");
    assert!(out.contains("Let me check the file."), "stdout missing para 2");
    assert!(out.contains("claude-sonnet-4-6"), "stdout missing model name");
    assert_eq!(region.block_count(), 0, "committing blocks should be gone");
    assert!(cur.row < 20);
}
