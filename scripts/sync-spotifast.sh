#!/usr/bin/env bash
# Brings the newest Spotifast release into Spindle.
#
#   scripts/sync-spotifast.sh            newest Spotifast release
#   scripts/sync-spotifast.sh v0.15.0    that release
#
# How it works (see scripts/rename-upstream.py for the why): the release is
# renamed to Spindle's branding and committed on the spotifast-renamed
# branch, whose history is Spotifast's history renamed, then that branch is
# merged into the current branch. Both sides carry the same renames, so a
# conflict is a real one: Spindle's own change meets Spotifast's. The
# version becomes <major>.<minor>.<patch * 100 + 1>: above the Spotifast
# release it carries, below the next one, so Spindle's updater offers it.
#
# Exit codes: 0 merged (or nothing to do), 2 conflicts left in the tree for
# a person to resolve (the merge is not committed), 1 anything else.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
upstream_url=https://github.com/crmne/spotifast.git
renamed_branch=spotifast-renamed

git remote get-url upstream >/dev/null 2>&1 || git remote add upstream "$upstream_url"
# Spotifast's tags live under refs/tags/spotifast/, apart from Spindle's
# own v* tags: the two would otherwise mix (the newest "v" tag would be
# Spindle's) and collide (Spindle's v0.15.1 against Spotifast's).
git fetch --quiet --no-tags upstream '+refs/tags/v*:refs/tags/spotifast/v*'

if [ $# -ge 1 ]; then
    tag=$1
else
    # The newest release tag: a plain v<major>.<minor>.<patch>, no pre-release.
    tag=$( (git for-each-ref --format='%(refname:lstrip=3)' 'refs/tags/spotifast/v*' \
        | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' || true) | sort -V | tail -1)
fi
[ -n "$tag" ] || { echo "no Spotifast release tag found" >&2; exit 1; }
upstream_commit=$(git rev-parse "refs/tags/spotifast/$tag^{commit}")
upstream_version=${tag#v}

if git merge-base --is-ancestor "$upstream_commit" HEAD; then
    echo "Spotifast $tag is already in $(git rev-parse --abbrev-ref HEAD)"
    exit 0
fi

# The renamed branch: fetch it if the remote has it, else it must exist.
git fetch --quiet origin "$renamed_branch:$renamed_branch" 2>/dev/null || true
git rev-parse --verify --quiet "$renamed_branch" >/dev/null \
    || { echo "branch $renamed_branch is missing" >&2; exit 1; }
previous_renamed=$(git rev-parse "$renamed_branch")

# Already renamed (a previous run that stopped at conflicts): reuse it.
renamed_commit=$( (git log --format=%H --min-parents=2 "$renamed_branch" || true) \
    | while read -r commit; do
        if [ "$(git rev-parse "$commit^2")" = "$upstream_commit" ]; then echo "$commit"; break; fi
      done || true)
if [ -z "$renamed_commit" ]; then
# Rename the release in a scratch worktree and commit it on the renamed
# branch, with the release as a second parent so history shows it.
scratch=$(mktemp -d)
trap 'git worktree remove --force "$scratch" 2>/dev/null || true' EXIT
git worktree add --quiet --detach "$scratch" "$upstream_commit"
python3 scripts/rename-upstream.py "$scratch" >/dev/null
mkdir -p "$scratch/scripts"
cp scripts/rename-upstream.py "$scratch/scripts/rename-upstream.py"
renamed_tree=$(git -C "$scratch" add -A && git -C "$scratch" write-tree)
renamed_commit=$(git commit-tree "$renamed_tree" -p "$previous_renamed" -p "$upstream_commit" \
    -m "Rename Spotifast $upstream_version to Spindle (scripts/rename-upstream.py)")
git branch -f "$renamed_branch" "$renamed_commit"
git worktree remove --force "$scratch"
trap - EXIT
fi

# Merge it. Every file merges three ways; the version line conflicts on
# every release and is set by the rule above.
IFS=. read -r major minor patch <<<"$upstream_version"
spindle_version="$major.$minor.$((patch * 100 + 1))"
git merge --no-commit --no-ff "$renamed_commit" >/dev/null 2>&1 || true

# Spindle's README is its own (Nick wants only Spindle there), so Spotifast's
# README changes never come in.
git checkout HEAD -- README.md
git add README.md

# Cargo.toml: merged like any file, so Spindle's own dependencies (qrcode,
# the vendored eframe) stay. A conflict that only touches version lines is
# settled here; any other is left for a person.
python3 scripts/settle-version.py Cargo.toml "$spindle_version"
if ! grep -q '^<<<<<<< ' Cargo.toml; then
    git add Cargo.toml
fi

# vendor/eframe is a copy of eframe at the crmne/egui revision Spotifast
# pins. If Spotifast moved the revision, the copy must be refreshed first.
pinned=$(grep -m1 -oE 'github.com/crmne/egui", rev = "[0-9a-f]+' Cargo.toml | grep -oE '[0-9a-f]{40}' || true)
if [ -n "$pinned" ] && [ "$pinned" != "$(cat vendor/eframe/SPINDLE_EGUI_REV)" ]; then
    echo "Spotifast moved crmne/egui to $pinned: repackage vendor/eframe from it (CLAUDE.md), then commit" >&2
    exit 2
fi

# Cargo.lock: Spotifast's, then cargo adds what Spindle's Cargo.toml has on
# top (qrcode, the path eframe) and keeps every other locked version.
git show "$renamed_commit:Cargo.lock" > Cargo.lock
if ! grep -q '^<<<<<<< ' Cargo.toml; then
    cargo metadata --format-version 1 >/dev/null
    git add Cargo.lock
fi

conflicts=$(git diff --name-only --diff-filter=U)
if [ -n "$conflicts" ]; then
    echo "Spotifast $tag merged with conflicts to resolve, then commit:"
    echo "$conflicts"
    exit 2
fi
git commit --quiet -m "Merge Spotifast $upstream_version

Spindle $spindle_version carries Spotifast $upstream_version."
echo "Merged Spotifast $tag as Spindle $spindle_version"
