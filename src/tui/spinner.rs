//! Spinner animation for indicating "work in progress".
//!
//! Matches Claude Code's aesthetic: rotating braille frames at ~8 FPS (120ms per frame).
//! Verb messages cycle from a curated word list.

/// Unicode braille spinner frames (commonly known as "dots" spinner).
/// 10 frames × 120ms = 1200ms full rotation.
pub const DEFAULT_FRAMES: &[&str] = &[
    "\u{2846}", // ⡆
    "\u{2847}", // ⡇
    "\u{2807}", // ⠇
    "\u{280F}", // ⠏
    "\u{2809}", // ⠉
    "\u{2819}", // ⠙
    "\u{2818}", // ⠘
    "\u{2838}", // ⠸
    "\u{2830}", // ⠰
    "\u{2834}", // ⠴
];

/// A curated list of "working" verbs (subset of Claude Code's SPINNER_VERBS).
pub const VERBS: &[&str] = &[
    "Baking",
    "Brewing",
    "Cogitating",
    "Computing",
    "Considering",
    "Crunching",
    "Deliberating",
    "Elaborating",
    "Formulating",
    "Mulling",
    "Noodling",
    "Percolating",
    "Pondering",
    "Processing",
    "Reflecting",
    "Reticulating",
    "Ruminating",
    "Simmering",
    "Synthesizing",
    "Thinking",
    "Weaving",
    "Whisking",
];

/// A spinner configured with frames and frame duration.
pub struct Spinner {
    frames: &'static [&'static str],
    frame_duration_ms: u64,
}

impl Spinner {
    /// Create a spinner with the default braille frames at 120ms per frame.
    pub fn new() -> Self {
        Spinner { frames: DEFAULT_FRAMES, frame_duration_ms: 120 }
    }

    /// Number of frames in the rotation.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Milliseconds per frame.
    pub fn frame_duration_ms(&self) -> u64 {
        self.frame_duration_ms
    }

    /// Return the frame at time `t_ms` (milliseconds since the spinner started).
    pub fn frame_at(&self, t_ms: u64) -> &'static str {
        let idx = (t_ms / self.frame_duration_ms) as usize % self.frames.len();
        self.frames[idx]
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

/// Deterministic random verb from the [`VERBS`] list, seeded by `seed`.
pub fn random_verb(seed: u64) -> &'static str {
    let idx = (seed as usize) % VERBS.len();
    VERBS[idx]
}
