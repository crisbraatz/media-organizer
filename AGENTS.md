# AGENTS.md

This project is managed by **AI Workflow**. These instructions apply to every AI agent (Claude Code, Codex, others).

## Start here
1. Read `.aiw/state.json`. If `currentFeature` is not null, a feature is in progress: resume it instead of starting new work (Claude Code: `/aiw:resume`; Codex: `/prompts:aiw-resume`). Always write both command forms in project files, since any agent may read them.
2. Read `docs/architecture.md` and `docs/business/overview.md` before planning anything.
3. Load the skills in `.aiw/skills/level2/` (stack) and `.aiw/skills/level3/` (business) relevant to the task.

## Project
- Summary: Media Organizer is a free, open-source interactive CLI for people who organize personal photo and video collections on Linux, Windows, and macOS. It previews changes before applying chronological renaming and offers optional duplicate removal and subfolder flattening. The rewrite uses a different language, preserves the behavior documented in the original README, and is invoked as media-organizer.
- Stack: Rust/Cargo 1.100.0, Crossterm 0.29.0, nom-exif 3.8.0 and renamest 0.1.3.
- Architecture: Layered modular monolith
- User language: pt-BR (talk to the user and write business docs in it; code and technical docs in English)

## Commands
- Install: `cargo fetch --locked`
- Run: `cargo run --locked -- --help`
- Test: `cargo test --locked`
- Lint/format: `cargo fmt --all -- --check and cargo clippy --locked --all-targets -- -D warnings`
- Build: `cargo build --locked --release`

## Rules
- Follow the architecture and conventions in `docs/architecture.md`; record new significant decisions as ADRs there.
- Every change ships with tests; lint, build and tests must pass before committing.
- Never commit secrets; configuration comes from environment variables (`.env.example` documents them).
- One commit per feature (`feat(<scope>): <summary>`); intermediate state lives in `.aiw/current/`.
- Branch mode is stored in `.aiw/state.json` (`main` or `branch`), and so is the trunk branch (`trunkBranch`). Never commit project work to any other branch.
