# Architecture

## Stack

| Layer | Technology | Version | Purpose |
| --- | --- | --- | --- |
| Language and build | Rust and Cargo | 1.100.0 toolchain, Edition 2024 | Portable native CLI |
| Terminal | Crossterm | 0.29.0 | Keyboard events and terminal rendering |
| Metadata | nom-exif | 3.8.0 | Read embedded capture times for the optional preview |
| Filesystem | renamest | 0.1.3 | Native no-replace renames |

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


## ADR-002 — Preview timestamp source

Accepted for the read-only preview. Use nom-exif 3.8.0 in infrastructure to read image capture and video creation tags. Generated JPEG, DNG and MP4 fixtures exercise the parser. When metadata is absent, malformed or unsupported, use file creation time when available. Interpret offset-free metadata as UTC. The OSV audit identified an informational maintenance notice for the indirect chrono dependency. [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2026-0543.html)


## ADR-003 — Exclusive two-phase renames

Accepted. Apply only the approved plan after validating it again. Stage changed names in the same directory before installing final names so cycles can proceed. Use native exclusive renames for every stage and restoration. On a reported failure, reverse final moves before restoring original names. List the last known path for each incomplete recovery. Process interruption can leave temporary names because this feature uses in-memory recovery state.

Preflight compares full source observations. On Windows, the per-move check omits creation time because the filesystem can change it when a name is reused. The checks are best-effort when the filesystem changes concurrently. [Renamest](https://docs.rs/crate/renamest/0.1.3) · [Microsoft file rename specification](https://learn.microsoft.com/openspecs/windows_protocols/ms-fsa/87f86c9b-6c2a-4803-84b7-131a74a434fa)

## ADR-004 — Duplicate cleanup after survivor renames

Accepted. Reuse the existing exact-content groups, with the earliest native path as keeper. The preview and the reviewed plan include each copy and its keeper. Compute chronological names from the survivors and stage copies alongside changed survivors. Restore stages on rename failure. After all survivor renames succeed, check file observations and exact content against each keeper's final path before unlinking its staged copy. Stop on the first cleanup error and report removed originals and last known paths for pending copies. Keep survivor names in place once cleanup begins.

## ADR-005 — Confined cross-directory moves and empty-folder cleanup

Accepted. Flattening plans one root sequence after optional duplicate filtering. Keep exclusive staging beside each original and validate both parents for cross-directory finalization and recovery. An unsupported cross-filesystem move fails through the existing rollback path. After successful file operations, check the captured identity of affected ancestor subfolders where available and attempt nonrecursive removal, deepest first. Preserve occupied and unrelated directories. Directory cleanup errors are separate from file execution errors.
