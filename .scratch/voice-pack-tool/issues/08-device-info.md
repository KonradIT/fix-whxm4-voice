# 08: device-info (read-only)

**What to build:** `device-info` connects to the headphones over Bluetooth Classic RFCOMM (BlueZ;
channels found by SDP UUID, see `docs/upload-protocol.md`) and prints what they report: current
language, supported languages, whether language update mode is on, and anything else the read-only
MDR queries return. It only sends commands that `docs/upload-protocol.md` lists as read-only, and
logs every command and reply. This is the safe first contact with the real device.

**Blocked by:** 01 (Workspace and flake skeleton), 07 (Reverse-engineer the upload procedure)

**Status:** resolved

- [x] A device link with a "send command, await reply" interface that can be faked in tests, for
      both the MDR channel (`Serial HPC`) and the RACE channel (`Airoha_APP`)
- [x] Only commands documented as read-only in `docs/upload-protocol.md` are allowed through this command
- [x] Clear errors when BlueZ isn't running, no headphones are found, or a channel isn't reachable
- [x] With `--verbose` (or similar), every raw command and reply is logged
- [x] Verified by hand against the user's WH-1000XM4

## Comments

- Changed after ticket 07: RFCOMM instead of BLE (`bluer`). The RACE FOTA queries (`0x1C00`, `0x1C04`)
  are refused unless language update mode is on, and enabling it is a set command. So device-info
  shows MDR data only, and leaves the FOTA partition to `upload --dry-run`.
- Done. Transport: `bluer`, with BlueZ doing the SDP lookup through a client profile per UUID
  (ADR 0004). `device-info --verbose` (`-v`) logs every frame sent (`>`) and received (`<`) as hex.
  The read-only allowlist is the closed set of queries in `src/device_info.rs`; nothing else can be
  expressed there. With the user's go-ahead, two queries not in the APK notes were added and documented
  in `docs/upload-protocol.md`: model name (`04 01`) and firmware version (`04 02`). The only RACE
  command is the `0x0A00` chip-name read, which also proves the RACE channel opens.
- Hand run on 2026-10-10 against the user's WH-1000XM4: model `WH-1000XM4`,
  firmware `3.0.1`, protocol info `70 00`, empty chip name, English active, update mode off,
  category `HP002`, 15 languages with service IDs (`VGIDLPB0401`…`VGIDLPB04F0`, including Russian
  `040A`, Japanese `040B`, Brazilian Portuguese `040D`, Korean `040F` and Chinese `04F0`, which have
  no stock pack this tool downloads). Abridged log:

  ```
  MDR > 3e 0c 00 00 00 00 02 00 00 0e 3c
  MDR < 3e 01 01 00 00 00 00 02 3c 3e 0c 01 00 00 00 04 01 00 70 00 82 3c
  MDR > 3e 01 00 00 00 00 00 01 3c
  MDR > 3e 0c 01 00 00 00 02 04 01 14 3c
  MDR < 3e 01 00 00 00 00 00 01 3c
  MDR < 3e 0c 00 00 00 00 0d 05 01 0a 57 48 2d 31 30 30 30 58 4d 34 8f 3c
  MDR > 3e 01 01 00 00 00 00 02 3c
  …
  RACE > 05 5a 06 00 00 0a 02 10 e8 03
  RACE < 05 5b 05 00 00 0a 00 00 00
  ```
- Not exercised by hand: the error paths for BlueZ not running and no headphones connected, which
  would have meant stopping the service or disconnecting the headphones.
