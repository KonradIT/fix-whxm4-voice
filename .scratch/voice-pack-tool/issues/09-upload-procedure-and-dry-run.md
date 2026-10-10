# 09: Upload procedure and --dry-run

**What to build:** The voice-pack update sequence from `docs/upload-protocol.md` over the device
link, with a range guard below it. The guard refuses any erase or write outside the FOTA partition
the headphones report (`0x1C00`), or beyond the pack rounded up to 4 KB, before it is sent.
`upload --dry-run` runs the procedure against the real headphones with a built pack, except that
erase, write, integrity check and commit are suppressed, and reports what it would have done.

**Blocked by:** 03 (Build an unchanged pack), 07 (Reverse-engineer the upload procedure), 08 (device-info)

**Status:** resolved

- [x] The sequence sends only the steps the app sends, in the app's order: MDR language update mode
      on, RACE FOTA start and queries, erase status and compare, erase, page program, integrity
      check, commit, update mode off
- [x] The range guard rejects any out-of-range erase or write, and cannot be bypassed by the procedure code
- [x] Only packs that passed `build`'s full verification are accepted
- [x] Dry-run sends no erase, write, integrity check or commit, switches update mode off again, and
      prints the reported FOTA partition, the planned address ranges and the page count
- [x] Tests against a fake device link (local, fast): the command order matches the documented
      sequence; an out-of-range request is refused before sending; dry-run sends no erase or write;
      success is reported only after the fake device's checksum check passes, and a failed check is
      reported as failure
- [x] `upload --dry-run` verified by hand against the user's headphones

## Comments

- Changed after ticket 07: the guard is about the reported FOTA partition, not the voice partition.
  The app never writes the voice partition. Dry-run has to switch language update mode on, because
  the headphones refuse `0x1C00` otherwise. That changes a mode, not flash, and dry-run switches it
  off again.
- Implemented, hand run pending. `src/fota.rs` holds the commands and reply layouts, `src/guard.rs` the
  range guard, and `src/upload.rs` the sequence and the dry-run plan. `upload::run` wraps the RACE link in
  the guard itself, so the procedure never sees the bare link. The guard decodes every command on its
  own. It lets through only the sequence's command IDs, with the app's exact payloads where there is no
  address. Erase, program, erase status and compare must lie on the reported storage, inside the
  reported partition and the pack rounded up to 4 KB. Nothing with an address goes through before
  `0x1C00` has been answered. In a dry run it also refuses `0x1C0A`, `0x1C06`, erase, program, integrity
  check and commit. Only a `BuiltPack` can be uploaded. It is made by a full unpack, a language `build`
  writes (not English), and the slot table at `0x380000`. `upload` without `--dry-run` still refuses:
  writing waits for ticket 10's confirmation. `upload --dry-run -v` logs every frame.
- Choices made beyond the protocol notes, for the user to confirm:
  - FOTA state `0x0211` (a checked image never committed) is refused, after update mode is switched off.
    The app would skip the transfer and commit whatever is staged.
  - Compare is skipped per erased block. The notes only state the skip for the all-erased case.
  - Commands go one at a time. The app keeps 4 in flight and resends after 9 s, so only the timing differs.
  - The RACE link now waits for the packet that completes a stage (`0x5D` where the notes say so). It
    stops early on an error response, and fails on a `0x1C03` cancel from the headphones.
- Left for ticket 10: after the commit the link drops, and update mode is not switched off. The app does
  that after the headphones reconnect and it has checked the language. The dry run sends `0x1C08` (FOTA
  start) and leaves that session open, like the app between transfer and install; watch for it in the
  hand run.
- Hand run on 2026-10-10 against the user's WH-1000XM4: `upload --dry-run -v` with the built German pack
  (662192 bytes). Update mode went on and off, and every query got the reply the protocol notes predict.
  The headphones report the FOTA partition at `0x00510000`, `0x5FF000` bytes, ID `0x00`, storage `0x01`,
  and state `0x0101`, which the notes do not name. All 162 blocks of the pack's range are erased, so the
  plan programs 2573 pages with no compare and no erase: the app's all-erased path. The per-block
  compare skip was not exercised. Details are in `docs/upload-protocol.md`, "Dry run on the user's
  headphones". The partition matches the second-hand voice partition at `0x0C510000`, which puts ADR
  0001's "never writes the voice partition" in doubt; left for the user to decide. After the run, the
  plan stopped calling unknown states "interrupted".
