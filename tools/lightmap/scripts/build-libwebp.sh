#!/bin/bash
# Build libwebp's encoder (1.4.0 from fbsource/third-party/webp; the game uses 1.6.0) as
# vendor/libwebp140.a for the baker's atlas encoder. Usage: scripts/build-libwebp.sh [SRC_DIR]
set -e
W=${1:-$HOME/fbsource/third-party/webp}
OUT=$(dirname "$0")/../vendor
mkdir -p "$OUT" /tmp/libwebp-build/inc/src/webp
cp "$W"/src/webp/*.h /tmp/libwebp-build/inc/src/webp/
cat > /tmp/libwebp-build/inc/src/webp/config.h <<'CFG'
#define WEBP_HAVE_SSE2 1
#define WEBP_HAVE_SSE41 1
#define HAVE_STDINT_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRING_H 1
#define HAVE_UNISTD_H 1
#define PACKAGE_VERSION "1.4.0"
CFG
cd /tmp/libwebp-build && rm -f *.o
for f in "$W"/src/enc/*.c "$W"/src/dsp/*.c "$W"/src/utils/*.c "$W"/sharpyuv/*.c; do
  gcc -O2 -fPIC -msse2 -msse4.1 -I"$W" -Iinc -c "$f" -o "$(basename "$f" .c).o"
done
ar rcs "$OUT/libwebp140.a" *.o
echo "wrote $OUT/libwebp140.a ($(ls *.o | wc -l) objects)"
