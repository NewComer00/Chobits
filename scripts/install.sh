#!/bin/sh
# shellcheck shell=sh
set -eu

# Detect sourcing: `. <(curl ... | sh)` updates PATH in the current shell.
_sourced=0
(return 0 2>/dev/null) && _sourced=1

GITHUB_REPO="${CHOBITS_GITHUB_REPO:-NewComer00/Chobits}"
XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
INSTALL_DIR="${CHOBITS_INSTALL_DIR:-$XDG_DATA_HOME/Chobits}"
VERSION="${CHOBITS_VERSION:-latest}"
BIN_DIR="${CHOBITS_BIN_DIR:-$INSTALL_DIR/bin}"

# ANSI styling (disabled when not a TTY or NO_COLOR is set)
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    _R='\033[0m'
    _B='\033[1m'
    _D='\033[2m'
    _C='\033[36m'
    _BC='\033[1;36m'
    _BY='\033[1;33m'
    _BG='\033[1;32m'
else
    _R=
    _B=
    _D=
    _C=
    _BC=
    _BY=
    _BG=
fi

err() {
    printf 'chobits: %s\n' "$1" >&2
    if [ "$_sourced" -eq 1 ]; then
        return 1
    fi
    exit 1
}

info() {
    printf 'chobits: %s\n' "$1"
}

success() {
    printf 'chobits: %b%s%b\n' "$_BG" "$1" "$_R"
}

download() {
    url=$1
    dest=$2
    show_progress=0
    if [ -t 2 ]; then
        show_progress=1
    fi

    if command -v curl >/dev/null 2>&1; then
        if [ "$show_progress" -eq 1 ]; then
            curl -fL --progress-bar "$url" -o "$dest"
            printf '\n'
        else
            curl -fsSL "$url" -o "$dest"
        fi
    elif command -v wget >/dev/null 2>&1; then
        if [ "$show_progress" -eq 1 ]; then
            wget "$url" -O "$dest"
        else
            wget -q "$url" -O "$dest"
        fi
    else
        err "curl or wget is required"
    fi
}

add_to_path() {
    if [ "${CHOBITS_NO_MODIFY_PATH:-}" = "1" ]; then
        info "skipped PATH update (CHOBITS_NO_MODIFY_PATH=1)"
        return 0
    fi

    env_file="${INSTALL_DIR}/chobits.env"
    {
        printf '# chobits\n'
        printf 'export PATH="%s:$PATH"\n' "$BIN_DIR"
    } > "$env_file"

    export PATH="${BIN_DIR}:$PATH"

    case "${SHELL:-}" in
        */zsh) rc="$HOME/.zshrc" ;;
        */bash) rc="$HOME/.bashrc" ;;
        *) rc="$HOME/.profile" ;;
    esac

    touch "$rc"
    path_line="export PATH=\"${BIN_DIR}:\$PATH\"  # chobits"

    # sed -i behaves differently on GNU (Linux) vs BSD (macOS):
    # GNU requires:  sed -i 'expr'
    # BSD requires:  sed -i '' 'expr'
    if sed --version 2>/dev/null | grep -q GNU; then
        _sed_i() { sed -i "$@"; }
    else
        _sed_i() { sed -i '' "$@"; }
    fi

    if grep -Fq '# chobits' "$rc"; then
        _sed_i "s|^export PATH=.*# chobits|${path_line}|" "$rc"
        info "updated PATH in ${rc}"
    else
        {
            printf '\n# chobits\n'
            printf '%s\n' "$path_line"
        } >> "$rc"
        info "added ${BIN_DIR} to PATH in ${rc}"
    fi

    if [ "$_sourced" -eq 0 ]; then
        info "open a new terminal, or run: source \"${env_file}\""
    fi
}

hint_next_steps() {
    config="${INSTALL_DIR}/config.toml"
    printf '\n'
    printf '  %bNext steps%b\n' "$_BC" "$_R"
    printf '  %b----------%b\n' "$_D" "$_R"
    printf '\n'
    printf '  %b1.%b Edit %b[llm]%b in %b%s%b\n' "$_B" "$_R" "$_BY" "$_R" "$_C" "$config" "$_R"
    printf '     pick one example:\n'
    printf '\n'
    printf '     %bOllama:%b\n' "$_BY" "$_R"
    cat <<'EOF'
     [llm]
     backend    = "ollama"
     url        = "http://localhost:11434"
     model      = "qwen3:0.6b"
     max_tokens = 512

EOF
    printf '     %bOpenAI-compatible API:%b\n' "$_BY" "$_R"
    cat <<'EOF'
     [llm]
     backend    = "deepseek"
     url        = "https://api.deepseek.com"
     model      = "deepseek-v4-flash"
     max_tokens = 512
     api_key    = "sk-..."

EOF
    printf '  %b2.%b %b(optional, Ollama only)%b install https://ollama.com, then run\n' "$_B" "$_R" "$_D" "$_R"
    printf '     %bollama pull qwen3:0.6b%b\n' "$_BG" "$_R"
    printf '\n'
    printf '  %b3.%b Launch:\n' "$_B" "$_R"
    printf '     %bchobits-start%b\n' "$_BG" "$_R"
    printf '\n'
}

# --------------- Platform detection ---------------
os=$(uname -s)
arch=$(uname -m)

case "$os" in
    Linux)
        case "$arch" in
            x86_64|amd64) ;;
            *)
                err "unsupported architecture on Linux: $arch (x86_64 only)"
                ;;
        esac

        libc="${CHOBITS_LIBC:-musl}"
        case "$libc" in
            musl) target="x86_64-unknown-linux-musl" ;;
            gnu)  target="x86_64-unknown-linux-gnu" ;;
            *)    err "CHOBITS_LIBC must be 'musl' or 'gnu'" ;;
        esac
        ;;

    Darwin)
        case "$arch" in
            arm64)    target="aarch64-apple-darwin" ;;
            x86_64)   target="x86_64-apple-darwin" ;;
            *)
                err "unsupported architecture on macOS: $arch (arm64 or x86_64 only)"
                ;;
        esac
        ;;

    *)
        err "unsupported OS: $os (Linux or macOS only)"
        ;;
esac

if [ "$VERSION" = "latest" ]; then
    base="https://github.com/${GITHUB_REPO}/releases/latest/download"
else
    base="https://github.com/${GITHUB_REPO}/releases/download/${VERSION}"
fi

archive="Chobits-${target}.tar.gz"
url="${base}/${archive}"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

info "downloading ${archive}"
download "$url" "${tmp}/${archive}"

info "installing to ${INSTALL_DIR}"
install_parent=$(dirname "$INSTALL_DIR")
mkdir -p "$install_parent"
rm -rf "$INSTALL_DIR"
extract_dir="${tmp}/extract"
mkdir -p "$extract_dir"
tar -xzf "${tmp}/${archive}" -C "$extract_dir"
root="${extract_dir}/Chobits"
if [ ! -d "$root" ]; then
    err "archive did not contain a Chobits/ folder"
fi
mv "$root" "$INSTALL_DIR"

add_to_path

success "installed to ${INSTALL_DIR}"
hint_next_steps
