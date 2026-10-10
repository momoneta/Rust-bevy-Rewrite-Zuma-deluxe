#!/bin/sh
# Renders the eight subsongs of the original music/zuma.mo3 (an Un4seen MO3
# tracker module, which Bevy cannot decode) to OGG files the game plays:
#
#   zuma_0  main theme (menus and levels)      zuma_4  game over
#   zuma_1  (order 24)                         zuma_5  danger: chain near the hole
#   zuma_2  (order 28)                         zuma_6  life lost
#   zuma_3  stage / temple complete            zuma_7  level complete
#
# Needs openmpt123 and oggenc (Debian/Ubuntu: apt install openmpt123 vorbis-tools).
# Usage: tools/convert_music.sh [path/to/data]
set -e
DATA="${1:-data}"
SRC="$DATA/music/zuma.mo3"
[ -f "$SRC" ] || SRC=$(find "$DATA" -iname 'zuma.mo3' | head -n 1)
[ -f "$SRC" ] || { echo "zuma.mo3 not found under $DATA" >&2; exit 1; }
OUT=$(dirname "$SRC")
for i in 0 1 2 3 4 5 6 7; do
    openmpt123 --batch --quiet --subsong "$i" --samplerate 44100 --force -o "$OUT/zuma_$i.wav" "$SRC"
    oggenc --quiet -q 5 -o "$OUT/zuma_$i.ogg" "$OUT/zuma_$i.wav"
    rm -f "$OUT/zuma_$i.wav"
    echo "wrote $OUT/zuma_$i.ogg"
done
