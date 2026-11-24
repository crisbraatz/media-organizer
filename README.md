# media-organizer

A local command-line tool for organizing personal photo and video collections. The current version analyzes a selected folder and reports photos, videos, duplicates and subfolders. See the [product overview](docs/business/overview.md) and [architecture](docs/architecture.md) for the planned behavior.

## Analyze a folder

Run `media-organizer` and enter the folder path when prompted, or run `media-organizer "/path/to/folder"`. From the source tree, use `cargo run --locked -- "/path/to/folder"`.

The report covers all levels of subfolders and includes file and duplicate totals by type and extension. Files with identical bytes count as extra copies after the first file in path order. The analysis leaves files and folders in place. It skips links found inside the selected folder and Windows reparse points, including junctions and some cloud placeholders.

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
