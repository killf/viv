# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**viv** — 一个自我进化的 AI 编程 Agent。核心理念是"越用越好用"，Agent 能够在使用过程中不断学习和进化。

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo run                # Build and run
cargo test               # Run all tests
cargo test <test_name>   # Run a single test
cargo fmt                # Format code
cargo clippy             # Lint
```

## Architecture

Single-binary Rust project. Entry point: `src/main.rs`. No external dependencies yet.

## 参考项目

* https://github.com/openai/codex
* https://github.com/ultraworkers/claw-code
* https://github.com/crossterm-rs/crossterm
* https://github.com/ratatui/ratatui