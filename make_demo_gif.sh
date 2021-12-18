#!/bin/bash
set -euxo pipefail

function cleanup {
  rm -f palette.png
}

trap cleanup EXIT

ffmpeg -y -i $1 -vf palettegen palette.png
ffmpeg -y -i $1 -i palette.png -filter_complex paletteuse -r 10 $1_new.gif
mogrify -layers 'optimize' -fuzz '7%' $1_new.gif
