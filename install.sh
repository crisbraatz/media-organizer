#!/bin/sh
set -eu
umask 077

fail() {
    printf 'media-organizer installer: %s\n' "$*" >&2
    exit 1
}

case $# in
    0) install_dir=${HOME:?HOME is required}/.local/bin ;;
    2) [ "$1" = --install-dir ] && [ -n "$2" ] || fail 'Usage: install.sh [--install-dir DIRECTORY]'
       install_dir=$2 ;;
    *) fail 'Usage: install.sh [--install-dir DIRECTORY]' ;;
esac
case $install_dir in /*) ;; *) fail 'The install directory must be an absolute path.' ;; esac

case "$(uname -s):$(uname -m)" in
    Linux:x86_64|Linux:amd64) platform=linux-x64 ;;
    Darwin:arm64) platform=macos-arm64 ;;
    Darwin:x86_64)
        if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
            platform=macos-arm64
        else
            platform=macos-x64
        fi ;;
    *) fail 'Supported systems are Linux x64 and macOS Intel or Apple Silicon.' ;;
esac

for tool in curl tar awk grep mktemp; do
    command -v "$tool" >/dev/null 2>&1 || fail "Required command not found: $tool"
done
if command -v sha256sum >/dev/null 2>&1; then
    sha256() { sha256sum < "$1"; }
elif command -v shasum >/dev/null 2>&1; then
    sha256() { shasum -a 256 < "$1"; }
else
    fail 'A SHA-256 utility (sha256sum or shasum) is required.'
fi

fetch() {
    curl -fsSL --proto '=https' --proto-redir '=https' "$@"
}

repository=https://github.com/crisbraatz/media-organizer
latest=$(fetch -o /dev/null --write-out '%{url_effective}' "$repository/releases/latest") ||
    fail 'Could not locate the latest release.'
tag=${latest#"$repository/releases/tag/"}
case $tag in *[!v0-9.]*) fail 'GitHub did not return a supported stable release tag.' ;; esac
if [ "$tag" = "$latest" ] || ! printf '%s\n' "$tag" |
    LC_ALL=C grep -Eq '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'; then
    fail 'GitHub did not return a supported stable release tag.'
fi
asset=media-organizer-${tag#v}-$platform.tar.gz
base=$repository/releases/download/$tag

scratch=$(mktemp -d) || fail 'Could not create a temporary directory.'
stage=
cleanup() {
    if [ -n "$stage" ]; then rm -f -- "$stage" || true; fi
    rm -f -- "$scratch/$asset" "$scratch/SHA256SUMS" "$scratch/members" || true
    rmdir -- "$scratch" || true
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

fetch -o "$scratch/$asset" "$base/$asset" || fail "Could not download $asset."
fetch -o "$scratch/SHA256SUMS" "$base/SHA256SUMS" || fail 'Could not download SHA256SUMS.'
expected=$(LC_ALL=C awk -v name="$asset" '
    $2 == name {
        matches++
        if (NF == 2 && length($1) == 64 && $1 ~ /^[[:xdigit:]]+$/) hash = tolower($1)
    }
    END { if (matches != 1 || hash == "") exit 1; print hash }
' "$scratch/SHA256SUMS") || fail "Expected exactly one SHA-256 checksum for $asset."
actual=$(sha256 "$scratch/$asset") || fail 'Could not calculate SHA-256.'
[ "${actual%% *}" = "$expected" ] || fail "SHA-256 mismatch for $asset."
tar -tzf "$scratch/$asset" > "$scratch/members" || fail 'Could not list the verified archive.'
LC_ALL=C awk '$0 == "media-organizer" { count++ } END { exit (count != 1) }' "$scratch/members" ||
    fail 'Expected exactly one media-organizer executable in the archive.'

mkdir -p -- "$install_dir" || fail 'Could not create the user installation directory.'
destination=$install_dir/media-organizer
[ ! -d "$destination" ] && [ ! -L "$destination" ] ||
    fail 'The executable destination is a directory or a symbolic link.'
stage=$(mktemp "$install_dir/.media-organizer.XXXXXXXX") || fail 'Could not stage the executable.'
tar -xOzf "$scratch/$asset" media-organizer > "$stage" || fail 'Could not extract the executable.'
chmod 755 "$stage" || fail 'Could not make the executable runnable.'
reported=$("$stage" --version) || fail 'The packaged executable did not run.'
[ "$reported" = "media-organizer ${tag#v}" ] || fail 'The executable version does not match the release.'
mv -f -- "$stage" "$destination" || fail 'Could not replace the installed executable.'
stage=
printf 'Installed %s at %s\n' "$reported" "$destination"
case :${PATH-}: in
    *":$install_dir:"*) ;;
    *) printf 'Add %s to your PATH to run media-organizer by name.\n' "$install_dir" ;;
esac
