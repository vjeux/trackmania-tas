#!/bin/bash
# Write the branch build's shim md5 sidecar (<lib>.md5) beside libforkshim.so. Run after `cargo build --release` in
# tools/search (the bootstrap does). forkoracle refuses a server whose on-disk shim md5 differs from this sidecar (or
# from FK_SHIM_MD5), and whose MAPPED inode differs from the file's -- the 24 phantom-finish rule (COMMON-RULES, 2026-09-09).
set -e
D=$(cd "$(dirname "$0")" && pwd)/target/release
md5sum "$D/libforkshim.so" | awk '{print $1}' > "$D/libforkshim.so.md5.tmp" && mv "$D/libforkshim.so.md5.tmp" "$D/libforkshim.so.md5"
echo "libforkshim.so md5 $(cat "$D/libforkshim.so.md5") (git $(git -C "$D/../.." rev-parse --short HEAD 2>/dev/null))"
