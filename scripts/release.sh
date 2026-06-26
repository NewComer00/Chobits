#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# release.sh — bump version, generate CHANGELOG, tag, and push
#
# Usage:
#   ./release.sh                     # auto-detect bump type from commits
#   ./release.sh patch               # force patch bump
#   ./release.sh minor               # force minor bump
#   ./release.sh major               # force major bump
#   ./release.sh --dry-run           # preview only, no changes made
#   ./release.sh minor --dry-run     # force bump + preview
# ---------------------------------------------------------------------------

TOMLS=(
    Cargo.toml
    crates/chobits/Cargo.toml
    crates/chobits-bar/Cargo.toml
    crates/chobits-start/Cargo.toml
    crates/chobits-zellij/Cargo.toml
)

CHANGELOG="CHANGELOG.md"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
DRY_RUN=false
FORCE_BUMP=""

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        major|minor|patch) FORCE_BUMP="$arg" ;;
        *)
            echo "ERROR: unknown argument '$arg'" >&2
            echo "Usage: $0 [major|minor|patch] [--dry-run]" >&2
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# 1. Determine current version from workspace Cargo.toml
# ---------------------------------------------------------------------------
CURRENT=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
if [[ -z "$CURRENT" ]]; then
    echo "ERROR: could not detect current version in Cargo.toml" >&2
    exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

# ---------------------------------------------------------------------------
# 2. Get commits since last tag
# ---------------------------------------------------------------------------
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
if [[ -n "$LAST_TAG" ]]; then
    COMMITS=$(git log "${LAST_TAG}..HEAD" --pretty=format:"%s")
else
    COMMITS=$(git log --pretty=format:"%s")
fi

if [[ -z "$COMMITS" ]]; then
    if [[ "$DRY_RUN" == true ]]; then
        echo "WARNING: no commits since last tag ${LAST_TAG:-<none>} — preview only."
        echo ""
    else
        echo "No commits since last tag ${LAST_TAG:-<none>} — nothing to release."
        exit 0
    fi
fi

# ---------------------------------------------------------------------------
# 3. Determine bump type from conventional commits
#    v0.x (pre-1.0):
#      breaking change / feat! / BREAKING CHANGE → minor  (major stays 0)
#      feat:                                      → minor
#      fix: / perf: / revert:                    → patch
#      chore: / docs: / style: / ci: / test:      → no bump
#    v1.x+:
#      breaking change / feat! / BREAKING CHANGE → major
#      feat:                                      → minor
#      fix: / perf: / revert:                    → patch
#      chore: / docs: / style: / ci: / test:      → no bump
# ---------------------------------------------------------------------------
detect_bump() {
    local bump="none"
    local pre_release=false
    [[ "$MAJOR" -eq 0 ]] && pre_release=true

    while IFS= read -r msg; do
        if echo "$msg" | grep -qiE '(BREAKING CHANGE|^feat!|^fix!|^refactor!)'; then
            if [[ "$pre_release" == true ]]; then
                echo "minor"; return
            else
                echo "major"; return
            fi
        elif echo "$msg" | grep -qiE '^feat(\(.+\))?:'; then
            bump="minor"
        elif echo "$msg" | grep -qiE '^(fix|perf|revert)(\(.+\))?:'; then
            [[ "$bump" == "none" ]] && bump="patch"
        fi
        # chore/docs/style/ci/test/build → no bump
    done <<< "$COMMITS"
    echo "$bump"
}

if [[ -n "$FORCE_BUMP" ]]; then
    BUMP="$FORCE_BUMP"
else
    BUMP=$(detect_bump)
fi

if [[ "$BUMP" == "none" ]]; then
    echo "No releasable commits since ${LAST_TAG:-<none>} (only chore/docs/style/ci/test)."
    echo "Use './release.sh patch' to force a release anyway."
    exit 0
fi

# ---------------------------------------------------------------------------
# 4. Calculate new version
# ---------------------------------------------------------------------------
case "$BUMP" in
    major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
    patch) PATCH=$((PATCH + 1)) ;;
    *)
        echo "ERROR: unknown bump type '$BUMP' (use major|minor|patch)" >&2
        exit 1
        ;;
esac

NEW="${MAJOR}.${MINOR}.${PATCH}"
DATE=$(date +%Y-%m-%d)

echo "Current version : $CURRENT"
echo "Bump type       : $BUMP"
echo "New version     : $NEW"
echo "Last tag        : ${LAST_TAG:-<none>}"
echo "Dry run         : $DRY_RUN"
echo ""

