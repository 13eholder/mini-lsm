# AGENTS.md - mini-lsm Repository Guide

## Project Overview

This is **mini-lsm**, a teaching project for building a simple key-value storage engine (LSM tree) in Rust. It's a Cargo workspace with 4 crates:

| Crate              | Purpose                          |
| ------------------ | -------------------------------- |
| `mini-lsm`         | Reference solution (weeks 1-2)   |
| `mini-lsm-mvcc`    | Reference solution (week 3 MVCC) |
| `mini-lsm-starter` | Starter code for students        |
| `xtask`            | Build automation utilities       |

## Build / Lint / Test Commands

### Primary Commands (via xtask)

```bash
cargo x check          # fmt + check + test + clippy (workspace)
cargo x scheck         # fmt + check + test + clippy (starter only)
cargo x ci             # check-fmt + check + test + clippy + build book
cargo x install-tools  # install nextest, mdbook, mdbook-toc, cargo-semver-checks
cargo x copy-test --week N --day N   # copy test cases to starter
cargo x sync           # check semver compatibility between starter and solution
cargo x book           # build and serve the book locally
```

### Individual Commands

```bash
cargo fmt                          # Format all code
cargo fmt --check                  # Check formatting
cargo check --all-targets          # Type-check everything
cargo clippy --all-targets         # Run clippy lints
cargo nextest run                  # Run all tests
cargo nextest run <test_name>      # Run a single test (e.g., week1_day1::test_basic)
cargo nextest run week1_day1       # Run all tests in a module
cargo test <test_name>             # Alternative: run single test with built-in runner
```

### Running Binaries

```bash
cargo run --bin mini-lsm-cli-ref           # Run reference CLI
cargo run --bin compaction-simulator-ref   # Run compaction simulator
```

## Code Style

### Toolchain
- **Rust Edition**: 2024
- **Channel**: nightly (see `rust-toolchain.toml`)
- **Formatter**: rustfmt with nightly options (see `rustfmt.toml.nightly`)

### Formatting Rules (from `rustfmt.toml.nightly`)
- `imports_granularity = "Module"` — group imports by module
- `group_imports = "StdExternalCrate"` — order: std, external crates, local crates
- `reorder_imports = true` and `reorder_impl_items = true`
- `wrap_comments = true`, `comment_width = 120`
- `tab_spaces = 4`
- `format_code_in_doc_comments = true`

### Imports
Order imports in three groups separated by blank lines:
1. `std::` / `core::` imports
2. External crate imports (e.g., `anyhow`, `bytes`, `parking_lot`)
3. Local crate imports (`crate::...`)

```rust
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use parking_lot::Mutex;

use crate::key::KeySlice;
use crate::table::SsTable;
```

### Naming Conventions
- **Modules**: `snake_case` (e.g., `lsm_storage`, `mem_table`, `two_merge_iterator`)
- **Types/Structs/Enums**: `PascalCase` (e.g., `LsmStorageState`, `SsTable`, `KeySlice`)
- **Functions/Methods**: `snake_case` (e.g., `put`, `get`, `close`)
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Test functions**: `test_` prefix (e.g., `test_basic_get_put`)

### Error Handling
- Use `anyhow::Result` for fallible functions throughout the codebase
- Use `.context("message")` for adding context to errors
- Public APIs return `anyhow::Result<T>`; internal helpers may use `Result<T>` with implicit anyhow

### Concurrency
- Use `Arc` for shared ownership across threads
- Use `parking_lot::Mutex` and `parking_lot::RwLock` (not std sync primitives)
- Use `Arc<AtomicUsize>` for atomic counters

### Module Organization
- Each module can be a file (`mod.rs` pattern) or a directory with `mod.rs`
- Submodules live in directories matching the parent module name
- Tests live in `src/tests/` directory, one file per week/day (e.g., `week1_day1.rs`)
- Test modules are declared in `src/tests.rs` and gated with `#[cfg(test)]`

### File Headers
Every source file must include the Apache 2.0 license header:

```rust
// Copyright (c) 2022-2025 Alex Chi Z
//
// Licensed under the Apache License, Version 2.0 (the "License");
// ...
```

### Test Patterns
- Tests use the `#[test]` attribute
- Use `tempfile::tempdir()` for temporary test directories
- Test files follow `week{N}_day{N}.rs` naming convention
- A shared `harness.rs` provides common test utilities

### Key Types
- `Bytes` (from `bytes` crate) for key/value data
- `KeySlice` for key references
- `Bound<&[u8]>` for range queries

## Documents Writting
- Update `index.md` when modify or create file in `.docs/`