#!/bin/sh
# Check that the packaging metadata agrees with itself.
#
#   sh packaging/check.sh
#
# The rule these all serve: the AppStream component, the desktop entry, the
# icons and the polkit policy must share one name, the application id, and
# the helper path the policy names must be the one the packages install and
# the one the runner looks for. None of this needs rpm, dpkg or a display, so
# it runs on every push.

set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

fail() { printf 'packaging: %s\n' "$1" >&2; exit 1; }
ok()   { printf '  ok  %s\n' "$1"; }

APP_ID=io.github.spillebulle.bapstore
HELPER=/usr/lib/bap-store/bap-helper

metainfo="packaging/$APP_ID.metainfo.xml"
desktop="packaging/$APP_ID.desktop"
policy="packaging/$APP_ID.policy"

[ -f "$metainfo" ] || fail "no AppStream file at $metainfo"
[ -f "$desktop" ]  || fail "no desktop entry at $desktop"
ok "metainfo and desktop entry are named for $APP_ID"

id=$(sed -n 's/.*<id>\(.*\)<\/id>.*/\1/p' "$metainfo" | head -1)
[ "$id" = "$APP_ID" ] || fail "<id> is '$id', expected '$APP_ID'"
ok "<id> is $APP_ID"

launchable=$(sed -n 's/.*<launchable[^>]*>\(.*\)<\/launchable>.*/\1/p' "$metainfo" | head -1)
[ "$launchable" = "$APP_ID.desktop" ] || \
    fail "<launchable> is '$launchable' but the desktop entry installs as '$APP_ID.desktop'"
ok "<launchable> resolves to the installed desktop entry"

icon=$(sed -n 's/^Icon=\(.*\)$/\1/p' "$desktop" | head -1)
[ "$icon" = "$APP_ID" ] || fail "Icon is '$icon' but the icons install as '$APP_ID.png'"
ok "Icon= resolves to the installed icons"

for size in 16 32 48 64 128 256; do
    [ -f "assets/icons/bap-store-$size.png" ] || \
        fail "assets/icons/bap-store-$size.png is missing, and every package installs it"
done
ok "all six icon sizes are present"

# The polkit policy is written by the transaction module; until it exists the
# packages cannot grant the helper anything, and that is a failure worth
# naming rather than a package that installs and then cannot install anything.
[ -f "$policy" ] || fail "no polkit policy at $policy"
grep -q "org.freedesktop.policykit.exec.path\">$HELPER<" "$policy" || \
    fail "$policy does not name $HELPER as the helper path"
ok "the policy names $HELPER"

grep -q "$HELPER" packaging/linux/build-packages.sh || \
    fail "build-packages.sh does not install the helper to $HELPER"
grep -q "$HELPER" packaging/linux/PKGBUILD || \
    fail "PKGBUILD does not install the helper to $HELPER"
grep -rq "$HELPER" crates/bap-core/src/transaction/ || \
    fail "the runner does not look for the helper at $HELPER"
ok "packages and the runner agree on the helper path"

# The version is stated in four places; the release test checks three of them
# against Cargo.toml and this checks the fourth, the AppStream release list.
version=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
grep -q "<release version=\"$version\"" "$metainfo" || \
    fail "$metainfo has no <release version=\"$version\">; add one for this version"
ok "metainfo lists release $version"
