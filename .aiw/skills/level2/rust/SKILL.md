---
name: rust
description: Load when working with rust in media-organizer.
level: 2
kind: technology
version: 1.100.0
sources:
  - https://doc.rust-lang.org/cargo/guide/project-layout.html — Rust Project
fetched: 2026-11-24
---

# rust

Keep one Cargo package with src/main.rs and integration tests in tests/. Commit Cargo.lock. Run cargo test --locked, cargo build --locked, cargo fmt --check and cargo clippy --locked. Keep paths as Path and OsStr at filesystem boundaries.
