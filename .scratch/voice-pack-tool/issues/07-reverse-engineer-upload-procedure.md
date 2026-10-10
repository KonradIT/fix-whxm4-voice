# 07: Reverse-engineer the upload procedure

**What to build:** A protocol notes document (under `docs/`) describing exactly how the Sony app uploads a voice pack to the WH-1000XM4 over Bluetooth LE. It is based on decompiling the Android Sound Connect APK with jadx (the Airoha FOTA SDK classes, e.g. `FotaControl2811`, `AirohaRaceOtaMgr`) and cross-checked against race-toolkit's preliminary FOTA code. The APK is supplied by the user and only decompiled, never executed. Nothing is sent to the headphones in this ticket.

**Blocked by:** None (can start immediately). Needs the Sound Connect APK from the user.

**Status:** ready-for-agent

- [ ] GATT service and characteristic UUIDs and RACE framing confirmed
- [ ] Every command of the voice-pack update sequence documented with ID, payload layout and expected reply: state query, start, erase, page write, integrity check (0x1C01), commit/reboot
- [ ] Chunking and page-skip rules confirmed (4 KB chunks, all-0xFF 256-byte pages skipped)
- [ ] The voice partition's exact address range documented, with the evidence
- [ ] Whether writes to the voice partition must be encrypted is answered
- [ ] How the install language is conveyed to the headphones, and whether a "custom" code is possible and safe, answered or listed as unknown with evidence
- [ ] Any clue about the unexplained check that ignores a pack installed under the already-active language is recorded
- [ ] race-toolkit's license recorded, and whether its code may be reused
- [ ] Findings that contradict ADR 0001 or `CONTEXT.md` are flagged explicitly
