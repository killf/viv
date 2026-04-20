//! Foldable ToolCall widget — shows a tool invocation in the conversation history.
//!
//! Folded (default): single row with icon, name, summary, and status.
//! Expanded: header row + rounded-border block containing the raw input JSON.

use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::tui::block::{Block, BorderStyle};
use crate::tui::widget::{StatefulWidget, Widget};

// ── Theme colors ────────────────────────────────────────────────────────────

const FOCUS_BAR_COLOR: Color = Color::Rgb(177, 185, 249); // suggestion / periwinkle
const DIM_COLOR: Color = Color::Rgb(136, 136, 136); // dim gray
const TEXT_COLOR: Color = Color::Rgb(255, 255, 255); // white
const SUCCESS_COLOR: Color = Color::Rgb(78, 186, 101); // green
const ERROR_COLOR: Color = Color::Rgb(171, 43, 63); // red
const BORDER_COLOR: Color = Color::Rgb(80, 80, 80); // dark gray for input block border

// ── ToolStatus ───────────────────────────────────────────────────────────────

/// The execution state of a tool call.
#[derive(Debug, Clone)]
pub enum ToolStatus {
    Running,
    Success { summary: String },
    Error { message: String },
}

// ── ToolCallState ────────────────────────────────────────────────────────────

/// Mutable state for a [`ToolCallWidget`].
#[derive(Debug, Clone)]
pub struct ToolCallState {
    /// When true, only the single-line header is rendered.
    pub folded: bool,
    /// Current execution status.
    pub status: ToolStatus,
    /// Vertical scroll offset for the expanded input view (future use).
    pub output_scroll: u16,
    /// Timestamp when this tool call entered Running state (used for breath animation).
    pub running_start: Option<std::time::Instant>,
}

impl ToolCallState {
    /// Create state for an in-progress tool call.
    pub fn new_running() -> Self {
        ToolCallState {
            folded: true,
            status: ToolStatus::Running,
            output_scroll: 0,
            running_start: Some(std::time::Instant::now()),
        }
    }

    /// Create state for a successful tool call.
    pub fn new_success(summary: String) -> Self {
        ToolCallState {
            folded: true,
            status: ToolStatus::Success { summary },
            output_scroll: 0,
            running_start: None,
        }
    }

    /// Create state for a failed tool call.
    pub fn new_error(message: String) -> Self {
        ToolCallState {
            folded: true,
            status: ToolStatus::Error { message },
            output_scroll: 0,
            running_start: None,
        }
    }

    /// Toggle between folded and expanded view.
    pub fn toggle_fold(&mut self) {
        self.folded = !self.folded;
    }
}

// ── ToolCallWidget ───────────────────────────────────────────────────────────

/// A widget that renders a single tool invocation.
pub struct ToolCallWidget<'a> {
    name: &'a str,
    input_summary: &'a str,
    input_raw: &'a str,
    focused: bool,
}

impl<'a> ToolCallWidget<'a> {
    /// Create a new widget (unfocused by default).
    pub fn new(name: &'a str, input_summary: &'a str, input_raw: &'a str) -> Self {
        ToolCallWidget {
            name,
            input_summary,
            input_raw,
            focused: false,
        }
    }

    /// Builder: set focus state.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    // ── Breathing animation helpers ──────────────────────────────────────────

    /// Returns a 0..1 alpha that oscillates on a 1-second period using sine.
    fn breath_alpha(running_start: std::time::Instant) -> f32 {
        let elapsed = running_start.elapsed().as_millis() as f32;
        let phase =
            (elapsed % 1000.0) / 1000.0 * 2.0 * std::f32::consts::PI;
        phase.sin() * 0.5 + 0.5 // 0..1, period 1 s
    }

