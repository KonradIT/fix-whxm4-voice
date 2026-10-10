#!/usr/bin/env bash
# Extracts short battery clips from the English originals with ffmpeg, to be used as changes: of
# "battery, about seventy percent" only "seventy" is kept, of "battery, fully charged" only "full".
# Only the cut points live in this script, so no audio needs to be committed.
#
# Usage: battery.sh [changes-dir] [original-dir]   (default: ./changes ./original)
#
# `fix-whxm4-voice generate` runs this script when asked to; it also runs on its own. The times
# below fit the English stock voice pack only.
#
# A word cut out of a sentence ends early: its last sound runs into the next word ("seventy p...").
# So the steady part of its last sound is stretched with atempo, which keeps the pitch, and the cut
# ends before the next word starts. Every clip fades in and out so it starts and ends without a click.
set -euo pipefail

out=${1:-changes}
original=${2:-original}

# Sample rate of the originals and the clips.
rate=48000
# Defaults of a stretch, in seconds: the crossfade into the stretched part, and how much audio after
# the cut atempo gets. atempo drops about 20-30 ms at the end of its output, so it is fed more and
# its output is trimmed to the exact stretched length.
xfade=0.005
pad=0.04
# Crossfade around an inserted copy of a noise, in seconds.
dup_xfade=0.003

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Writes clip $1.wav, cut from original $1.mp3 from $2 to $3 seconds. The parts that follow shape it:
#   fade:IN:OUT                  fade in and out over IN and OUT seconds (required)
#   stretch:FROM:TEMPO[:XFADE[:PAD]]
#                                play FROM..end at TEMPO (< 1 is longer; below 0.5 atempo is chained),
#                                crossfaded in over XFADE seconds. PAD 0 feeds atempo no extra audio.
#   dup:FROM:TO[:N]              insert N (default 1) extra copies of FROM..TO, which lies before the
#                                stretch: lengthens a noise (th, s, f), too short for atempo to stretch
clip() {
    local name=$1 start=$2 end=$3
    shift 3
    local fade_in="" fade_out="" split="" tempo xf=$xfade pd=$pad dup_from="" dup_to="" dup_n=1 kind a b c d
    for part in "$@"; do
        IFS=: read -r kind a b c d <<<"$part"
        case $kind in
            fade) fade_in=$a fade_out=$b ;;
            stretch) split=$a tempo=$b xf=${c:-$xfade} pd=${d:-$pad} ;;
            dup) dup_from=$a dup_to=$b dup_n=${c:-1} ;;
            *) echo "unknown part: $part" >&2 && return 1 ;;
        esac
    done
    local in=$original/$name.mp3 raw=$work/$name.wav
    if [[ ! -f $in ]]; then
        echo "missing original: $in" >&2 && return 1
    fi
    if [[ -z $split ]]; then
        ffmpeg -nostdin -hide_banner -loglevel error -y -i "$in" -af "atrim=$start:$end" \
            -ar $rate -ac 1 -c:a pcm_s16le "$raw"
    else
        local head="[a]atrim=$start:$split,asetpts=PTS-STARTPTS[head]"
        if [[ -n $dup_from ]]; then
            # [start..TO] x N * [FROM-xf..TO] x [TO-xf..split]; the last join overlaps identical audio.
            local d0 d1 i
            d0=$(awk "BEGIN { print $dup_from - $dup_xfade }")
            d1=$(awk "BEGIN { print $dup_to - $dup_xfade }")
            head="[a]asplit=$((dup_n + 2))$(for ((i = 0; i < dup_n + 2; i++)); do printf '[h%d]' $i; done);
                [h0]atrim=$start:$dup_to,asetpts=PTS-STARTPTS[p0]"
            for ((i = 1; i <= dup_n; i++)); do
                head+=";[h$i]atrim=$d0:$dup_to,asetpts=PTS-STARTPTS[c$i];
                    [p$((i - 1))][c$i]acrossfade=d=$dup_xfade:c1=tri:c2=tri[p$i]"
            done
            head+=";[h$((dup_n + 1))]atrim=$d1:$split,asetpts=PTS-STARTPTS[r];
                [p$dup_n][r]acrossfade=d=$dup_xfade:c1=tri:c2=tri[head]"
        fi
        # atempo takes 0.5 at least, so a slower tempo is a chain of 0.5s and the rest.
        local chain trim="" tail_end=$end
        chain=$(awk -v t="$tempo" 'BEGIN { c = ""; while (t < 0.5) { c = c "atempo=0.5,"; t /= 0.5 }
            printf "%satempo=%s", c, t }')
        if [[ $pd != 0 ]]; then
            trim=",atrim=end=$(awk "BEGIN { print ($end - $split + $xf) / $tempo }")"
            tail_end=$(awk "BEGIN { print $end + $pd }")
        fi
        # The stretched part starts xf early, so the crossfade overlaps real audio.
        ffmpeg -nostdin -hide_banner -loglevel error -y -i "$in" -filter_complex "
            [0:a]asplit[a][b];
            $head;
            [b]atrim=$(awk "BEGIN { print $split - $xf }"):$tail_end,asetpts=PTS-STARTPTS,$chain$trim[tail];
            [head][tail]acrossfade=d=$xf:c1=tri:c2=tri[out]" \
            -map "[out]" -ar $rate -ac 1 -c:a pcm_s16le "$raw"
    fi
    # Fades in a pass of their own, at exact sample positions.
    local samples fade_samples
    samples=$(($(ffmpeg -nostdin -loglevel error -i "$raw" -f s16le - | wc -c) / 2))
    fade_samples=$(awk "BEGIN { printf \"%d\", $fade_out * $rate }")
    ffmpeg -nostdin -hide_banner -loglevel error -y -i "$raw" -af "afade=t=in:d=$fade_in,\
afade=t=out:start_sample=$((samples - fade_samples)):nb_samples=$fade_samples:curve=qsin" \
        -c:a pcm_s16le "$out/$name.wav"
}

