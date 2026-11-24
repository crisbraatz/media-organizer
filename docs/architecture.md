# Architecture

## Stack

| Layer | Technology | Version | Purpose |
| --- | --- | --- | --- |
| Language and build | Rust and Cargo | 1.100.0 toolchain, Edition 2024 | Portable native CLI |
| Terminal | Crossterm | 0.29.0 | Keyboard events and terminal rendering |
| Metadata | nom-exif | 3.8.0 candidate | Validate capture timestamp coverage before adoption |

## Architecture style

A layered modular monolith ships as one executable from one Cargo package. The CLI drives application use cases. The application orchestrates analysis, planning, confirmation and execution. The domain contains pure rules for chronology, duplicate grouping from content evidence and sequence naming. Infrastructure reads metadata and operates on the filesystem. The entry point wires the modules together. When an application use case needs an external operation, define its contract in the application and implement it in infrastructure.

## Folder structure

- `src/main.rs` wires the executable.
- `src/cli/` owns terminal input and output.
- `src/application/` owns orchestration.
- `src/domain/` owns organization rules.
- `src/infrastructure/` owns filesystem and metadata adapters.
- `tests/` exercises the executable and filesystem integration.

## Conventions and security

Keep analysis read-only. Build the complete operation plan before applying changes and require interactive confirmation before execution. Test name collisions and partial failures before enabling file mutations. Keep media and metadata local. Preserve native path types at filesystem boundaries. The original README defines expected behavior.

## Testing strategy

Use Cargo unit tests for domain rules, temporary directories for filesystem behavior, and CLI integration tests. Check both metadata priority and creation-time fallback with representative media and filesystems before adopting the metadata parser. Run formatting, Clippy, tests and builds in CI across Linux, Windows and macOS.

## Distribution

Publish native executables through GitHub Releases. The container build is available for local verification and has no exposed ports.

## ADR-001 — Initial stack and architecture

Accepted. Use Rust and a layered modular monolith to keep distribution simple while validating filesystem behavior separately from rules and terminal interaction. Crossterm will support the interactive terminal. The metadata adapter lets the project verify format coverage before committing to a parser.
