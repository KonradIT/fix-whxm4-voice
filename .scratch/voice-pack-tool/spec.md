# Spec: fix-whxm4-voice

Status: ready-for-agent

## Problem Statement

The Sony WH-1000XM4 headphones speak voice prompts ("Power on", "Noise canceling", "Battery about
50%", "Bluetooth connected", …) for nearly every event. The owner wants to replace them with their
own clips, such as short tones or silence. Sony provides no way to do this. The only known working
method needs a man-in-the-middle proxy, a CA certificate trusted on the phone, and the Sony phone
app, spread across several hand-run Python scripts. The owner has an iPhone and no Mac or Android
phone, and wants the whole job done from a Linux PC (NixOS) with one tool.

## Solution

A Rust command-line tool, `fix-whxm4-voice`, run from a dedicated **workspace** directory:

1. It downloads the stock **voice pack** for a **base language** (English by default) and saves
   every **clip** as a playable MP3 in `original/`, named by **slot** number and **prompt**.
2. The user drops **changes** into `changes/`, each named starting with the slot number of the
   original it replaces. Any audio format works. An empty file means silence.
3. The tool builds a modified voice pack in `build/`, checks it fits, and verifies it by unpacking it again.
4. The tool **uploads** the pack from the PC straight to the headphones over Bluetooth LE, by
   replaying the update procedure the Sony app uses. The phone app is not involved. Ideally the pack
   is installed under a dedicated "custom" **install language**.

Running the tool with no subcommand walks through the whole workflow. Each step is also its own subcommand.

## User Stories

### Workspace

1. As a user, I want the tool to work in the current directory by default, so that I don't need to
   pass paths for normal use.
2. As a user, I want to choose the workspace with `-d <dir>`, so that I can keep it anywhere.
3. As a user, I want `-d` to create the directory if it doesn't exist, so that I don't have to create it first.
4. As a user, I want to be warned when the tool is about to set up `original/`, `changes/` and
   `build/` in a directory that already holds other things, so that I don't clutter an unrelated
   folder by accident.
5. As a user, I want that warning to explain that the tool should be run in its own directory and to
   ask whether to continue anyway, so that I can choose.
6. As a user, I want `-f` to skip that question, so that I can script the tool.
7. As a user, I want the warning to appear only when the workspace folders are first created, so
   that later runs never ask again.
8. As a user, I want hidden files (such as `.envrc`, `.direnv/`, `.git/`) not to count as "other
   things", so that a workspace managed with direnv or git doesn't trigger the warning.

### Default workflow

9. As a user, I want running `fix-whxm4-voice` with no subcommand to set up the workspace and
   download the originals if they are missing, so that one command gets me started.
10. As a user, I want the default command to finish by telling me where to put changes and which
    command uploads them, so that I know the next step.
11. As a user, I want the default command to offer to upload right away when changes already exist,
    so that re-running it after editing finishes the job.
12. As a user, I want the upload offer to default to "no", so that pressing Enter by accident never
    writes to my headphones.

### Download

13. As a user, I want `download` to fetch the stock voice pack from Sony's public server, so that I
    have the originals to listen to and to use as the naming reference.
14. As a user, I want the base language to default to English, so that I don't need to pick one.
15. As a user, I want `--lang <name>` to choose any of the ten base languages (english, french,
    german, spanish, italian, portuguese, dutch, swedish, finnish, turkish), so that I can start
    from another language's clips.
16. As a user, I want every clip saved as an MP3 that mpv plays as-is, so that I can listen to each prompt.
17. As a user, I want originals named `NN-prompt-name.mp3` (e.g. `07-ambient-sound-off.mp3`), so
    that I can find a prompt without a lookup table and the files sort by slot.
18. As a user, I want slots of unknown purpose named `NN-unknown.mp3`, so that every slot is still
    present and addressable.
19. As a user, I want the download checked against Sony's manifest (size and checksum), so that a
    broken download is caught.
20. As a user, I want the default command to skip the download when the originals are already
    present, so that re-runs are fast.

### Changes

21. As a user, I want a change to override the original with the same slot number, so that I only add files
    for the prompts I want to replace.
22. As a user, I want a change to match its original by the slot number its name starts with, ignoring the rest, so that
    `7-ambient-off-8-bit.wav` replaces `07-ambient-sound-off.mp3`.
23. As a user, I want changes in any format ffmpeg reads to be accepted, so that I don't have to
    convert anything myself.
24. As a user, I want changes converted to exactly the format of the stock clips (sample rate,
    channels, bitrate, MP3 type, as measured from the originals), so that the headphones play them
    like the stock clips.
25. As a user, I want an empty change file to make that slot silent, so that
    `touch changes/00-power-on.mp3` mutes a prompt.
26. As a user, I want a change whose name starts with no slot number to be an error listing the closest
    original names, so that a typo is never silently ignored.
27. As a user, I want every slot without a change to keep its original clip, so that I can replace prompts gradually.

### Build

28. As a user, I want `build` to produce the modified voice pack in `build/` without needing the
    headphones, so that I can prepare and check everything offline.
29. As a user, I want `build` to refuse a pack whose clips together are larger than the stock clips
    of the base language, so that the pack stays within what the headphones are known to accept.
30. As a user, I want a failed size check to list each clip's size, the total, and the limit, so
    that I can see what to shorten.
31. As a user, I want a failed size check to tell me that every clip is already at the bitrate of its
    slot and that I should trim changes, or copy originals into `changes/` and trim them there, so
    that I know shorter audio is the only fix.
32. As a user, I want every built pack verified by unpacking it again (decrypt, decompress, check
    every checksum, read the slot table, read every clip), so that a broken pack never reaches the
    headphones.
33. As a user, I want the last built pack kept in `build/`, so that a failed upload can be investigated.

### Device and upload

34. As a user, I want `device-info` to show what the headphones report about themselves (model,
    firmware version, current language, update state) without changing anything, so that I can check
    the PC is talking to them safely.
35. As a user, I want `upload` to rebuild first when changes or originals are newer than the last
    build, so that I never upload a stale pack.
36. As a user, I want `upload` to ask me to turn off Bluetooth on my phone before it starts, and to
    check the PC is the only connection it can see, so that the phone app can't interfere.
37. As a user, I want `upload` to show its plan (pack size, address range, install language) and
    require a typed confirmation before writing, so that I always know what is about to happen.
38. As a user, I want `upload --dry-run` to go through every step except erasing and writing, so
    that I can test the connection and procedure without risk.
39. As a user, I want the tool to refuse any erase or write outside the voice partition, so that a
    bug in the tool can never overwrite firmware.
40. As a user, I want the upload to send only the steps the Sony app itself sends, in the same
    order, so that the headphones receive a procedure they were designed for.
41. As a user, I want progress shown during the upload, so that I know it hasn't stalled.
42. As a user, I want the upload reported as done only when the headphones confirm their checksum
    check, so that "done" means the pack really arrived intact.
43. As a user, I want a clear message if the headphones reject the pack (for example at the 99%
    checksum check), so that I know it failed and that the old voice pack can be restored by picking
    a stock language in the Sony app.
44. As a user, I want the pack installed under a dedicated "custom" install language that the app
    later shows as custom, so that I can tell my pack apart from Sony's.
45. As a user, if a custom install language isn't possible, I want the pack installed under a real
    language other than the active one, so that the headphones actually switch to it.

### Packaging

46. As a user, I want `nix run .#` to run the tool with ffmpeg always available, so that I don't install anything else.
47. As a developer, I want a devShell with the Rust toolchain and ffmpeg, so that I can build and
    run the tool. Reverse-engineering tools such as jadx come from `nix shell` when needed.

## Implementation Decisions

### Delivery stages

- **Stage 1**: workspace handling, `download`, `build`, the default workflow, the flake. No device needed.
- **Stage 2**: reverse engineering of the upload procedure, then `device-info`, then `upload`.
- Stage 2's sources are a decompile of the Android Sound Connect APK (supplied by the user, never
  executed), cross-checked against race-toolkit's preliminary firmware update code. No phone capture
  is possible (iPhone, no Mac).
