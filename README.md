# media-organizer

A local command-line tool for organizing personal photo and video collections. The initial scaffold provides the command and its help and version flags. See the [product overview](docs/business/overview.md) and [architecture](docs/architecture.md) for the planned behavior.

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
