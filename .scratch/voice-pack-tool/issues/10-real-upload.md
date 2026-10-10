# 10: Real upload

**What to build:** `upload` writes the built voice pack to the headphones. It rebuilds first when
`original/` or `changes/` are newer than the last build. It asks the user to turn off Bluetooth on
the phone and checks the PC is the only connection it can see. It shows the plan (pack size, address
range, install language) and requires a typed confirmation. Progress is shown during the transfer,
and the upload is reported as done only when the headphones confirm their checksum check. The pack
is installed as the install language it was built as (`--install-lang`, German by default; see
ADR 0003). If the headphones report that language as already active, `upload` warns that they may
ignore the pack. The default command now offers to upload when changes exist, defaulting to "no".

**Blocked by:** 04 (Changes override originals), 09 (Upload procedure and --dry-run)

**Status:** ready-for-agent

- [ ] A stale build is rebuilt automatically before uploading
- [ ] The phone-Bluetooth warning and the single-connection check happen before any write
- [ ] Nothing is erased or written without the typed confirmation
- [ ] Progress is shown, and done is reported only after the headphones confirm the checksum check
- [ ] A rejected pack (e.g. at 99%) gives a clear message, including that picking a stock language
      in the Sony app restores a stock voice pack
- [ ] The pack's install language is shown in the plan, and a warning appears when it is the active one
- [ ] The default command's offer to upload ("Upload N changes now? [y/N]") appears when changes exist
- [ ] Verified by hand: a pack with at least one change uploads and the headphones play it

## Comments

- Changed after ticket 07: the install language is no longer picked at upload time. It is baked
  into the pack by `build` (ADR 0003), and `upload` reads it from the built pack.
