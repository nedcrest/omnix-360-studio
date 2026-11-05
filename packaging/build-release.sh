#!/bin/sh
# Builds the release assets into dist/: the binary archive (with install.sh), the
# AppImage, the Flatpak with its ffmpeg's sources, the Windows zip with its
# ffmpeg's sources, install.sh on its own and SHA256SUMS. Needs docker, flatpak with org.flatpak.Builder (flatpak install
# --user flathub org.flatpak.Builder) and network access.
set -eu
cd "$(dirname "$0")/.."
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
dist=dist
rm -rf "$dist" && mkdir -p "$dist"

docker build -q -t omnix-build:jammy packaging >/dev/null
docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$PWD":/src -w /src \
    -e CARGO_TARGET_DIR=/src/target/jammy omnix-build:jammy cargo build --release --locked
exe=target/jammy/release/omnix
strip "$exe"

# Archive: omnix-x86_64-linux/ with the binary, install script, menu entry and icons.
# The asset names carry no version, so .../releases/latest/download/<name> stays valid.
pkg="$dist/omnix-x86_64-linux"
mkdir -p "$pkg"
cp "$exe" packaging/install.sh packaging/omnix.desktop packaging/omnix.png \
    packaging/omnix.svg README.md LICENSE-MIT LICENSE-APACHE "$pkg/"
chmod 755 "$pkg/omnix" "$pkg/install.sh"
# The built-in fonts are inside the binary; their licences go beside it.
for d in fonts/*/; do mkdir -p "$pkg/fonts/$(basename "$d")" && cp "$d"*.txt "$pkg/fonts/$(basename "$d")/"; done
cp fonts/README.md "$pkg/fonts/"
tar -czf "$dist/omnix-x86_64-linux.tar.gz" -C "$dist" omnix-x86_64-linux
cp packaging/install.sh "$dist/install.sh"

# AppImage. ffmpeg and ALSA come from the system: a bundled ffmpeg would lose the
# GPU decoders (VA-API, Vulkan, CUDA) that full-res stills and export depend on.
app="$dist/Omnix.AppDir"
mkdir -p "$app/usr/bin" "$app/usr/share/applications" "$app/usr/share/icons/hicolor/256x256/apps"
cp "$exe" "$app/usr/bin/"
cp packaging/AppRun "$app/AppRun" && chmod 755 "$app/AppRun"
cp packaging/omnix.desktop "$app/" && cp packaging/omnix.desktop "$app/usr/share/applications/"
cp packaging/omnix.png "$app/" && cp packaging/omnix.png "$app/usr/share/icons/hicolor/256x256/apps/"
ln -s omnix.png "$app/.DirIcon"
for d in fonts/*/; do mkdir -p "$app/usr/share/licenses/omnix/fonts/$(basename "$d")" && cp "$d"*.txt "$app/usr/share/licenses/omnix/fonts/$(basename "$d")/"; done
cp fonts/README.md "$app/usr/share/licenses/omnix/fonts/"
tool=target/appimagetool-x86_64.AppImage
if [ ! -x "$tool" ]; then
    curl -fsSL https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage -o "$tool"
    chmod 755 "$tool"
fi
ARCH=x86_64 VERSION="$version" "$tool" --appimage-extract-and-run -n "$app" "$dist/Omnix-x86_64.AppImage" >/dev/null
rm -rf "$app" "$pkg"

# Flatpak: the same binary with an ffmpeg of its own (see the manifest), as a
# single-file bundle that fetches its runtime from Flathub on install.
fp=target/flatpak
id=local.omnix.Omnix
flatpak run --filesystem="$PWD" org.flatpak.Builder --user --install-deps-from=flathub --force-clean \
    --state-dir="$fp/state" --repo="$fp/repo" "$fp/build" "packaging/flatpak/$id.yml"
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo "$fp/repo" "$dist/Omnix-x86_64.flatpak" "$id"
# Its ffmpeg is GPL: the sources it was built from go beside it.
src="$dist/Omnix-flatpak-ffmpeg-sources"
mkdir -p "$src"
cp -r "$fp/state/downloads/." "$src/" 2>/dev/null || true
# Each git source at the commit the manifest pins, from flatpak-builder's mirror.
awk '/url: .*\.git$/ {url=$2} /commit:/ {print url, $2}' "packaging/flatpak/$id.yml" | while read -r url commit; do
    name="$(basename "$url" .git)"
    repo="$(find "$fp/state/git" -maxdepth 1 -name "*$name.git" | head -n 1)"
    git -C "$repo" archive --format=tar.gz --prefix="$name-$commit/" -o "$PWD/$src/$name-$commit.tar.gz" "$commit"
done
tar -cf "$dist/Omnix-flatpak-ffmpeg-sources.tar" -C "$dist" "$(basename "$src")"
rm -rf "$src"

# Windows: the zip with its own ffmpeg, and that ffmpeg's sources (see
# packaging/windows/build.sh), built fresh so they match.
sh packaging/windows/build.sh --fresh-ffmpeg
cp target/windows/Omnix-x86_64-windows.zip target/windows/Omnix-windows-ffmpeg-sources.tar "$dist/"

(cd "$dist" && sha256sum omnix-x86_64-linux.tar.gz Omnix-x86_64.AppImage Omnix-x86_64.flatpak \
    Omnix-flatpak-ffmpeg-sources.tar Omnix-x86_64-windows.zip Omnix-windows-ffmpeg-sources.tar \
    install.sh > SHA256SUMS)
ls -l "$dist"
