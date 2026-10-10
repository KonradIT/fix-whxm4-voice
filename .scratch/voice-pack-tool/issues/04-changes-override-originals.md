# 04: Changes override originals

**What to build:** Files in `changes/` replace the **original** with the same name, matched on the name without its extension, so `07-ambient-sound-off.wav` replaces `07-ambient-sound-off.mp3`. Any format ffmpeg reads is accepted and converted to exactly the format of the stock clips (sample rate, channels, bitrate, MP3 type), measured from the originals. An empty change file makes that slot silent. A change matching no original is an error listing the closest original names. Slots without a change keep their original.

**Blocked by:** 03 (Build an unchanged pack)

**Status:** ready-for-agent

- [ ] Matching ignores the extension; two changes for the same slot are an error
- [ ] Converted clips match the measured stock format
- [ ] A zero-byte change becomes a short silent clip in the stock format
- [ ] An unmatched change fails the build and lists the closest original names
- [ ] The built pack plays the changed clips on the replaced slots (verifiable by unpacking `build/` and listening)
