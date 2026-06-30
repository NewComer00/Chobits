#!/usr/bin/env bash
set -euo pipefail

# On Windows, only UCRT64 and MINGW64 are tested
if [[ -n "${MSYSTEM:-}" && ! "${MSYSTEM}" =~ ^(UCRT64|MINGW64)$ ]]; then
    echo "WARNING: \"MSYSTEM=${MSYSTEM}\" is not tested. Recommended: 'UCRT64' or 'MINGW64'." >&2
    echo "         Things may not work correctly." >&2
fi

FORCE_ALL=0
FORCE_CHOBITS=0
FORCE_LIVE_ASCII=0
FORCE_ZELLIJ=0
LOCKED=0
YES=0
DEST="${DEST:-install/Chobits}"

usage() {
    cat <<EOF
Usage: $0 [--dest DIR | -d DIR] [--force-rebuild-all] [--force-rebuild-chobits] [--force-rebuild-live-ascii] [--force-rebuild-zellij] [--locked] [-y]

  --dest, -d DIR                Install Chobits into DIR (default: ${DEST})
  --force-rebuild-all           Reinstall every component even if it already exists
  --force-rebuild-chobits       Reinstall chobits* crates even if they already exist
  --force-rebuild-live-ascii    Reinstall live-ascii even if it already exists
  --force-rebuild-zellij        Reinstall zellij even if it already exists
  --locked                      Pass --locked to all cargo install calls
  -y                            Skip the confirmation prompt
  -h, --help                    Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dest|-d)
            [[ $# -ge 2 ]] || { echo "error: $1 requires an argument" >&2; exit 1; }
            DEST="$2"
            shift 2
            ;;
        --dest=*)
            DEST="${1#--dest=}"
            shift
            ;;
        --force-rebuild-all)
            FORCE_ALL=1
            shift
            ;;
        --force-rebuild-chobits)
            FORCE_CHOBITS=1
            shift
            ;;
        --force-rebuild-live-ascii)
            FORCE_LIVE_ASCII=1
            shift
            ;;
        --force-rebuild-zellij)
            FORCE_ZELLIJ=1
            shift
            ;;
        --locked)
            LOCKED=1
            shift
            ;;
        -y)
            YES=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            usage
            exit 1
            ;;
    esac
done

need() {
    # need <install_dir> <bin_name> <description> <force> <install_cmd...>
    local install_dir="$1" bin_name="$2" desc="$3" force="$4"; shift 4
    if [[ $force -eq 1 || ! -f "$install_dir/$bin_name" ]]; then
        echo "Installing $desc..."
        "$@"
    else
        echo "Skipping $desc (already exists)"
    fi
}

echo "Prerequisites:"
echo "  cargo           - Rust toolchain with native and 'wasm32-wasip1' targets"
echo "  cargo-binstall  - For easier installation of Zellij. Install it with: \`cargo install cargo-binstall\`"
echo "  jq              - JSON processor"
echo "  wget            - HTTP downloader"
echo "  unzip           - ZIP extractor"
echo "  make, cc        - GNU Make and C toolchain (for live-ascii)"
echo ""
echo "Install destination: $DEST"
if [[ $YES -eq 0 ]]; then
    read -rsp "Continue? [y/N] " -n 1 confirm
    echo
    [[ "$confirm" =~ ^[Yy]$ ]] || exit 0
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$DEST"

CHOBITS_ROOT_FLAG="$DEST/.chobits-root"
BIN="$DEST/bin"
LOCAL_BIN="$DEST/local/bin"

LOCK_FLAG=""
[[ $LOCKED -eq 1 ]] && LOCK_FLAG="--locked"

# Force helpers: use component-specific flag, fall back to the all-inclusive FORCE_ALL.
force_chobits=$(( FORCE_ALL || FORCE_CHOBITS ))
force_live_ascii=$(( FORCE_ALL || FORCE_LIVE_ASCII ))
force_zellij=$(( FORCE_ALL || FORCE_ZELLIJ ))

# Mark dest as chobits-root dir
if [[ ! -f "$CHOBITS_ROOT_FLAG" ]]; then
    touch "$CHOBITS_ROOT_FLAG"
fi

# chobits starter → bin/
need "$BIN" "chobits-start" "chobits-start" "$force_chobits" \
    cargo install $LOCK_FLAG --force --path "crates/chobits-start" --root "$DEST"

# chobits sibling binaries → local/bin/
for c in "" "-bar"; do
    need "$LOCAL_BIN" "chobits${c}" "chobits${c}" "$force_chobits" \
        cargo install $LOCK_FLAG --force --path "crates/chobits${c}" --root "$DEST/local"
done

# chobits-zellij plugin → local/bin/
need "$LOCAL_BIN" "chobits-zellij.wasm" "chobits-zellij" "$force_chobits" \
    cargo install $LOCK_FLAG --force --path crates/chobits-zellij --root "$DEST/local" --target wasm32-wasip1

# live-ascii → local/bin/
need "$LOCAL_BIN" "live-ascii" "live-ascii" "$force_live_ascii" \
    cargo install $LOCK_FLAG --force --git https://github.com/NewComer00/live-ascii --root "$DEST/local"

# zellij (version pinned to match zellij-tile in Cargo.lock) → local/bin/
ZELLIJ_VER=$(cargo metadata --format-version 1 | jq -r '
    .packages[] | select(.name == "zellij-tile") | .version')
if [[ "${MSYSTEM:-}" =~ ^(UCRT64|MINGW64)$ ]]; then
    need "$LOCAL_BIN" "zellij" "zellij $ZELLIJ_VER" "$force_zellij" \
        cargo binstall --force zellij --version "$ZELLIJ_VER" --root "$DEST/local" -y \
            $LOCK_FLAG \
            --target x86_64-pc-windows-msvc \
            --pkg-url "https://github.com/zellij-org/zellij/releases/download/v{ version }/{ name }-x86_64-pc-windows-msvc.zip" \
            --pkg-fmt zip \
            --bin-dir "{ bin }{ binary-ext }" \
            --disable-strategies compile
else
    need "$LOCAL_BIN" "zellij" "zellij $ZELLIJ_VER" "$force_zellij" \
        cargo binstall --force zellij --version "$ZELLIJ_VER" --root "$DEST/local" -y $LOCK_FLAG
fi

# Live2D sample model (hiyori)
if [[ $FORCE_ALL -eq 1 || ! -d "$DEST/models/hiyori_free" ]]; then
    echo "Downloading hiyori Live2D model..."
    wget -P "$TMP" https://cubism.live2d.com/sample-data/bin/hiyori/hiyori_en.zip
    unzip -qo "$TMP/hiyori_en.zip" -d "$TMP"
    mkdir -p "$DEST/models"
    cp -r "$TMP/hiyori_free" "$DEST/models/"
else
    echo "Skipping hiyori model (already exists)"
fi

# Example config (only if not already present)
if [[ ! -f "$DEST/config.toml" ]]; then
    cp example_config.toml "$DEST/config.toml"
    echo "Wrote example config to $DEST/config.toml."
fi

echo -e "\nDone. Run \`$DEST/bin/chobits-start\` to launch Chobits."
