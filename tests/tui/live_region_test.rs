use viv::tui::live_region::{BlockState, LiveBlock, LiveRegion};
use viv::tui::content::{InlineSpan, MarkdownNode};
use viv::core::terminal::size::TermSize;

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