- Fallback, only if the reverse engineering fails: the proven route through the app and a proxy, as
  `upload --via-app`. Not part of this spec. See ADR 0001.

### Modules

- **Pack codec** (deep, pure, no I/O). The interface is roughly "unpack: pack bytes → slot clips
  and metadata" and "pack: slot clips plus metadata from the stock pack → pack bytes". It owns all
  knowledge of Sony's format:
  - the 32-byte SHA-256 at offset 0 covers everything from 0x100 to the end, and is recomputed last;
  - the tag-length metadata block at 0x100: tags 0x0011 (compression type, body offset, body size),
    0x0012 (version, header size, unpacked size, constant), 0x0013 (version string), 0x0014
    (algorithm, SHA-256 of the unpacked image);
  - the body at 0x1000: AES-128-CBC with a fixed key and IV, padded with 0xFF to 16 bytes, over a
    13-byte LZMA1 header plus raw LZMA1 stream (lc=3, lp=0, pb=2, dictionary 16 KiB);
  - the unpacked image: u32 version, u32 slot count (54), a table of 54 × (u32 size, u32 absolute
    offset) based on the voice partition (base = first offset − table end), then the clips back to
    back, zero-padded to exactly 1 MiB.
  
  Keys are built in, as published constants kept together in one place.
- **Manifest codec** (pure). It reads Sony's manifest: header lines `eaid`, `daid`, `digest`, a
  blank line, then an AES-128-ECB body (fixed key, zero padding) holding a `Distribution` element
  with `MAC` (SHA-1 of the pack) and `Size`. It is used to verify downloads, and in stage 2 wherever
  the upload needs manifest data.
- **Slot catalogue** (pure data). Maps the 54 slots to prompt names used in file names. Slots of
  unknown purpose are `unknown`.
