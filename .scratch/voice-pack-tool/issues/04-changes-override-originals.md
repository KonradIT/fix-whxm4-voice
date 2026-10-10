# 04: Changes override originals

**What to build:** Files in `changes/` replace the **original** with the same name, matched on the
name without its extension, so `07-ambient-sound-off.wav` replaces `07-ambient-sound-off.mp3`. Any
format ffmpeg reads is accepted and converted to exactly the format of the stock clips (sample rate,
channels, bitrate, MP3 type), measured from the originals. An empty change file makes that slot
silent. A change matching no original is an error listing the closest original names. Slots without
a change keep their original.

**Blocked by:** 03 (Build an unchanged pack)

**Status:** ready-for-human

- [x] Matching ignores the extension; two changes for the same slot are an error
- [x] Converted clips match the measured stock format
- [x] A zero-byte change becomes a short silent clip in the stock format
- [x] An unmatched change fails the build and lists the closest original names
- [ ] The built pack plays the changed clips on the replaced slots (verifiable by unpacking `build/` and listening)

## Comments

- The stock clips differ in bitrate (64 or 48 kbit/s), so each change takes the format of the clip it replaces;
  see `docs/adr/0002-convert-changes-to-the-replaced-slots-format.md`. A silent clip is 0.1 s.
- Checked on the English pack: changes in WAV (44.1 kHz stereo), FLAC and OGG, plus an empty file, built and
  verified. Unpacked again, the replaced slots were 48 kHz mono CBR at their slot's bitrate, the silent slot peaked
  at -91 dB, and the kept slots were byte-identical to the originals. Nobody has listened to them yet.
- Left as `ready-for-human` until someone listens to the changed slots in an unpacked `build/pack.bin`.
- Changed on request: a change now matches by the slot number its name starts with, and the rest of the name is
  ignored, so `7-ambient-off-8-bit.wav` replaces `07-ambient-sound-off.mp3`. A name with no slot number in 00–53 is
  the "unmatched" error. Updated in `CONTEXT.md` and spec stories 21, 22 and 26.
