# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**viv** — 一个自我进化的 AI 编程 Agent。核心理念是"越用越好用"，Agent 能够在使用过程中不断学习和进化。Rust 实现，nightly edition 2024，早期阶段(v0.1.0)。

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
