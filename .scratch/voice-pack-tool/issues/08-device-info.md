# 08: device-info (read-only)

**What to build:** `device-info` connects to the headphones over Bluetooth Classic RFCOMM (BlueZ;
channels found by SDP UUID, see `docs/upload-protocol.md`) and prints what they report: current
language, supported languages, whether language update mode is on, and anything else the read-only
MDR queries return. It only sends commands that `docs/upload-protocol.md` lists as read-only, and
logs every command and reply. This is the safe first contact with the real device.

**Blocked by:** 01 (Workspace and flake skeleton), 07 (Reverse-engineer the upload procedure)

**Status:** ready-for-agent

- [ ] A device link with a "send command, await reply" interface that can be faked in tests, for
      both the MDR channel (`Serial HPC`) and the RACE channel (`Airoha_APP`)
- [ ] Only commands documented as read-only in `docs/upload-protocol.md` are allowed through this command
- [ ] Clear errors when BlueZ isn't running, no headphones are found, or a channel isn't reachable
- [ ] With `--verbose` (or similar), every raw command and reply is logged
- [ ] Verified by hand against the user's WH-1000XM4

## Comments

- Changed after ticket 07: RFCOMM instead of BLE (`bluer`). The RACE FOTA queries (`0x1C00`, `0x1C04`)
  are refused unless language update mode is on, and enabling it is a set command. So device-info
  shows MDR data only, and leaves the FOTA partition to `upload --dry-run`.