    /// Linearly interpolate between two 8-bit color components.
    fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
        (a as f32 + (b as f32 - a as f32) * t) as u8
    }

    // ── Header row rendering ─────────────────────────────────────────────────

    /// Render the single header row at `y` within `area`.
    fn render_header(&self, area: Rect, buf: &mut Buffer, state: &ToolCallState) {
        if area.is_empty() {
            return;
        }

        let y = area.y;

        // Apply breathing background for Running state
        if matches!(state.status, ToolStatus::Running) {
            if let Some(start) = state.running_start {
                let t = Self::breath_alpha(start);
                // dark: (15,15,25) -> light: (35,30,50) interpolated by t
                let bg = Color::Rgb(
                    Self::lerp_channel(15, 35, t),
                    Self::lerp_channel(15, 30, t),
                    Self::lerp_channel(25, 50, t),
                );
                for x in area.x..(area.x + area.width) {
                    let cell = buf.get_mut(x, y);
                    cell.bg = Some(bg);
                }
            }
        }

        let mut x = area.x;
        let max_x = area.x + area.width;

        // Column 0: focus bar (1 col)
        if self.focused {
            buf.set_str(x, y, "┃", Some(FOCUS_BAR_COLOR), false);
        } else {
            buf.set_str(x, y, " ", None, false);
        }
        x += 1;

        if x >= max_x {
            return;
        }

        // Column 1: gear icon + space (2 cols: "⚙ ")
        buf.set_str(x, y, "⚙ ", Some(DIM_COLOR), false);
        x += 2;

        if x >= max_x {
            return;
        }

        // Tool name in white bold
        let name_len = self.name.len() as u16;
        buf.set_str(x, y, self.name, Some(TEXT_COLOR), true);
        x += name_len;

        if x >= max_x {
            return;
        }

        // Space separator
        buf.set_str(x, y, " ", None, false);
        x += 1;

        if x >= max_x {
            return;
        }

        // Build the status string for right-alignment
        let status_str = match &state.status {
            ToolStatus::Running => "⚙ running...".to_string(),
            ToolStatus::Success { summary } => format!("✓ {}", summary),
            ToolStatus::Error { message } => format!("✗ {}", message),
        };
        let status_len = status_str.chars().count() as u16;

        // Available space between current x and where the status will start
        let status_start = max_x.saturating_sub(status_len);

        // Summary — fills from x to status_start, truncated if needed
        if x < status_start {
            let avail = (status_start - x) as usize;
            let summary_display = truncate_str(self.input_summary, avail);
            buf.set_str(x, y, summary_display, Some(DIM_COLOR), false);
            x += summary_display.chars().count() as u16;
        }

        // Padding to push status to the right
        while x < status_start {
            buf.set_str(x, y, " ", None, false);
            x += 1;
        }

        if x >= max_x {
            return;
        }

        // Status right-aligned
        let status_color = match &state.status {
            ToolStatus::Running => DIM_COLOR,
            ToolStatus::Success { .. } => SUCCESS_COLOR,
            ToolStatus::Error { .. } => ERROR_COLOR,
        };
        buf.set_str(x, y, &status_str, Some(status_color), false);
    }

    // ── Expanded input block ─────────────────────────────────────────────────

    /// Render the input block below the header row.
    fn render_expanded(&self, area: Rect, buf: &mut Buffer, _state: &ToolCallState) {
        if area.is_empty() || area.height < 3 {
            return;
        }

        // Indent by 1 column (skip the focus bar column) and leave 1 col on right
        let block_area = if area.width > 2 {
            Rect::new(area.x + 1, area.y, area.width - 1, area.height)
        } else {
            area
        };

        let block = Block::new()
            .title(" input ")
            .border(BorderStyle::Rounded)
            .border_fg(BORDER_COLOR);

        block.render(block_area, buf);

        let inner = block.inner(block_area);
        if inner.is_empty() {
            return;
        }

        // Render input_raw lines inside the block
        let lines: Vec<&str> = self.input_raw.split('\n').collect();
        for (i, line) in lines.iter().enumerate() {
            let row_y = inner.y + i as u16;
            if row_y >= inner.y + inner.height {
                break;
            }
            let display = truncate_str(line, inner.width as usize);
            buf.set_str(inner.x, row_y, display, Some(DIM_COLOR), false);
        }
    }
}

