#!/bin/sh
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
mkdir "$scratch/tools" "$scratch/package" "$scratch/install"
export INSTALL_FIXTURE="$scratch"
cat > "$scratch/tools/uname" <<'MOCK'
#!/bin/sh
case "$1" in -s) echo Linux;; -m) echo x86_64;; esac
MOCK
cat > "$scratch/tools/curl" <<'MOCK'
#!/bin/sh
set -eu
while [ "$#" -gt 1 ]; do
    case "$1" in
        -o) destination=$2; shift ;;
        --proto|--proto-redir|--write-out) shift ;;
    esac
    shift
done
case "$1" in
    */releases/latest) printf '%s' "${FIXTURE_URL:-https://github.com/crisbraatz/media-organizer/releases/tag/v1.0.0}" ;;
    */releases/download/v1.0.0/*) cp "$INSTALL_FIXTURE/${1##*/}" "$destination" ;;
    *) exit 1 ;;
esac
MOCK
chmod +x "$scratch/tools/"*
export PATH="$scratch/tools:$PATH"
asset=media-organizer-1.0.0-linux-x64.tar.gz
make_asset() {
    printf '#!/bin/sh\nprintf "media-organizer %s\\n"\n' "$1" > "$scratch/package/media-organizer"
    tar -czf "$scratch/$asset" -C "$scratch/package" media-organizer
    if command -v sha256sum >/dev/null 2>&1; then
        hash=$(sha256sum < "$scratch/$asset")
    else
        hash=$(shasum -a 256 < "$scratch/$asset")
    fi
    printf '%s  %s\n' "${hash%% *}" "$asset" > "$scratch/SHA256SUMS"
}
install_test() {
    sh "$root/install.sh" --install-dir "$scratch/install" > "$scratch/output" 2>&1
}
make_asset 1.0.0
install_test
[ "$("$scratch/install/media-organizer" --version)" = 'media-organizer 1.0.0' ]
printf 'previous bytes\n' > "$scratch/install/media-organizer"
cp "$scratch/install/media-organizer" "$scratch/previous"
make_asset 0.9.0
if install_test; then echo 'Unexpected success with wrong version' >&2; exit 1; fi
cmp "$scratch/previous" "$scratch/install/media-organizer"
make_asset 1.0.0
cp "$scratch/SHA256SUMS" "$scratch/manifest"
cat "$scratch/manifest" >> "$scratch/SHA256SUMS"
if install_test; then echo 'Unexpected success with duplicate hash' >&2; exit 1; fi
cmp "$scratch/previous" "$scratch/install/media-organizer"
printf 'changed archive bytes' >> "$scratch/$asset"
cp "$scratch/manifest" "$scratch/SHA256SUMS"
if install_test; then echo 'Unexpected success with invalid hash' >&2; exit 1; fi
cmp "$scratch/previous" "$scratch/install/media-organizer"
make_asset 1.0.0
if (FIXTURE_URL=https://example.com/releases/tag/v1.0.0 install_test); then
    echo 'Unexpected success with foreign release URL' >&2; exit 1
fi
install_test
[ "$("$scratch/install/media-organizer" --version)" = 'media-organizer 1.0.0' ]