mkdir -p "$out"

# Times are seconds in the original. Each comment maps the sounds around the cut.

# "battery, fully charged": f 0.89, u 1.03, l ~1.13, y ~1.22, ch ~1.27. The end of u into l is
# stretched, so the word ends on a long l rather than "fullul" or "fully".
clip 15-battery-full 0.88 1.18 stretch:1.08:0.5:0.02:0 fade:0.015:0.08

# "battery, about N percent": the y (or n) ends where the lips close for the p of "percent", which
# alone sounds cut off. So the cut ends before the closure and the steady part of the y is stretched
# by 85 ms. atempo copies its first ~25 ms unchanged, so the stretch starts in the steady y, never at
# the t or n before it.

# about 0.92, gap 1.19-1.24, se 1.25, n 1.54-1.60, t spoken as d ~1.605, y 1.61-1.66, p closure 1.68.
clip 16-battery-70 1.23 1.665 stretch:1.62:0.3462 fade:0.01:0.05
# t gap 1.17-1.24, n 1.25, ai 1.31-1.44, n 1.45, t flap 1.49, y 1.50-1.57, p closure 1.58.
clip 28-battery-90 1.235 1.565 stretch:1.525:0.32 fade:0.01:0.05
# t of about released 1.23-1.28, ei 1.29-1.39, t flap 1.40, y 1.42-1.48, p closure 1.49.
clip 29-battery-80 1.285 1.485 stretch:1.44:0.3462 fade:0.015:0.05
# t closure 1.18-1.23, s 1.24-1.35, i, k, s 1.47-1.52, t 1.53-1.58, y 1.59-1.66, p closure 1.67.
clip 30-battery-60 1.235 1.655 stretch:1.605:0.3704 fade:0.01:0.05
# t closure 1.18-1.22, f 1.23-1.30, i, f 1.37-1.40, t 1.41-1.46, y 1.47-1.52, p closure 1.53.
clip 17-battery-50 1.222 1.525 stretch:1.48:0.3462 fade:0.01:0.05
# t of about released 1.235-1.247 (starting earlier gives "tforty"), f -1.35, or 1.36-1.46,
# t flap 1.47-1.49, y 1.50-1.555, p closure 1.56.
clip 31-battery-40 1.248 1.555 stretch:1.51:0.3462 fade:0.015:0.05
# t closure 1.15-1.28, th 1.285-1.30, ir 1.30-1.40, t flap 1.40, y 1.42-1.48, p closure 1.49. The th
# is ~15 ms, which sounds like a t: one extra copy of its noise makes ~25 ms.
clip 32-battery-30 1.282 1.48 dup:1.288:1.298 stretch:1.435:0.3462 fade:0.005:0.05
# One t burst 1.28-1.35, the t of twenty (kept, so a short fade-in), we 1.37-1.43, n 1.44-1.455,
# y 1.46-1.505 (no second t), darker from 1.505 as the lips close, p closure 1.52.
clip 18-battery-20 1.275 1.505 stretch:1.46:0.3462 fade:0.003:0.05
# One t burst 1.26-1.31, the t of ten, e 1.32-1.38, n 1.39-1.47, p closure 1.48. The n is stretched by
# 40 ms only, with a short fade-out so it keeps its full level.
clip 33-battery-10 1.255 1.47 stretch:1.41:0.6 fade:0.003:0.02

echo "extracted battery clips in $out"
