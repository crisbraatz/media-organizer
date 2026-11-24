# media-organizer

A local command-line tool for organizing personal photo and video collections. See the [product overview](docs/business/overview.md) and [architecture](docs/architecture.md) for details.

## Organize interactively

Run `media-organizer` with no arguments in a terminal. From the source tree, use `cargo run --locked`.

1. Type the folder path. Relative paths start from your current directory. Press Tab to complete a directory name or extend it to the shared prefix of matching directories. Press Enter to analyze the folder.
2. Review the report, then use the arrow keys and Enter to decide whether to remove duplicate copies and gather subfolder media into the selected folder. Both choices start at No.
3. Review the preview, then select Exit, Restart or Apply. Exit is selected by default. Apply is available when there are changes and the preview has no blocked destinations.
4. After selecting Apply, type `yes` and press Enter to confirm. The command warns before asking you to confirm permanent duplicate removal.

Escape and Ctrl+C leave a guided prompt without applying the plan. To use the explicit modes below, pass a path or mode flag. With no arguments and redirected input or output, the command reads the folder path at the prompt and prints the analysis report.

## Analyze a folder

Run `media-organizer "/path/to/folder"`. From the source tree, use `cargo run --locked -- "/path/to/folder"`.

The report covers all levels of subfolders and includes file and duplicate totals by type and extension. Files with identical bytes count as extra copies after the first file in path order. The analysis leaves files and folders in place. It skips links found inside the selected folder and Windows reparse points, including junctions and some cloud placeholders.

## Preview chronological names

Run `media-organizer --preview "/path/to/folder"`, or omit the path to enter it when prompted. The preview follows the analysis report and shows the proposed names relative to the selected folder. Each folder is numbered independently from 1 with enough digits for its full media count. File extensions keep their original spelling.

The preview uses embedded capture timestamps when available, then the filesystem creation timestamp. It compares timestamps with explicit offsets as UTC and interprets offset-free timestamps as UTC. Files without either timestamp appear last. Native filenames break ties. JPEG, DNG and MP4 capture metadata are covered by tests. Parser coverage varies across media formats, and malformed, absent or unsupported metadata can require the filesystem fallback.

A `[blocked]` row marks an observed destination occupied by an entry that the plan does not move. A different row scheduled to move may free its current name. Direct conflict indicators do not guarantee that a chain of renames can be applied. The preview reads the live filesystem and leaves the collection in place.

## Apply chronological names

Run `media-organizer --apply "/path/to/folder"` and review the displayed names. Type `yes` and press Enter to apply them. To choose the folder at the prompt, omit the path. Any other answer or an incomplete confirmation cancels the operation.

After confirmation, the terminal shows a progress bar with the current step and its count. If renaming fails, the bar shows attempts to restore the original names and the final output reports any unresolved files. Analysis, previews and redirected runs keep their plain output.

The command checks the reviewed plan again before moving files and uses native operations that leave existing destinations intact. Files remain in their original folders. A filesystem that cannot perform an exclusive rename causes the operation to stop. If a rename fails, the command attempts to restore the original names and reports any files that need manual recovery.

Avoid editing the folder during an apply operation. Validation checks the live filesystem on a best-effort basis. The command can attempt recovery after a reported move error. A process interruption can leave files at a final name or under temporary `.media-organizer-*.stage` names.

## Remove duplicate copies

Add `--remove-duplicates` to `--preview` to see which copies would be removed and which file would be kept. Add it to `--apply` to remove those copies permanently after you review the plan and type `yes` followed by Enter.

Duplicates are checked byte for byte across the selected folder and its subfolders. The first path in native sort order is kept, and each folder's numbering uses only its surviving files. The analysis totals describe the folder before any changes.

The command stages duplicate copies until every survivor rename succeeds. Before deleting each staged copy, it confirms that the keeper still matches. If cleanup fails, completed survivor renames stay in place. The command lists the copies already removed and the last known staged paths of the remaining copies.

## Organize media into the selected folder

Add `--flatten` to `--preview` to inspect one chronological sequence for the media in the
selected folder and its subfolders. Add it to `--apply` to move media to the selected folder
after reviewing and confirming the plan. Combine it with `--remove-duplicates` to number only
retained media.

After all file changes succeed, the command removes affected subfolders that became empty.
Unrelated empty folders and folders containing other entries stay in place. If a move crosses
a filesystem boundary that cannot support a native exclusive rename, the command stops and
attempts to restore the original names. If empty-subfolder cleanup fails after the file changes,
the command reports the cleanup errors and leaves the completed file changes in place.

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
