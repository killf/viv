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
    assert!(out.ends_with("\r\n"));
    assert_eq!(region.last_live_rows(), 0);
}

#[test]
fn commit_text_with_zero_live_rows_skips_cursor_up() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let mut backend = TestBackend::new(40, 10);
    region.commit_text(&mut backend, "hi").unwrap();
    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(!out.contains("\x1b[0A"));
    assert!(out.contains("hi\r\n"));
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
    assert!(out.ends_with("\r\n"));
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

#[test]
fn finish_last_running_tool_marks_committing_with_output() {
    use viv::tui::tool_call::ToolCallState;
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    region.push_live_block(LiveBlock::ToolCall {
        id: 0,
        name: "Bash".into(),
        input: "ls".into(),
        output: None,
        error: None,
        tc_state: ToolCallState::new_running(),
        state: BlockState::Live,
    });
    region.finish_last_running_tool(Some("drwx----".into()), None);
    assert_eq!(region.state_at(0), Some(BlockState::Committing));
}

#[test]
fn paint_renders_permission_menu_multiple_rows() {
    use viv::tui::permission::PermissionState;
    let mut region = LiveRegion::new(TermSize { cols: 60, rows: 20 });
    region.push_live_block(LiveBlock::PermissionPrompt {
        tool: "Bash".into(),
        input: "rm -rf /".into(),
        menu: PermissionState::new(),
    });
    let ctx = StatusContext {
        cwd: "~/p".into(), branch: None, model: "m".into(),
        input_tokens: 0, output_tokens: 0,
        spinner_frame: None, spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(60, 20);
    region.paint(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();
    // Menu widget is multi-row; live region should occupy more rows than the
    // input+status baseline (3 input + 1 status + 1 blank = 5).
    assert!(region.last_live_rows() > 5,
        "expected live region to include multi-row permission menu, got {} rows",
        region.last_live_rows());
}

#[test]
fn paint_erases_stale_chars_when_input_shrinks() {
    // Regression: after the user deletes characters from the input, the new
    // (shorter) row must erase the tail of the previous frame so stale chars
    // don't linger on screen. `buffer_rows_to_ansi` trims trailing blanks, so
    // `paint` must append an explicit erase-in-line sequence (`\x1b[K` or
    // `\x1b[2K`) on each row, or pad rows to full width.
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let ctx = StatusContext {
        cwd: "~/p".into(), branch: None, model: "m".into(),
        input_tokens: 0, output_tokens: 0,
        spinner_frame: None, spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(40, 10);

    region.paint(&mut backend, "hello world", 11, InputMode::Chat, &ctx).unwrap();
    backend.output.clear();
    region.paint(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();

    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(
        out.contains("\x1b[K") || out.contains("\x1b[0K") || out.contains("\x1b[2K"),
        "paint must emit erase-in-line when content shrinks; got {:?}",
        out
    );
}

#[test]
fn paint_clears_rows_above_new_top_when_live_region_shrinks() {
    // Regression: when vertical size of the live region shrinks (e.g. user
    // deletes a newline so the input box gets shorter), the old rows above
    // the new top_y must be cleared so they don't keep showing stale content.
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 20 });
    let ctx = StatusContext {
        cwd: "~/p".into(), branch: None, model: "m".into(),
        input_tokens: 0, output_tokens: 0,
        spinner_frame: None, spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(40, 20);

    // 5 logical lines → input_h = 7, live_rows = 8.
    region.paint(&mut backend, "a\nb\nc\nd\ne", 9, InputMode::Chat, &ctx).unwrap();
    let tall_rows = region.last_live_rows();
    backend.output.clear();

    // 1 logical line → input_h = 3, live_rows = 4. Frame shrinks vertically.
    region.paint(&mut backend, "a", 1, InputMode::Chat, &ctx).unwrap();
    let short_rows = region.last_live_rows();
    assert!(tall_rows > short_rows, "setup: tall {tall_rows} should exceed short {short_rows}");

    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(
        out.contains("\x1b[K") || out.contains("\x1b[0K") || out.contains("\x1b[2K") || out.contains("\x1b[0J"),
        "paint must erase the old live region when it shrinks; got {:?}",
        out
    );
}
