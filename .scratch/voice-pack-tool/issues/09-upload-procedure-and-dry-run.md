# 09: Upload procedure and --dry-run

**What to build:** The voice-pack update sequence from `docs/upload-protocol.md` over the device
link, with a range guard below it. The guard refuses any erase or write outside the FOTA partition
the headphones report (`0x1C00`), or beyond the pack rounded up to 4 KB, before it is sent.
`upload --dry-run` runs the procedure against the real headphones with a built pack, except that
erase, write, integrity check and commit are suppressed, and reports what it would have done.

**Blocked by:** 03 (Build an unchanged pack), 07 (Reverse-engineer the upload procedure), 08 (device-info)

**Status:** ready-for-agent

- [ ] The sequence sends only the steps the app sends, in the app's order: MDR language update mode
      on, RACE FOTA start and queries, erase status and compare, erase, page program, integrity
      check, commit, update mode off
- [ ] The range guard rejects any out-of-range erase or write, and cannot be bypassed by the procedure code
- [ ] Only packs that passed `build`'s full verification are accepted
- [ ] Dry-run sends no erase, write, integrity check or commit, switches update mode off again, and
      prints the reported FOTA partition, the planned address ranges and the page count
- [ ] Tests against a fake device link (local, fast): the command order matches the documented
      sequence; an out-of-range request is refused before sending; dry-run sends no erase or write;
      success is reported only after the fake device's checksum check passes, and a failed check is
      reported as failure
- [ ] `upload --dry-run` verified by hand against the user's headphones

## Comments

- Changed after ticket 07: the guard is about the reported FOTA partition, not the voice partition.
  The app never writes the voice partition. Dry-run has to switch language update mode on, because
  the headphones refuse `0x1C00` otherwise. That changes a mode, not flash, and dry-run switches it
  off again.