impl<'a> StatefulWidget for ToolCallWidget<'a> {
    type State = ToolCallState;

    fn render(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }

        if state.folded {
            // Only the header row
            let (header_area, _) = area.split_vertical(1);
            self.render_header(header_area, buf, state);
        } else {
            // Header row + expanded input block
            let (header_area, rest) = area.split_vertical(1);
            self.render_header(header_area, buf, state);

            // How many rows for the expanded block?
            // Use min(rest.height, needed) — at least 3 rows (border + 1 content + border)
            if rest.height >= 3 {
                let content_lines = self.input_raw.split('\n').count() as u16;
                // +2 for top/bottom border
                let block_height = (content_lines + 2).min(rest.height);
                let (block_area, _) = rest.split_vertical(block_height);
                self.render_expanded(block_area, buf, state);
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Truncate `s` to at most `max_chars` characters, returning a &str slice.
fn truncate_str(s: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }
    let mut char_count = 0;
    let mut byte_end = s.len();
    for (byte_idx, _) in s.char_indices() {
        if char_count >= max_chars {
            byte_end = byte_idx;
            break;
        }
        char_count += 1;
    }
    &s[..byte_end]
}

// ── extract_input_summary ────────────────────────────────────────────────────

/// Extract a human-readable one-liner from a tool's JSON input.
///
/// Uses simple string searching — no full JSON parser required.
pub fn extract_input_summary(tool_name: &str, input_json: &str) -> String {
    match tool_name {
        "Read" | "Write" | "Edit" => {
            extract_string_field(input_json, "file_path").unwrap_or_default()
        }
        "Bash" => {
            let cmd = extract_string_field(input_json, "command").unwrap_or_default();
            truncate_string(cmd, 60)
        }
        "Grep" | "Glob" => extract_string_field(input_json, "pattern").unwrap_or_default(),
        "WebFetch" => extract_string_field(input_json, "url").unwrap_or_default(),
        "Agent" | "SubAgent" => {
            let desc = extract_string_field(input_json, "description").unwrap_or_default();
            truncate_string(desc, 40)
        }
        _ => {
            // Fall back: take the first string field value
            if let Some(val) = first_string_field_value(input_json) {
                truncate_string(val, 50)
            } else {
                String::new()
            }
        }
    }
}

/// Extract the value of `"key": "value"` from a JSON string without a full parser.
fn extract_string_field<'a>(json: &'a str, key: &str) -> Option<String> {
    // Search for `"key":` (with optional whitespace before the value)
    let needle = format!("\"{}\"", key);
    let key_pos = json.find(&needle)?;
    let after_key = &json[key_pos + needle.len()..];

    // Skip whitespace and the colon
    let after_colon = after_key.trim_start();
    let after_colon = after_colon.strip_prefix(':')?;
    let after_colon = after_colon.trim_start();

    // Expect a quoted string value
    let after_quote = after_colon.strip_prefix('"')?;

    // Read until the closing quote, handling basic escape sequences
    let mut result = String::new();
    let mut chars = after_quote.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(result),
            '\\' => {
                // Skip one escaped character
                if let Some(escaped) = chars.next() {
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        'r' => result.push('\r'),
                        other => result.push(other),
                    }
                }
            }
            other => result.push(other),
        }
    }
    // Unterminated string — return what we have
    Some(result)
}

/// Return the value of the first `"key": "value"` pair in the JSON.
fn first_string_field_value(json: &str) -> Option<String> {
    // Find the first occurrence of `": "` (i.e., key colon space quote)
    let marker = "\": \"";
    let pos = json.find(marker)?;
    let after_quote = &json[pos + marker.len()..];
    let mut result = String::new();
    let mut chars = after_quote.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(result),
            '\\' => {
                if let Some(escaped) = chars.next() {
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        'r' => result.push('\r'),
                        other => result.push(other),
                    }
                }
            }
            other => result.push(other),
        }
    }
    Some(result)
}

/// Truncate an owned String to at most `max_chars` characters.
fn truncate_string(s: String, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s
    } else {
        s.chars().take(max_chars).collect()
    }
}
