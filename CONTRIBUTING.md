# Contributing to Media Organizer

Use [GitHub Issues](https://github.com/crisbraatz/media-organizer/issues) for ordinary bugs and feature requests. Check existing issues before opening a new one. Follow the [code of conduct](CODE_OF_CONDUCT.md) and use the [security policy](SECURITY.md) for vulnerabilities.

For a bug, include the output of `media-organizer --version`, your operating system, the command, and steps to reproduce the problem. Use a synthetic folder and files in examples. Keep private media, filenames and credentials out of issues and pull requests.

## Develop and test

1. Fork and clone the repository. Create a branch for a focused change.
2. Install [rustup](https://rustup.rs/) and run the checks below from the repository root.
3. Add or update tests for behavior changes and update the README when command usage changes.
4. Open a pull request that explains the change, links the issue when applicable, and lists the checks you ran.

```sh
cargo fetch --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

Use temporary directories for tests that rename, move or remove files. See the [architecture guide](docs/architecture.md) for code boundaries and the maintainer release procedure.
