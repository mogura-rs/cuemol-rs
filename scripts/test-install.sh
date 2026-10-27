#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/cuemol-install-test.XXXXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/release" "$work/input" "$work/bin"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  *) exit 1 ;;
esac
printf '#!/bin/sh\nprintf "installed\\n"\n' > "$work/input/cuemol"
tar -czf "$work/release/cuemol-$target.tar.gz" -C "$work/input" cuemol
(cd "$work/release" && shasum -a 256 "cuemol-$target.tar.gz" > SHA256SUMS)
CUEMOL_RELEASE_URL="file://$work/release" CUEMOL_INSTALL_DIR="$work/bin" sh "$root/install.sh"
test "$("$work/bin/cuemol")" = installed
printf '%064d  cuemol-%s.tar.gz\n' 0 "$target" > "$work/release/SHA256SUMS"
if CUEMOL_RELEASE_URL="file://$work/release" CUEMOL_INSTALL_DIR="$work/bin" sh "$root/install.sh"; then
  printf 'Installer accepted a bad checksum\n' >&2
  exit 1
fi
test "$("$work/bin/cuemol")" = installed
if [ "$#" -gt 0 ]; then
  cp "$1" "$work/bin/cuemol"
  (cd "$work/release" && shasum -a 256 "cuemol-$target.tar.gz" > SHA256SUMS)
  CUEMOL_RELEASE_URL="file://$work/release" "$work/bin/cuemol" self-upgrade
  test "$("$work/bin/cuemol")" = installed
fi
