#!/bin/sh
# Cut a release.
#
#   sh tools/release.sh 0.1.1
#   sh tools/release.sh 0.1.1 --dry-run
#
# In order, stopping at the first thing that is not right:
#
#   1. the working tree is clean and on main
#   2. Cargo.toml, tauri.conf.json, package.json and the metainfo all say this version
#   3. CHANGELOG.md has a section for it; the notes are printed
#   4. fmt, clippy, the tests, the page's lint and the packaging check: the same gates CI runs
#   5. the branch is pushed and CI passes on that very commit
#   6. an annotated tag is written and pushed
#
# Pushing the tag is the whole of "make a release": the Release workflow
# builds the binaries, packages them and publishes the notes. Nothing here
# uploads anything, so this script cannot half-publish.

set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

fail() { printf 'release: %s\n' "$1" >&2; exit 1; }
step() { printf '==> %s\n' "$1"; }

version=
dry_run=0
skip_tests=0
skip_ci=0
for arg in "$@"; do
    case "$arg" in
        --dry-run)    dry_run=1 ;;
        --skip-tests) skip_tests=1 ;;
        --skip-ci)    skip_ci=1 ;;
        -*)           fail "unknown option '$arg'" ;;
        *)            version=$arg ;;
    esac
done

[ -n "$version" ] || fail "usage: $0 <version> [--dry-run] [--skip-tests] [--skip-ci]"
case "$version" in
    v*) fail "give the version as ${version#v}, without the leading v" ;;
esac
tag="v$version"

step 'checking the working tree'
[ -z "$(git status --porcelain)" ] || \
    fail 'the working tree has uncommitted changes. A tag points at a commit, so anything not committed is not in the release.'
branch=$(git rev-parse --abbrev-ref HEAD)
[ "$branch" = main ] || fail "on branch '$branch', not main. Releases are cut from main."
[ -z "$(git tag -l "$tag")" ] || \
    fail "$tag already exists. Bump the version rather than moving a tag somebody may already have fetched."

step 'checking the version'
declared=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
[ "$declared" = "$version" ] || \
    fail "Cargo.toml says $declared but you asked for $version. Edit [workspace.package] version, commit it, then run this again."
conf=$(python3 -c "import json;print(json.load(open('crates/brokey/tauri.conf.json'))['version'])")
[ "$conf" = "$version" ] || fail "crates/brokey/tauri.conf.json says $conf, not $version."
pkg=$(python3 -c "import json;print(json.load(open('package.json'))['version'])")
[ "$pkg" = "$version" ] || fail "package.json says $pkg, not $version."
grep -q "<release version=\"$version\"" packaging/io.github.spillebulle.brokey.metainfo.xml || \
    fail "the metainfo has no <release version=\"$version\">."

step 'reading the release notes'
notes=$(sh tools/release-notes.sh "$version") || \
    fail "CHANGELOG.md has no notes under '## $version'."
printf '%s\n' "$notes" | grep -q '^\s*- ' || \
    fail "the '## $version' section has no bullet points."
printf '%s\n\n' "$notes"

if [ "$skip_tests" -eq 0 ]; then
    step 'running the gates'
    cargo fmt --all --check
    cargo clippy --workspace --all-targets
    cargo test --workspace
    npm run lint
    npm run check:design
    sh packaging/check.sh
fi

if [ "$dry_run" -eq 1 ]; then
    step 'dry run: stopping before the push'
    exit 0
fi

step 'pushing main'
git push origin main

if [ "$skip_ci" -eq 0 ]; then
    step 'waiting for CI on this commit'
    command -v gh >/dev/null 2>&1 || fail 'the GitHub CLI (gh) is needed to wait for CI; pass --skip-ci to tag without it.'
    sha=$(git rev-parse HEAD)
    for _ in $(seq 60); do
        run=$(gh run list --workflow=ci.yml --commit "$sha" --json databaseId,status,conclusion --jq '.[0]' 2>/dev/null || true)
        [ -n "$run" ] && break
        sleep 5
    done
    [ -n "$run" ] || fail 'CI did not start for this commit.'
    id=$(printf '%s' "$run" | python3 -c 'import json,sys;print(json.load(sys.stdin)["databaseId"])')
    gh run watch "$id" --exit-status || fail 'CI failed on this commit. Fix it before tagging.'
fi

step "tagging $tag"
git tag -a "$tag" -m "$notes"
git push origin "$tag"
step "pushed $tag; the Release workflow takes it from here"
