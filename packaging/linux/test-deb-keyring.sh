#!/bin/sh
# Install Brokey's .deb beside another application from the archive, in both
# orders, and check the archive's keyring survives every step.
#
#   sudo sh packaging/linux/test-deb-keyring.sh dist/brokey_<version>_amd64.deb
#
# Every .deb from the archive enrols the machine with the same keyring at the
# same path. Umber and Muster 0.x ship it as a file dpkg owns, and dpkg refuses
# a second package that ships it too, so Brokey 0.1.2 would not install beside
# Umber. Runs as root on a throwaway machine (CI): it installs and removes
# packages and deletes the archive's source file and keyring.

set -eu

deb=$1
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
keyring=/usr/share/keyrings/spillebulle-archive.gpg
sources=/etc/apt/sources.list.d/spillebulle.sources
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fail() { printf 'deb keyring: %s\n' "$1" >&2; exit 1; }
ok()   { printf '  ok  %s\n' "$1"; }

gpg --dearmor < "$root/packaging/linux/spillebulle-archive.asc" > "$work/expected.gpg"

# A stand-in for Umber 0.x: a package that owns the keyring.
mkdir -p "$work/other/DEBIAN" "$work/other/usr/share/keyrings"
cp "$work/expected.gpg" "$work/other$keyring"
cat > "$work/other/DEBIAN/control" <<EOF
Package: spillebulle-other
Version: 0.1.0
Architecture: all
Maintainer: Spillebulle <spillebulle@gmail.com>
Description: Stand-in for an application that ships the archive keyring
EOF
dpkg-deb --build --root-owner-group "$work/other" "$work/other.deb" >/dev/null

# Brokey's own dependencies (WebKit and so on) are not what is under test.
install_brokey() { dpkg -i --force-depends "$deb" >/dev/null || fail "$1"; }
key_is_there() {
    cmp -s "$keyring" "$work/expected.gpg" || fail "$1"
}
clean() {
    dpkg --purge --force-depends brokey spillebulle-other >/dev/null 2>&1 || true
    rm -f "$keyring" "$sources"
}

clean
dpkg -i "$work/other.deb" >/dev/null
install_brokey "Brokey does not install beside a package that owns the keyring"
ok "installs beside a package that owns the keyring"
[ -f "$sources" ] || fail "no source file after installing Brokey"
key_is_there "the keyring changed when Brokey was installed"
dpkg -r spillebulle-other >/dev/null
key_is_there "removing the other package took the keyring away from Brokey"
ok "the keyring survives removing the package that owned it"
dpkg --purge --force-depends brokey >/dev/null
key_is_there "purging Brokey removed the keyring"
[ -f "$sources" ] || fail "purging Brokey removed the source file"
ok "purging Brokey leaves the keyring and the source file"

clean
install_brokey "Brokey does not install on its own"
key_is_there "Brokey installed on its own writes no keyring"
[ -f "$sources" ] || fail "no source file after installing Brokey on its own"
if dpkg -S "$keyring" >/dev/null 2>&1; then
    fail "the keyring belongs to a package, so the next application cannot ship it"
fi
ok "installed on its own it writes the keyring and owns nothing"
dpkg -i "$work/other.deb" >/dev/null || fail "a package that owns the keyring does not install after Brokey"
dpkg -r spillebulle-other >/dev/null
key_is_there "removing a package installed after Brokey took the keyring away"
ok "the keyring survives a package installed after Brokey and removed"

# The source the postinst wrote, with the keyring it names, is one apt accepts.
apt-get update --error-on=any \
    -o Dir::Etc::sourcelist="$sources" -o Dir::Etc::sourceparts=- \
    -o APT::Get::List-Cleanup=0 >/dev/null || fail "apt does not accept the archive with this keyring"
ok "apt reads the archive with the keyring and source Brokey wrote"

clean
