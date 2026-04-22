use viv::tui::live_region::{BlockState, LiveBlock, LiveRegion};
use viv::tui::content::{InlineSpan, MarkdownNode};
use viv::core::terminal::size::TermSize;
use viv::core::terminal::backend::TestBackend;
use viv::tui::input::InputMode;
use viv::tui::status::StatusContext;

#[test]
fn new_region_has_no_blocks_and_zero_last_live_rows() {
    let region = LiveRegion::new(TermSize { cols: 80, rows: 24 });
    assert_eq!(region.block_count(), 0);
    assert_eq!(region.last_live_rows(), 0);
}

#[test]
fn push_live_block_appends_with_live_state() {
    let mut region = LiveRegion::new(TermSize { cols: 80, rows: 24 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("hello".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Live });
    assert_eq!(region.block_count(), 1);
}

#[test]
fn mark_last_markdown_committing_transitions_state() {
    let mut region = LiveRegion::new(TermSize { cols: 80, rows: 24 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("hi".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Live });
    region.mark_last_markdown_committing();
    assert_eq!(region.state_at(0), Some(BlockState::Committing));
}

#[test]
fn commit_text_clears_live_region_then_writes_line() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let mut backend = TestBackend::new(40, 10);
    region.set_last_live_rows_for_test(3);
    region.commit_text(&mut backend, "> hello world").unwrap();

    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(out.starts_with("\x1b[3A\x1b[0J"));
    assert!(out.contains("> hello world"));
    assert!(out.ends_with("\n"));
    assert_eq!(region.last_live_rows(), 0);
}

#[test]
fn commit_text_with_zero_live_rows_skips_cursor_up() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let mut backend = TestBackend::new(40, 10);
    region.commit_text(&mut backend, "hi").unwrap();
    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(!out.contains("\x1b[0A"));
    assert!(out.contains("hi\n"));
}

#[test]
fn commit_pending_writes_markdown_then_removes_block() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("hello".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Committing });
    let mut backend = TestBackend::new(40, 10);
    region.commit_pending(&mut backend).unwrap();

    assert_eq!(region.block_count(), 0);
    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(out.contains("hello"), "got {:?}", out);
    assert!(out.ends_with("\n"));
}

#[test]
fn commit_pending_leaves_live_blocks_untouched() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("staying".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Live });
    let mut backend = TestBackend::new(40, 10);
    region.commit_pending(&mut backend).unwrap();
    assert_eq!(region.block_count(), 1);
}

#[test]
fn paint_returns_cursor_inside_input_and_updates_last_live_rows() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let ctx = StatusContext {
        cwd: "~/p".into(), branch: None, model: "m".into(),
        input_tokens: 0, output_tokens: 0,
        spinner_frame: None, spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(40, 10);
    let cur = region.paint(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();
    assert!(region.last_live_rows() >= 4);
    assert!(cur.row < 10);
}

#[test]
fn paint_includes_in_flight_markdown_block() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("streaming…".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Live });
    let ctx = StatusContext {
        cwd: "~/p".into(), branch: None, model: "m".into(),
        input_tokens: 0, output_tokens: 0,
        spinner_frame: None, spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(40, 10);
    region.paint(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();
    assert!(region.last_live_rows() >= 5);
}

#[test]
fn frame_commits_then_paints_and_returns_cursor() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("done".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Committing });
    let ctx = StatusContext {
        cwd: "~/p".into(), branch: None, model: "m".into(),
        input_tokens: 0, output_tokens: 0,
        spinner_frame: None, spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(40, 10);
    let cur = region.frame(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();

    assert_eq!(region.block_count(), 0);
    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(out.contains("done"));
    assert!(cur.row < 10);
    assert!(region.last_live_rows() > 0);
}

#[test]
fn drop_trailing_live_markdown_removes_only_trailing_live() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("a".into())],
    }];
    region.push_live_block(LiveBlock::Markdown {
        nodes: nodes.clone(), state: BlockState::Committing,
    });
    region.push_live_block(LiveBlock::Markdown {
        nodes, state: BlockState::Live,
    });
    region.drop_trailing_live_markdown();
    assert_eq!(region.block_count(), 1);
    assert_eq!(region.state_at(0), Some(BlockState::Committing));
}
