# media-organizer

A local command-line tool for organizing personal photo and video collections. The current version analyzes a selected folder and reports photos, videos, duplicates and subfolders. See the [product overview](docs/business/overview.md) and [architecture](docs/architecture.md) for the planned behavior.

## Analyze a folder

Run `media-organizer` and enter the folder path when prompted, or run `media-organizer "/path/to/folder"`. From the source tree, use `cargo run --locked -- "/path/to/folder"`.

The report covers all levels of subfolders and includes file and duplicate totals by type and extension. Files with identical bytes count as extra copies after the first file in path order. The analysis leaves files and folders in place. It skips links found inside the selected folder and Windows reparse points, including junctions and some cloud placeholders.

## Preview chronological names

Run `media-organizer --preview "/path/to/folder"`, or omit the path to enter it when prompted. The preview follows the analysis report and shows the proposed names relative to the selected folder. Each folder is numbered independently from 1 with enough digits for its full media count. File extensions keep their original spelling.

The preview uses embedded capture timestamps when available, then the filesystem creation timestamp. It compares timestamps with explicit offsets as UTC and interprets offset-free timestamps as UTC. Files without either timestamp appear last. Native filenames break ties. JPEG, DNG and MP4 capture metadata are covered by tests. Parser coverage varies across media formats, and malformed, absent or unsupported metadata can require the filesystem fallback.

A `[blocked]` row marks an observed destination occupied by an entry that the plan does not move. A different row scheduled to move may free its current name. Direct conflict indicators do not guarantee that a chain of renames can be applied. The preview reads the live filesystem and leaves the collection in place.

## Apply chronological names

Run `media-organizer --apply "/path/to/folder"` and review the displayed names. Type `yes` and press Enter to apply them. To choose the folder at the prompt, omit the path. Any other answer or an incomplete confirmation cancels the operation.

The command checks the reviewed plan again before moving files and uses native operations that leave existing destinations intact. Files remain in their original folders. A filesystem that cannot perform an exclusive rename causes the operation to stop. If a rename fails, the command attempts to restore the original names and reports any files that need manual recovery.

Avoid editing the folder during an apply operation. Validation checks the live filesystem on a best-effort basis. The command can attempt recovery after a reported move error. A process interruption can leave files at a final name or under temporary `.media-organizer-*.stage` names.

## Remove duplicate copies

Add `--remove-duplicates` to `--preview` to see which copies would be removed and which file would be kept. Add it to `--apply` to remove those copies permanently after you review the plan and type `yes` followed by Enter.

Duplicates are checked byte for byte across the selected folder and its subfolders. The first path in native sort order is kept, and each folder's numbering uses only its surviving files. The analysis totals describe the folder before any changes.

The command stages duplicate copies until every survivor rename succeeds. Before deleting each staged copy, it confirms that the keeper still matches. If cleanup fails, completed survivor renames stay in place. The command lists the copies already removed and the last known staged paths of the remaining copies.

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
