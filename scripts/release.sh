#!/usr/bin/env bash
# =============================================================
#  release.sh — Create a new versioned release
#
#  Usage : ./scripts/release.sh <X.Y.Z>
#  Example: ./scripts/release.sh 0.2.0
#
#  What this script does:
#    1. Validates the version argument (X.Y.Z format)
#    2. Verifies the working tree is clean and the branch is main
#    3. Checks the tag does not already exist
#    4. Updates the version in monitoring-alert/Cargo.toml
#    5. Promotes [Unreleased] → [X.Y.Z] in CHANGELOG.md
#    6. Runs cargo test to confirm everything still passes
#    7. Commits the version bump
#    8. Creates an annotated git tag vX.Y.Z
#    9. Pushes the commit and the tag to origin/main
# =============================================================

set -euo pipefail

VERSION="${1:?Usage: $0 <version>   e.g. $0 0.2.0}"
TAG="v${VERSION}"
GITHUB_REPO="jordanatdown/monitoring-alert"

REPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
CARGO_TOML="${REPO_ROOT}/monitoring-alert/Cargo.toml"
CHANGELOG="${REPO_ROOT}/CHANGELOG.md"

# ── 1. Validate semver format ─────────────────────────────────
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: version must be in X.Y.Z format (got: ${VERSION})" >&2
    exit 1
fi

# ── 2. Check branch and working tree ─────────────────────────
BRANCH=$(git -C "$REPO_ROOT" symbolic-ref --short HEAD 2>/dev/null || echo "DETACHED")
if [[ "$BRANCH" != "main" ]]; then
    echo "Error: must be on branch 'main' (currently on: ${BRANCH})" >&2
    exit 1
fi

if ! git -C "$REPO_ROOT" diff --quiet || ! git -C "$REPO_ROOT" diff --staged --quiet; then
    echo "Error: working tree has uncommitted changes — commit or stash them first." >&2
    exit 1
fi

# ── 3. Check the tag does not already exist ───────────────────
if git -C "$REPO_ROOT" tag --list "$TAG" | grep -q .; then
    echo "Error: tag ${TAG} already exists." >&2
    exit 1
fi

echo "==> Preparing release ${TAG}"

# ── 4. Update Cargo.toml version ─────────────────────────────
CURRENT_VERSION=$(grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')
if [[ "$CURRENT_VERSION" == "$VERSION" ]]; then
    echo "    Cargo.toml already at ${VERSION} — no change needed."
else
    sed -i "s/^version = \"${CURRENT_VERSION}\"/version = \"${VERSION}\"/" "$CARGO_TOML"
    echo "    Cargo.toml: ${CURRENT_VERSION} → ${VERSION}"
fi

# ── 5. Promote [Unreleased] → [X.Y.Z] in CHANGELOG.md ───────
DATE=$(date +%Y-%m-%d)
echo "    CHANGELOG.md: [Unreleased] → [${VERSION}] — ${DATE}"

# Insert the new version header immediately after ## [Unreleased]
awk -v ver="$VERSION" -v date="$DATE" '
/^## \[Unreleased\]/ {
    print
    print ""
    print "## [" ver "] \342\200\224 " date
    next
}
{ print }
' "$CHANGELOG" > "${CHANGELOG}.tmp" && mv "${CHANGELOG}.tmp" "$CHANGELOG"

# Update the comparison links at the bottom of the file
awk -v ver="$VERSION" -v tag="$TAG" -v repo="$GITHUB_REPO" '
/^\[Unreleased\]:/ {
    print "[Unreleased]: https://github.com/" repo "/compare/" tag "...HEAD"
    print "[" ver "]: https://github.com/" repo "/releases/tag/" tag
    next
}
{ print }
' "$CHANGELOG" > "${CHANGELOG}.tmp" && mv "${CHANGELOG}.tmp" "$CHANGELOG"

# ── 6. Run tests ──────────────────────────────────────────────
echo "==> Running cargo test"
(cd "${REPO_ROOT}/monitoring-alert" && cargo test --quiet)
echo "    All tests passed."

# ── 7. Commit ─────────────────────────────────────────────────
echo "==> Committing version bump"
git -C "$REPO_ROOT" add \
    "$CARGO_TOML" \
    "$CHANGELOG" \
    "${REPO_ROOT}/monitoring-alert/Cargo.lock"
git -C "$REPO_ROOT" commit -m "chore(release): bump version to ${VERSION}"

# ── 8. Tag ────────────────────────────────────────────────────
echo "==> Creating annotated tag ${TAG}"
git -C "$REPO_ROOT" tag -a "$TAG" -m "Release ${TAG}"

# ── 9. Push ───────────────────────────────────────────────────
echo "==> Pushing to origin"
git -C "$REPO_ROOT" push -u origin main
git -C "$REPO_ROOT" push origin "$TAG"

echo ""
echo "Release ${TAG} created and pushed successfully."
echo "GitHub Actions will build the Windows binary and publish the release."
echo "https://github.com/${GITHUB_REPO}/actions"
