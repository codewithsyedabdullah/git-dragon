#!/bin/sh
# git-dragon action entrypoint.
#
# Reads the action inputs (exposed as INPUT_* environment variables),
# collects the account activity snapshot, renders the dragon assets, and
# publishes them to the configured output branch.

set -eu

input() {
    printenv | sed -n "s/^INPUT_$1=//p" | head -n 1
}

LOGIN="$(input LOGIN)"
WINDOW="$(input WINDOW)"
CANON="$(input CANON)"
OUTPUT_DIR="$(input OUTPUT-DIR)"
COMMIT_INPUT="$(input COMMIT)"
BRANCH="$(input BRANCH)"
TOKEN="$(input TOKEN)"

: "${WINDOW:=annual}"
: "${CANON:=false}"
: "${OUTPUT_DIR:=dragon}"
: "${COMMIT_INPUT:=true}"
: "${BRANCH:=output}"

if [ -z "$LOGIN" ]; then
    LOGIN="${GITHUB_REPOSITORY_OWNER:-}"
fi

if [ -z "$LOGIN" ]; then
    echo "git-dragon: no login provided; set the login input or run from a profile repository" >&2
    exit 1
fi

if [ -z "$TOKEN" ]; then
    TOKEN="${GITHUB_TOKEN:-}"
fi

if [ -n "$TOKEN" ]; then
    export GITHUB_TOKEN="$TOKEN"
fi

# The workspace is owned by the runner user, not the container user.
git config --global --add safe.directory "${GITHUB_WORKSPACE:-$PWD}" 2>/dev/null || true
git config --global --add safe.directory '*' 2>/dev/null || true

echo "git-dragon: collecting activity for @${LOGIN} (${WINDOW} window)"
git-dragon collect "$LOGIN" --window "$WINDOW" --output /tmp/git-dragon-snapshot.json

CANON_FLAG=""
if [ "$CANON" = "true" ]; then
    CANON_FLAG="--canon"
fi

echo "git-dragon: rendering dragon"
git-dragon render /tmp/git-dragon-snapshot.json \
    --output-dir "$OUTPUT_DIR" \
    $CANON_FLAG

if [ "$COMMIT_INPUT" != "true" ]; then
    echo "git-dragon: commit disabled; assets left in ${OUTPUT_DIR}"
    exit 0
fi

if [ -z "$TOKEN" ] || [ -z "${GITHUB_REPOSITORY:-}" ]; then
    echo "git-dragon: no token available; skipping branch publication" >&2
    exit 0
fi

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git remote set-url origin "https://x-access-token:${TOKEN}@github.com/${GITHUB_REPOSITORY}.git"

mkdir -p /tmp/git-dragon-assets
cp -r "${OUTPUT_DIR}/." /tmp/git-dragon-assets/

git checkout --orphan "$BRANCH" >/dev/null 2>&1 || git checkout -B "$BRANCH"
git rm -rf -- . >/dev/null 2>&1 || true

mkdir -p "$OUTPUT_DIR"
cp -r /tmp/git-dragon-assets/. "$OUTPUT_DIR"/
git add "$OUTPUT_DIR"

if git diff --cached --quiet; then
    echo "git-dragon: no changes to commit"
else
    git commit -m "chore(dragon): refresh profile dragon assets"
    git push origin "HEAD:${BRANCH}" --force
    echo "git-dragon: assets published to branch ${BRANCH}"
fi
