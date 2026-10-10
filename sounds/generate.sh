#!/usr/bin/env bash
# Generates beep clips with ffmpeg, to be used as changes. Only the clips are generated, so the
# sounds live in this script and no audio needs to be committed.
#
# Usage: generate.sh [changes-dir]   (default: ./changes)
#
# `fix-whxm4-voice generate` runs this script; it also runs on its own.
#
# Theme: modern, like the tones of current phone UIs. Every tone is warm and soft, and rings out like
# something struck instead of holding a level. Pitch moves like an ease-out animation: at full speed
# from the first moment, then decelerating onto its note. No echo or other effects.
set -euo pipefail

out=${1:-changes}

# Sample rate of the generated clips; the tool converts them to the stock format anyway.
rate=48000

# Timbre of a tone: a sine with an overtone at twice its pitch, which warms it up at the start and
# fades, and a low tone an octave below for weight. Levels are relative to the sine; times are time
# constants in seconds.
overtone_level=0.3
overtone_decay=0.06
low_level=0.4
# Ring: the tone swells in over the onset time, then decays like something struck.
onset=0.006
decay=0.07
# Lowpass cutoff in Hz, which keeps the tone warm and dull rather than bright.
lowpass=3000
# Fade-in and fade-out of a tone, in seconds, and their curves (see ffmpeg's afade). They only make
# sure the tone starts and ends without a click; the ring above shapes what you hear.
attack=0.004
attack_curve=qsin
release=0.08
release_curve=hsin
# Final gain in dB. At -7 dB the tones average about -17 dB while sounding; stock speech averages
# about -20 dB.
volume=-7dB
# Length of a dot, a short tone at one pitch, in seconds.
dot_length=0.05
# Pause after a dot, in seconds.
pause=0.06
# Scratchy effect for bad news, as an ffmpeg filter: random grain mixed into the tone, like a crackly
# bad radio, raised 4.3 dB to make up for the level the grain takes away.
scratchy="aeval='val(0)*(1-0.9*random(0))',volume=4.3dB"

# Notes, in Hz.
a3=220
e4=329.63
a4=440
e5=659.25
a5=880

# Prints a filter chain making one tone, labelled [$5], $4 seconds long. Its pitch starts at $1 Hz
# and moves onto $2 Hz, arriving after $3 seconds, along curve $6:
#   out     ease-out (the default): to - (to - from) * (1 - u)^2, with u = t / glide. It moves
#           fastest at the start and slows down onto the note.
#   in-out  ease-in-out: from + (to - from) * (3u^2 - 2u^3). It starts at rest on the first pitch,
#           speeds up, and slows down onto the note.
# With $1 = $2 it holds one pitch.
tone() {
    local from=$1 to=$2 glide=$3 dur=$4 label=$5 curve=${6:-out}
    # cycles is the integral of the frequency over time, so the phase stays continuous while the
    # pitch moves.
    local u="(min(t,$glide)/$glide)" cycles
    case $curve in
        out) cycles="($to*t-($to-$from)*$glide/3*(1-(1-$u)^3))" ;;
        in-out) cycles="($from*t+($to-$from)*($glide*($u^3-$u^4/2)+max(t-$glide,0)))" ;;
        *) echo "unknown curve: $curve" >&2 && return 1 ;;
    esac
    local overtone="$overtone_level*exp(-t/$overtone_decay)*sin(4*PI*$cycles)"
    local low="$low_level*sin(PI*$cycles)"
    local wave="sin(2*PI*$cycles)+$overtone+$low"
    local ring="(1-exp(-t/$onset))*exp(-t/$decay)"
    # A short tone fades out over its second half at most.
    local fade release_start
    fade=$(awk "BEGIN { print ($release < $dur / 2) ? $release : $dur / 2 }")
    release_start=$(awk "BEGIN { print $dur - $fade }")
    echo "aevalsrc='$ring*($wave)':s=$rate:d=$dur,lowpass=f=$lowpass,\
        afade=t=in:d=$attack:curve=$attack_curve,\
        afade=t=out:st=$release_start:d=$fade:curve=$release_curve[$label]"
}

