# Media Organizer

Organize photos and videos by capture time from your terminal. Review the plan before applying it, with optional duplicate removal and subfolder consolidation.

[Downloads](https://github.com/crisbraatz/media-organizer/releases) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [MIT license](LICENSE)

## What it does

- Counts photos, videos, subfolders and duplicates in a folder tree.
- Proposes chronological filenames and flags occupied destinations before you apply changes.
- Compares duplicate files byte for byte. You can keep all copies or retain one from each group.
- Keeps files in their existing folders by default. You can bring the surviving media into a single chronological sequence in the selected folder.
- Processes media and metadata locally.

## Quick start

After installing, try the guided terminal flow with `media-organizer` or inspect a read-only preview:

```sh
media-organizer --preview "/path/to/photos"
```

Use a copy of a small folder for your first apply. Applying renames changes filenames. If you choose duplicate removal, the listed copies are deleted permanently.

## Install a release

Download your archive and the matching `SHA256SUMS` from the same [GitHub release](https://github.com/crisbraatz/media-organizer/releases). The executable runs without Rust. Replace `<version>` with the version shown on the release, such as `1.0.0`.

| Platform | Archive |
| --- | --- |
| Linux x64 | `media-organizer-<version>-linux-x64.tar.gz` |
| Windows x64 | `media-organizer-<version>-windows-x64.zip` |
| macOS Intel | `media-organizer-<version>-macos-x64.tar.gz` |
| macOS Apple Silicon | `media-organizer-<version>-macos-arm64.tar.gz` |

The Linux executable is statically linked with musl. Each archive includes the project license and third-party notices.

### Linux and macOS

In the download directory, set `archive` to the filename for your system and verify its checksum. Proceed only if the check reports `OK`.

```sh
archive=media-organizer-1.0.0-linux-x64.tar.gz
awk -v file="$archive" '$2 == file' SHA256SUMS | shasum -a 256 -c -
```

Use `sha256sum -c -` in place of `shasum -a 256 -c -` if your Linux system has `sha256sum`.

```sh
package_dir="$(mktemp -d)"
tar -xzf "$archive" -C "$package_dir"
"$package_dir/media-organizer" --version
mkdir -p "$HOME/.local/bin"
install -m 755 "$package_dir/media-organizer" "$HOME/.local/bin/media-organizer"
```

Add `$HOME/.local/bin` to your shell's `PATH` if needed.

### Windows

In PowerShell, set the zip filename and verify its SHA-256 hash before extracting it:

```powershell
$archive = 'media-organizer-1.0.0-windows-x64.zip'
$matching = @(Get-Content SHA256SUMS | Where-Object { $_.EndsWith("  $archive") })
if ($matching.Count -ne 1) { throw 'Expected one checksum for this archive.' }
$expected = ($matching[0] -split '\s+', 2)[0]
if ((Get-FileHash -Algorithm SHA256 $archive).Hash -ne $expected) {
    throw 'Checksum mismatch.'
}
Expand-Archive $archive -DestinationPath .\media-organizer
.\media-organizer\media-organizer.exe --version
```

Keep the executable in a folder on your user `Path` to run it by name from other directories.

## Commands

| Command | What it does |
| --- | --- |
| `media-organizer` | Opens the guided flow in an interactive terminal. |
| `media-organizer "<folder>"` | Analyzes the folder tree without changing it. |
| `media-organizer --preview "<folder>"` | Displays the proposed names and direct conflicts. |
| `media-organizer --apply "<folder>"` | Displays the plan and applies it after you type `yes` and press Enter. |
| `--remove-duplicates` | Adds duplicate removal to preview or apply. |
| `--flatten` | Adds a root-level sequence to preview or apply. |
| `media-organizer --help` / `--version` | Prints usage or the installed version. |

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

## Build from source

Install Rust with [rustup](https://rustup.rs/). The repository pins the toolchain and formatting and linting components.

```sh
cargo build --locked --release
cargo run --locked -- --help
```

The executable is under `target/release/`. See the [contribution guide](CONTRIBUTING.md) for the full checks and the [architecture](docs/architecture.md) for the release workflow. For a container smoke test, use `docker compose run --rm --build media-organizer` or build and run the image with Podman.

## Contributing and security

Read the [contribution guide](CONTRIBUTING.md) and [code of conduct](CODE_OF_CONDUCT.md) before opening a contribution. Follow the [security policy](SECURITY.md) to report vulnerabilities privately. The [product overview](docs/business/overview.md) describes the project goals.

## License

[MIT](LICENSE). Release archives include dependency and Rust toolchain notices in `third-party-notices/`.
