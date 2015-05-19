#!/bin/bash
set -e
convert ../GroupProjectData/CleanSignal_Spectrogram.jpg -crop 651x343+108+31 input.png
RUST_LOG=inv_fft=trace LD_LIBRARY_PATH=/home/tpg/apps/lib cargo run --release
#avconv -y -f s16le -ac 1 -ar 44.1k -i out.pcm out.wav
avconv -y -f s16le -ac 1 -ar 96k -i out.pcm out.wav