# Writes clip $1, $2 seconds long, made of the parts that follow, played one after another and
# padded with silence. $3 is an ffmpeg filter applied to the whole clip ("anull" for none). A part
# is one of:
#   dot:FREQ                      a dot at FREQ Hz
#   gap:SECONDS                   silence
#   slide:FROM:TO:GLIDE:SECONDS[:CURVE]
#                                 a tone moving from FROM Hz onto TO Hz, arriving after GLIDE seconds
#                                 along CURVE (see tone)
clip() {
    local name=$1 total=$2 effect=$3
    shift 3
    local graph="" labels="" n=0 kind a b c d e
    for part in "$@"; do
        IFS=: read -r kind a b c d e <<<"$part"
        case $kind in
            dot) graph+="$(tone "$a" "$a" 1 "$dot_length" "p$n");" ;;
            gap) graph+="anullsrc=r=$rate:cl=mono,atrim=0:$a[p$n];" ;;
            slide) graph+="$(tone "$a" "$b" "$c" "$d" "p$n" "${e:-out}");" ;;
            *) echo "unknown part: $part" >&2 && return 1 ;;
        esac
        labels+="[p$n]"
        n=$((n + 1))
    done
    ffmpeg -nostdin -hide_banner -loglevel error -y -filter_complex "
        $graph
        ${labels}concat=n=$n:v=0:a=1,$effect,apad,atrim=0:$total,volume=$volume[out]" \
        -map "[out]" -ac 1 "$out/$name"
}

mkdir -p "$out"

# Rises and falls. Power uses two octaves; connections use one, so they sound smaller. Connections
# follow a dot, so they start at rest on its pitch and speed up rather than leaping off it.
power_on="slide:$a3:$a5:0.12:0.16"
power_off="slide:$a5:$a3:0.12:0.16"
connect="slide:$a4:$a5:0.1:0.14:in-out"
disconnect="slide:$a5:$e4:0.07:0.14:in-out"

# Slot 0, power on: one tone rising from A3 onto A5, arriving at 120 ms, 160 ms long.
clip 00-power-on.wav 0.25 anull "$power_on"
# Slot 1, power off: power on reversed, falling from A5 onto A3.
clip 01-power-off.wav 0.25 anull "$power_off"

# Connected: a dot per device, then a smaller rise than power on. Disconnected: the same, falling.
clip 03-bluetooth-connected.wav 0.3 anull "dot:$a4" "gap:$pause" "$connect"
clip 47-device-1-connected.wav 0.3 anull "dot:$a4" "gap:$pause" "$connect"
clip 49-device-2-connected.wav 0.4 anull "dot:$a4" "gap:$pause" "dot:$a4" "gap:$pause" "$connect"
clip 53-second-device-connected.wav 0.4 anull "dot:$a4" "gap:$pause" "dot:$a4" "gap:$pause" "$connect"
clip 04-bluetooth-disconnected.wav 0.3 anull "dot:$a5" "gap:$pause" "$disconnect"
clip 48-device-1-disconnected.wav 0.3 anull "dot:$a5" "gap:$pause" "$disconnect"
clip 50-device-2-disconnected.wav 0.4 anull "dot:$a5" "gap:$pause" "dot:$a5" "gap:$pause" "$disconnect"

# Slot 34, low battery: three short tones at E5.
low_battery=("dot:$e5" gap:0.05 "dot:$e5" gap:0.05 "dot:$e5")
clip 34-low-battery.wav 0.3 anull "${low_battery[@]}"
# Slot 5, recharge power off: the low battery warning, then power off.
clip 05-recharge-power-off.wav 0.6 anull "${low_battery[@]}" gap:0.1 "$power_off"

# Slot 11, connection lost: a scratchy fall over the same length as a disconnect.
clip 11-connection-lost.wav 0.3 "$scratchy" "slide:$a5:$a3:0.2:0.23"

# Slot 39, volume step: one short beep.
clip 39-unknown-volume-1.wav 0.1 anull "dot:$e5"

echo "generated clips in $out"
