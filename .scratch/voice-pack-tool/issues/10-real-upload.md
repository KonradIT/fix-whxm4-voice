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

- [x] A stale build is rebuilt automatically before uploading
- [x] The phone-Bluetooth warning and the single-connection check happen before any write
- [x] Nothing is erased or written without the typed confirmation
- [x] Progress is shown, and done is reported only after the headphones confirm the checksum check
- [x] A rejected pack (e.g. at 99%) gives a clear message, including that picking a stock language
      in the Sony app restores a stock voice pack
- [x] The pack's install language is shown in the plan, and a warning appears when it is the active one
- [x] The default command's offer to upload ("Upload N changes now? [y/N]") appears when changes exist
- [ ] Verified by hand: a pack with at least one change uploads and the headphones play it

## Comments

- Changed after ticket 07: the install language is no longer picked at upload time. It is baked
  into the pack by `build` (ADR 0003), and `upload` reads it from the built pack.
- Implemented, hand run pending. `build::fresh` rebuilds when there is no build, when `original/`, `changes/`
  or a file in them is newer than `build/pack.bin` (a removed change touches its folder), or when the build's
  install language differs from `--install-lang`. `upload::run` takes an `Operator` that sees the plan and
  follows the progress. The guard starts with writes refused for every upload and allows them only after the
  operator confirmed the plan, so no write can reach the headphones before that. The plan now shows the
  install language and the language the headphones play (`46 01 02`, read-only, sent before update mode
  goes on), with a warning when they match. Progress counts each erase, each page and the checksum check;
  it reaches 100 % only after the check passes, and the terminal shows it as a progress bar. Every failure
  after the confirmation ends with the hint that picking a stock language in the Sony app restores a stock
  pack. The default command asks "Upload N changes now? [y/N]" when `changes/` holds files.
- Choices made beyond the ticket, for the user to confirm:
  - The typed confirmation is the word `upload`. Declining exits with status 1.
  - The single-connection check covers what the PC can see: exactly one connected device offering Sony's
    MDR channel (`Headphones::find`). Whether the headphones are also connected to a phone isn't visible
    without an MDR command outside the protocol notes, so the tool only asks the user to turn the phone's
    Bluetooth off and press Enter.
  - `--install-lang` differing from the built pack's language counts as stale, so `upload --install-lang X`
    always installs as X.
  - `--dry-run` also rebuilds a stale build, so its plan matches what an upload would send.
- Still open: after the commit the headphones restart and update mode is not switched off by the tool (the
  app does that after reconnecting). The tool suggests `device-info` to check the language afterwards.
- Two gaps closed after review:
  - FOTA state `0x0211` (a checked pack waiting for its commit) no longer always refuses. The tool compares
    the waiting image block by block with the pack. If it is this pack, the plan says so and, once confirmed,
    only the commit is sent, as the app does in that state. A different waiting pack is still refused with
    nothing written, because how the headphones take an upload over it is unknown.
  - The commit waits 15 s for the link to drop, as the app does (`fota::COMMIT_TIMEOUT`; `Transport::receive`
    now takes the timeout). Neither a reply nor a drop by then is reported as "may still install, check with
    `device-info`".
- `SAFETY.md` at the root explains to users, stage by stage, what a failure leaves the headphones in and how to
  get back to Sony's voice prompts.
- Changed after the user's first real upload, which succeeded while the headphones were still connected to
  the phone: the tool no longer asks to turn off the phone's Bluetooth. Before the typed confirmation it says
  to leave the headphones alone during the upload: no audio, no buttons, no Sony app.
- Changed on the user's request after the first upload:
  - The install language is chosen when building: `build` uses `--install-lang` or German. `upload` keeps the
    existing build's language (also when it rebuilds a stale build) unless `--install-lang` asks for another.
    The build line no longer names the language; the plan does.
  - English is built after a warning and a `[y/N]` confirmation, and the plan warns for an English pack
    (ADR 0003 updated).
  - A different checked pack waiting for its commit is overwritten by the upload instead of refused. The app
    never does this; whether the headphones accept it is untested.
  - A silent clip is now the shortest ffmpeg makes: two frames, 48 ms (was 0.1 s, six frames).
  - The next steps say to run the default command again, not `upload`.
