#!/bin/sh
# orange.sh DIR TAG NAME... : per camera, the orange-pixel mask of each frame
# (colorkey on the Level1 orange), its coverage, and how much the mask moved
# between frames — background-free motion evidence.
D=$1; TAG=$2; shift 2
F=/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin/ffmpeg.exe
W=$(echo "$D" | sed 's|^/mnt/c/|C:\\|; s|/|\\|g')
for name in "$@"; do
  for f in 1 2 3; do
    $F -y -loglevel error -i "$W\\cmp-$TAG$name$f-o.png" -vf "crop=1600:1000:1120:540,colorkey=0xE8701C:similarity=0.12:blend=0,alphaextract,negate,scale=800:500" "$W\\mask-$name$f.png"
  done
  cov=""
  for f in 1 2 3; do
    v=$($F -loglevel info -i "$W\\mask-$name$f.png" -vf "signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=-" -f null - 2>/dev/null | grep -o "YAVG=[0-9.]*" | head -1 | cut -d= -f2)
    cov="$cov $v"
  done
  mv=""
  for p in "1 2" "2 3" "1 3"; do
    set -- $p
    v=$($F -loglevel info -i "$W\\mask-$name$1.png" -i "$W\\mask-$name$2.png" -filter_complex "[0][1]blend=all_mode=difference,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=-" -f null - 2>/dev/null | grep -o "YAVG=[0-9.]*" | head -1 | cut -d= -f2)
    mv="$mv $v"
  done
  echo "$name: orange coverage (x/255 of frame)$cov | mask moved$mv"
done
