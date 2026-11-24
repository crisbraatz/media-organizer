# media-organizer

A local command-line tool for organizing personal photo and video collections. The current version analyzes a selected folder and reports photos, videos, duplicates and subfolders. See the [product overview](docs/business/overview.md) and [architecture](docs/architecture.md) for the planned behavior.

## Analyze a folder

Run `media-organizer` and enter the folder path when prompted, or run `media-organizer "/path/to/folder"`. From the source tree, use `cargo run --locked -- "/path/to/folder"`.

The report covers all levels of subfolders and includes file and duplicate totals by type and extension. Files with identical bytes count as extra copies after the first file in path order. The analysis leaves files and folders in place. It skips links found inside the selected folder and Windows reparse points, including junctions and some cloud placeholders.

## Preview chronological names

Run `media-organizer --preview "/path/to/folder"`, or omit the path to enter it when prompted. The preview follows the analysis report and shows the proposed names relative to the selected folder. Each folder is numbered independently from 1 with enough digits for its full media count. File extensions keep their original spelling.

The preview uses embedded capture timestamps when available, then the filesystem creation timestamp. It compares timestamps with explicit offsets as UTC and interprets offset-free timestamps as UTC. Files without either timestamp appear last. Native filenames break ties. JPEG, DNG and MP4 capture metadata are covered by tests. Parser coverage varies across media formats, and malformed, absent or unsupported metadata can require the filesystem fallback.

A `[blocked]` row marks an observed destination occupied by an entry that the plan does not move. A different row scheduled to move may free its current name. Direct conflict indicators do not guarantee that a chain of renames can be applied. The preview reads the live filesystem and leaves the collection in place.

## Develop locally

Install Rust with [rustup](https://rustup.rs/). The repository pins the toolchain and formatting and linting components.

```sh
cargo fetch --locked
cargo run --locked -- --help
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

## Distribution

Build with `cargo build --locked --release` on the target platform. The executable is under `target/release/` and can be placed on the user's path. Native releases will be distributed through GitHub Releases.

For a local container smoke test, run `docker compose run --rm --build media-organizer` or `podman build -t media-organizer .` followed by `podman run --rm media-organizer`.
