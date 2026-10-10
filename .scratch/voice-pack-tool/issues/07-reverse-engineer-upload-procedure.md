# 07: Reverse-engineer the upload procedure

**What to build:** A protocol notes document (under `docs/`) describing exactly how the Sony app
uploads a voice pack to the WH-1000XM4 over Bluetooth LE. It is based on decompiling the Android
Sound Connect APK with jadx (run through `nix shell nixpkgs#jadx`; the Airoha FOTA SDK classes, e.g.
`FotaControl2811`, `AirohaRaceOtaMgr`) and cross-checked against race-toolkit's preliminary FOTA
code. The APK is supplied by the user and only decompiled, never executed. Nothing is sent to the
headphones in this ticket.

**Blocked by:** None (can start immediately). Needs the Sound Connect APK from the user.

**Status:** resolved

- [x] GATT service and characteristic UUIDs and RACE framing confirmed
- [x] Every command of the voice-pack update sequence documented with ID, payload layout and
      expected reply: state query, start, erase, page write, integrity check (0x1C01), commit/reboot
- [x] Chunking and page-skip rules confirmed (4 KB chunks, all-0xFF 256-byte pages skipped)
- [ ] The voice partition's exact address range documented, with the evidence
- [x] Whether writes to the voice partition must be encrypted is answered
- [x] How the install language is conveyed to the headphones, and whether a "custom" code is
      possible and safe, answered or listed as unknown with evidence
- [x] Any clue about the unexplained check that ignores a pack installed under the already-active language is recorded
- [x] race-toolkit's license recorded, and whether its code may be reused
- [x] Findings that contradict ADR 0001 or `CONTEXT.md` are flagged explicitly

## Comments

- Notes are in `docs/upload-protocol.md`. The user agreed to let the agent download the APK: Sound Connect 12.4.2
  from APKPure, signed by Sony Corporation. It was only decompiled. Nothing was sent to the headphones: only
  BlueZ's cached info and an SDP browse were read.
- The premise changed in several ways:
  - The WH-1000XM4 (MT2811) is updated over Classic RFCOMM (`Airoha_APP`, `8901dfa8-…`), not BLE. The SDK
    refuses BLE FOTA on this chip.
  - The app writes the pack file verbatim into the FOTA partition. It never writes the voice partition.
  - Erase works in 4 KB blocks, but program works in 256-byte pages.
  - The install language is the first u32 of the unpacked image (`image_version` in `src/pack.rs`); it is not
    sent during the upload.
  - English packs point at a different flash bank (`0x80000`) from all other languages (`0x380000`).
- These contradict ADR 0001 and affect tickets 08–11. The doc's last two sections list them. They need the
  user's decisions before 08 starts.
- The GATT box is ticked for what the app holds (UUIDs in `AirohaFotaAdapterSony`). Whether the headphones
  expose them over LE was not checked, and this chip's FOTA does not use them.
- The voice-partition-range box stays open. The range has two conflicting readings: `0x0C380000` from the
  pack header, and `0x0C510000` (6 MB) from sony-xm4-voice-mod's notes. The app never writes there, so the
  upload does not depend on it. Settling it needs read-only RACE queries to the headphones (`0x1C00`, a
  partition-table read), which need the user's go-ahead.
- The user confirmed the direction: no app, upload from the PC, German install language by default
  with an option, English refused, read-only device access allowed, no writes yet. ADR 0001 was
  rewritten, ADR 0003 added, and `build` now writes the install language (`--install-lang`).
- A read-only probe of the headphones confirmed MDR on `Serial HPC` and the language list and codes.
  It also showed that RACE flash reads are blocked, and that FOTA queries are refused outside language
  update mode. So the voice-partition-range box cannot be settled by reading. The upload does not
  need it, so the ticket is resolved with that box open.
