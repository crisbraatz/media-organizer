"""Build release archives and verify the complete set before publishing."""

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "x86_64-unknown-linux-musl": ("linux-x64", "tar.gz"),
    "x86_64-pc-windows-msvc": ("windows-x64", "zip"),
    "x86_64-apple-darwin": ("macos-x64", "tar.gz"),
    "aarch64-apple-darwin": ("macos-arm64", "tar.gz"),
}
CHECKSUMS = "SHA256SUMS"


def cargo_metadata(*args):
    output = subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1", *args],
        cwd=ROOT,
    )
    return json.loads(output)


def version():
    packages = cargo_metadata("--no-deps")["packages"]
    package = next(package for package in packages if package["name"] == "media-organizer")
    value = package["version"]
    if not re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", value):
        raise ValueError(f"Expected a stable release version, got {value!r}")
    return value


def archive_name(release_version, target):
    platform, suffix = TARGETS[target]
    return f"media-organizer-{release_version}-{platform}.{suffix}"


def stage_notices(target, directory):
    metadata = cargo_metadata("--filter-platform", target)
    dependencies = {node["id"] for node in metadata["resolve"]["nodes"]}
    for package in metadata["packages"]:
        if package["id"] not in dependencies or package["source"] is None:
            continue
        crate = Path(package["manifest_path"]).parent
        files = {
            path
            for path in crate.iterdir()
            if path.is_file()
            and path.name.upper().startswith(("LICENSE", "COPYING", "COPYRIGHT", "NOTICE"))
        }
        if package["license_file"]:
            files.add(crate / package["license_file"])
        if not files:
            raise ValueError(f"No license notice in {package['name']} {package['version']}")
        crate_notices = directory / f"{package['name']}-{package['version']}"
        for path in sorted(files):
            packaged = crate_notices / path.relative_to(crate)
            packaged.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, packaged)

    sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], cwd=ROOT, text=True)
    rust_docs = Path(sysroot.strip()) / "share" / "doc" / "rust"
    toolchain = directory / "rust"
    shutil.copytree(rust_docs / "licenses", toolchain / "licenses")
    shutil.copyfile(rust_docs / "COPYRIGHT-library.html", toolchain / "COPYRIGHT-library.html")


def package(target, binary, directory):
    release_version = version()
    name = ".exe" if TARGETS[target][1] == "zip" else ""
    executable = f"media-organizer{name}"
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / archive_name(release_version, target)

    with tempfile.TemporaryDirectory(prefix="media-organizer-release-") as scratch:
        stage = Path(scratch)
        shutil.copy2(binary, stage / executable)
        shutil.copyfile(ROOT / "LICENSE", stage / "LICENSE")
        stage_notices(target, stage / "third-party-notices")
        files = sorted(path for path in stage.rglob("*") if path.is_file())
        if name:
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
                for path in files:
                    output.write(path, path.relative_to(stage).as_posix())
            with zipfile.ZipFile(archive) as output:
                archived_binary = output.read(executable)
        else:
            with tarfile.open(archive, "w:gz") as output:
                for path in files:
                    output.add(path, arcname=path.relative_to(stage).as_posix())
            with tarfile.open(archive) as output:
                with output.extractfile(executable) as archived:
                    archived_binary = archived.read()

        extracted = stage / "smoke" / executable
        extracted.parent.mkdir()
        extracted.write_bytes(archived_binary)
        if not name:
            extracted.chmod(0o755)
        reported = subprocess.check_output(
            [str(extracted), "--version"], cwd=extracted.parent, text=True
        ).strip()
        if reported != f"media-organizer {release_version}":
            raise ValueError(f"Packaged executable reported unexpected version {reported!r}")


def digest(path):
    sha256 = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            sha256.update(chunk)
    return sha256.hexdigest()


def check_set(directory, verify):
    release_version = version()
    archives = sorted(archive_name(release_version, target) for target in TARGETS)
    expected = set(archives)
    if verify:
        expected.add(CHECKSUMS)
    entries = {path.name for path in directory.iterdir() if path.is_file()}
    if entries != expected or len(list(directory.iterdir())) != len(expected):
        raise ValueError(f"Expected exactly {sorted(expected)}, found {sorted(entries)}")
    checksums = "".join(f"{digest(directory / name)}  {name}\n" for name in archives).encode()
    manifest = directory / CHECKSUMS
    if verify:
        if manifest.read_bytes() != checksums:
            raise ValueError("Release checksums do not match the archives")
    else:
        manifest.write_bytes(checksums)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("version")
    pack = commands.add_parser("package")
    pack.add_argument("target", choices=TARGETS)
    pack.add_argument("binary", type=Path)
    pack.add_argument("directory", type=Path)
    for name in ("assemble", "verify"):
        commands.add_parser(name).add_argument("directory", type=Path)

    args = parser.parse_args()
    if args.command == "version":
        print(version())
    elif args.command == "package":
        package(args.target, args.binary, args.directory)
    else:
        check_set(args.directory, verify=args.command == "verify")


if __name__ == "__main__":
    main()