- **Change resolution** (pure over a directory listing). Takes the original names and the changes
  directory. It returns, per slot, one of: keep original, replace with this file, or silence. It
  reports unmatched changes with the closest original names.
- **Audio conversion** (shells out to ffmpeg). Measures the stock clip format and converts changes
  to it. Silence becomes a short silent clip in the same format.
- **Size check** (pure). Compares the total clip size to the limit. The limit is currently the stock
  clip total of the base language, kept in one place so it can later be replaced by a hardcoded
  constant. Produces the per-file report and asks for the changes to be trimmed.
- **Pack source** (HTTP). Fetches the manifest and pack for a language from Sony's public update
  server (`info.update.sony.net/HP002/VGIDLPB04XX/…`, XX = language number 01–10).
- **Device link** (Bluetooth LE through BlueZ, using the `bluer` crate). It connects to the update
  channel. The GATT service and characteristic UUIDs are taken from existing research and confirmed
  during stage 2. The interface is "send command, await reply" so it can be faked.
- **Upload procedure** (protocol logic over the device link). It replays the app's update sequence
  (query state, start, erase, write in 4 KB chunks skipping all-0xFF 256-byte pages, integrity
  check, commit). A range guard sits below it and rejects any erase or write outside the voice
  partition before it is sent. Dry-run mode suppresses erase and write.
- **Workspace** (filesystem). Resolves the directory (`-d`, default the current directory), creates
  it if missing, and creates `original/`, `changes/`, `build/`. It runs the "not dedicated" check
  only while creating those, ignores hidden entries, and honours `-f`.
- **CLI**: subcommands `download [--lang]`, `build`, `upload [--dry-run]`, `device-info`. The
  default command runs the workflow. Global flags: `-d`, `-f`.

### Behaviour decisions

- `build` output in `build/` includes the pack and is kept after uploading.
- `upload` rebuilds when anything in `original/` or `changes/` is newer than the last build.
- The install language is decided in stage 2, once it is known how the app tells the headphones the
  language. Order of preference:
  1. a dedicated "custom" code, but only if it is shown safe on the device side;
  2. a real language other than the active one, read from the headphones if possible, otherwise asked for.
- The headphones ignore a pack installed under the language that is already active. The cause is
  unknown and the check sits inside the headphones.

### Packaging

- The flake provides a package (used through `nix run .#`), wrapped so ffmpeg is on its PATH, and a
  devShell (Rust toolchain, ffmpeg). Reverse-engineering tools (jadx) are not in the devShell; use
  `nix shell nixpkgs#jadx`. BlueZ must be running on the host.

## Testing Decisions

- Tests are local and fast. They never use the network, Sony's files, ffmpeg or real Bluetooth. Not
  every feature gets a test; tests focus on where a bug would be costly.
- Tests check behaviour at a module's interface, never its internals.
- **Pack codec**: tests use small packs generated inside the test (arbitrary byte blobs as clips;
  the codec doesn't care what's inside a clip). Packing and then unpacking reproduces the clips, and
  every checksum is valid. Tampering with any checksummed region is detected. An image over the
  limit is rejected. The padding and size rules hold.
- **Upload procedure**: tested against a fake device link that implements the reply sequence learned
  in stage 2 and records every command. Erases and writes stay within the voice partition, and an
  out-of-range request is refused before anything is sent. Dry-run sends no erase or write. Success
  is reported only after the fake device's checksum check passes, and failure when it fails.
- Real stock packs are covered at runtime, not in tests: `build` always verifies its output by unpacking it again.
- Prior art: none. This is a new codebase.

## Out of Scope

- The upload route through the app and a proxy (fallback only, see ADR 0001).
- Any write outside the voice partition, any firmware update, and any device or model other than the WH-1000XM4.
- Android or iOS app patching, and certificate installation.
- Automatically shrinking clips that don't fit, and suggesting commands for it. The user trims the changes.
- A hardcoded size limit. That comes later; for now the limit is the base language's stock total.
- End-to-end tests, and tests that download files or need hardware.

## Further Notes

- Domain terms follow `CONTEXT.md`. The upload approach and its safety limits are in
  `docs/adr/0001-direct-bluetooth-upload.md`.
- `info.md` contains format notes and links to existing implementations. It is reference material, not a spec.
- Reference implementations: SantiMoncada/sony-xm4-voice-mod (no license; read only for
  understanding, don't copy code) and HelgeSverre/sony-vp-extract (MIT). race-toolkit's license
  still needs checking before anything is borrowed from it.
- Open questions for stage 2:
  - the exact erase, write, start, commit and reboot commands and their framing;
  - the reply sequence;
  - whether the voice partition needs encrypted writes;
  - the voice partition's exact range;
  - how the install language is conveyed, and whether "custom" is possible;
  - the second, unexplained check that ignores a pack installed under the already-active language.
- Recovery if a voice-pack upload goes wrong while the firmware is intact: pick a stock language in
  the Sony app, which rewrites the voice partition.
