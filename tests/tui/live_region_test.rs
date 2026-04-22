use viv::tui::live_region::{BlockState, LiveBlock, LiveRegion};
use viv::tui::content::{InlineSpan, MarkdownNode};
use viv::core::terminal::size::TermSize;
use viv::core::terminal::backend::TestBackend;

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
