# 05: Size limit report

**What to build:** If the clips together are larger than the stock clips of the base language,
`build` fails with a report: each clip's size, the total, the limit and where it comes from. Every clip
is already at the bitrate of its slot, so the report asks for changes to be trimmed, or originals
copied into `changes/` and trimmed there. The user doesn't need to know ffmpeg.

**Blocked by:** 04 (Changes override originals)

**Status:** resolved

- [x] The limit is the stock clip total of the base language and lives in one place, so it can later
      be replaced by a hardcoded constant
- [x] The report lists every clip with its size (changed ones marked), the total, and the limit
- [x] The report says every clip is already at the bitrate of its slot, and asks for changes to be trimmed,
      or originals copied into `changes/` and trimmed there, then built again
- [x] A pack within the limit builds normally with no report

## Comments

- Changed on request: the report suggests no ffmpeg commands. They would have to work on files outside
  `changes/` (a backup in `build/`), and since every change is already converted to its slot's bitrate,
  trimming the audio is the only fix. Spec story 31, the "Size check" module and Out of Scope are updated.
- The limit is `size::limit`, the total of the stock clips in `.stock/pack.bin`. The check runs on the
  converted clips, after conversion and before packing, so the sizes in the report are what goes into the pack.
- A single change may be larger than the clip it replaces, as long as the total fits.
- Checked on a real workspace (the generated tones plus a 3‑minute and a 1‑minute change): `build` failed with
  the report and wrote no pack.