# ---------------------------------------------------------------------------
# 5. Generate changelog entry
# ---------------------------------------------------------------------------
generate_changelog_entry() {
    local breaking="" feats="" fixes="" chores="" others=""

    while IFS= read -r msg; do
        if echo "$msg" | grep -qiE '(BREAKING CHANGE|^feat!|^fix!|^refactor!)'; then
            breaking+="- $msg"$'\n'
        elif echo "$msg" | grep -qiE '^feat(\(.+\))?:'; then
            feats+="- $msg"$'\n'
        elif echo "$msg" | grep -qiE '^(fix|perf|revert)(\(.+\))?:'; then
            fixes+="- $msg"$'\n'
        elif echo "$msg" | grep -qiE '^(chore|docs|style|refactor|perf|test|build|ci)(\(.+\))?:'; then
            chores+="- $msg"$'\n'
        else
            others+="- $msg"$'\n'
        fi
    done <<< "$COMMITS"

    echo "## [v${NEW}] - ${DATE}"
    echo ""
    if [[ -n "$breaking" ]]; then printf "### ⚠ Breaking Changes\n\n%s\n" "$breaking"; fi
    if [[ -n "$feats" ]]; then printf "### Features\n\n%s\n" "$feats"; fi
    if [[ -n "$fixes" ]]; then printf "### Bug Fixes\n\n%s\n" "$fixes"; fi
    if [[ -n "$chores" ]]; then printf "### Chores\n\n%s\n" "$chores"; fi
    if [[ -n "$others" ]]; then printf "### Other\n\n%s\n" "$others"; fi
    return 0
}

ENTRY=$(generate_changelog_entry)

echo "--- Changelog entry preview ---"
echo "$ENTRY"
echo "-------------------------------"
echo ""

if [[ "$DRY_RUN" == true ]]; then
    echo "Dry-run mode — the following steps would be executed:"
    echo ""
    for f in "${TOMLS[@]}"; do
        echo "  [dry-run] sed version $CURRENT → $NEW in $f"
    done
    echo "  [dry-run] cargo check  (update Cargo.lock)"
    echo "  [dry-run] prepend entry to $CHANGELOG"
    echo "  [dry-run] git add -A"
    echo "  [dry-run] git commit -m \"chore: release v${NEW}\""
    echo "  [dry-run] git tag v${NEW}"
    echo "  [dry-run] git push"
    echo "  [dry-run] git push origin v${NEW}"
    echo ""
    echo "Run without --dry-run to apply."
    exit 0
fi

read -rp "Proceed with release v${NEW}? [y/N] " CONFIRM
if [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]]; then
    echo "Aborted."
    exit 0
fi

# ---------------------------------------------------------------------------
# 6. Bump versions in Cargo.toml files
# ---------------------------------------------------------------------------
echo "Bumping versions $CURRENT → $NEW..."
for f in "${TOMLS[@]}"; do
    # Bump workspace version declaration
    sed -i "s/^version = \"${CURRENT}\"/version = \"${NEW}\"/" "$f"
    # Bump intra-workspace path dependency versions only (not external deps)
    sed -i "s/path = \"\([^\"]*\)\", version = \"${CURRENT}\"/path = \"\1\", version = \"${NEW}\"/g" "$f"
    echo "  updated $f"
done

# ---------------------------------------------------------------------------
# 7. Update Cargo.lock
# ---------------------------------------------------------------------------
echo "Updating Cargo.lock..."
cargo check

# ---------------------------------------------------------------------------
# 8. Update CHANGELOG.md
# ---------------------------------------------------------------------------
echo "Updating ${CHANGELOG}..."
if [[ -f "$CHANGELOG" ]]; then
    TMPFILE=$(mktemp)
    # Find the last blank line before the first ## section — insert there.
    HEADER_END=0
    LINENO=0
    while IFS= read -r line; do
        LINENO=$((LINENO + 1))
        if [[ "$line" =~ ^## ]]; then
            break
        fi
        [[ -z "$line" ]] && HEADER_END=$LINENO
    done < "$CHANGELOG"
    # If no ## found yet (fresh changelog), insert after all current content
    [[ "$HEADER_END" -eq 0 ]] && HEADER_END=$LINENO
    head -n "$HEADER_END" "$CHANGELOG" > "$TMPFILE"
    echo "$ENTRY" >> "$TMPFILE"
    tail -n +"$((HEADER_END + 1))" "$CHANGELOG" >> "$TMPFILE"
    mv "$TMPFILE" "$CHANGELOG"
else
    {
        echo "# Changelog"
        echo ""
        echo "All notable changes to this project will be documented in this file."
        echo ""
        echo "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),"
        echo "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)."
        echo ""
        echo "$ENTRY"
    } > "$CHANGELOG"
fi

# ---------------------------------------------------------------------------
# 9. Commit, tag, push
# ---------------------------------------------------------------------------
echo "Committing..."
git add -A
git commit -m "chore: release v${NEW}"
git tag "v${NEW}"
git push
git push origin "v${NEW}"

echo ""
echo "✓ Released v${NEW}"
