#!/bin/sh
# Installs or updates Keygloss from the latest GitHub release: the .deb on Debian and Ubuntu,
# the .rpm on Fedora and openSUSE.
#   curl -fsSL https://raw.githubusercontent.com/xArchange1/keygloss/main/packaging/linux/install.sh | sh
set -eu

REPO=xArchange1/keygloss

fail() {
    echo "keygloss: $*" >&2
    exit 1
}

[ "$(uname -m)" = x86_64 ] || fail "only x86_64 builds are published"
command -v curl >/dev/null || fail "curl is required"

if command -v apt-get >/dev/null; then
    pattern='_amd64\.deb$'
elif command -v dnf >/dev/null || command -v zypper >/dev/null; then
    pattern='\.x86_64\.rpm$'
else
    fail "no apt, dnf or zypper found: download a package from https://github.com/$REPO/releases"
fi

url=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
    grep -o '"browser_download_url": *"[^"]*"' | cut -d'"' -f4 | grep -E "$pattern" | head -n1)
[ -n "$url" ] || fail "the latest release has no package for this system"

dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
file="$dir/$(basename "$url")"
echo "Downloading $url"
curl -fL --progress-bar -o "$file" "$url"
# The package manager reads the file as another user (apt's sandbox).
chmod 755 "$dir"
chmod 644 "$file"

sudo=
[ "$(id -u)" -eq 0 ] || sudo=sudo
if command -v apt-get >/dev/null; then
    $sudo apt-get install -y "$file"
elif command -v dnf >/dev/null; then
    $sudo dnf install -y "$file"
else
    $sudo zypper --non-interactive install --allow-unsigned-rpm "$file"
fi

echo
echo "Keygloss is installed: start it from the application menu."
case "${XDG_CURRENT_DESKTOP:-}" in
    *KDE*) ;;
    *) echo "Note: Keygloss needs KDE Plasma 6 on Wayland, other desktops are not supported yet." ;;
esac
