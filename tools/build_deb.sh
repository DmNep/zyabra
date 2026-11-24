#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1)
arch=${DEB_ARCH:-$(dpkg --print-architecture)}
out="$root/target/debian"
stage="$out/zyabra_${version}_${arch}"

if [ -z "$version" ]; then
    echo "Cargo.toml: version not found" >&2
    exit 1
fi
command -v dpkg-deb >/dev/null 2>&1 || {
    echo "dpkg-deb is required" >&2
    exit 1
}

cargo build --release --locked --manifest-path "$root/Cargo.toml"
binary="$root/target/release/zyabra"
[ -f "$binary" ] || {
    echo "missing $binary" >&2
    exit 1
}

rm -rf "$stage"
mkdir -p "$stage/DEBIAN" "$stage/usr/bin" "$stage/usr/lib/systemd/user" \
    "$stage/usr/lib/zyabra" "$stage/usr/lib/udev/rules.d"

cat > "$stage/DEBIAN/control" <<EOF
Package: zyabra
Version: $version
Section: utils
Priority: optional
Architecture: $arch
Depends: libc6, systemd, udev
Maintainer: Zyabra <zyabra@localhost>
Description: Local RU/EN keyboard layout switcher
 Rewrites a word typed in the wrong layout, then switches the layout.
 Nothing is stored and no network connection is opened.
EOF

install -m 0755 "$binary" "$stage/usr/bin/zyabra"
install -m 0644 "$root/packaging/zyabra.service" \
    "$stage/usr/lib/systemd/user/zyabra.service"
install -m 0755 "$root/packaging/launch" "$stage/usr/lib/zyabra/launch"
install -m 0755 "$root/packaging/session-unit" "$stage/usr/lib/zyabra/session-unit"
install -m 0644 "$root/packaging/70-zyabra.rules" \
    "$stage/usr/lib/udev/rules.d/70-zyabra.rules"
for script in postinst prerm postrm; do
    install -m 0755 "$root/packaging/$script" "$stage/DEBIAN/$script"
done

dpkg-deb --build --root-owner-group "$stage" "$out/zyabra_${version}_${arch}.deb"
echo "Built: $out/zyabra_${version}_${arch}.deb"
