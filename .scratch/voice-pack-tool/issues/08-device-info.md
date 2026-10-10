# 08: device-info (read-only)

**What to build:** `device-info` connects to the headphones over Bluetooth LE (BlueZ through `bluer`) and prints what they report: model, firmware version, current language, and update state. It only sends read-only commands documented in ticket 07, and logs every command and reply. This is the safe first contact with the real device.

**Blocked by:** 01 (Workspace and flake skeleton), 07 (Reverse-engineer the upload procedure)

**Status:** ready-for-agent

- [ ] A device link with a "send command, await reply" interface that can be faked in tests
- [ ] Only commands documented as read-only in ticket 07 are allowed through this command
- [ ] Clear errors when BlueZ isn't running, no headphones are found, or the update channel isn't reachable
- [ ] With `--verbose` (or similar), every raw command and reply is logged
- [ ] Verified by hand against the user's WH-1000XM4
