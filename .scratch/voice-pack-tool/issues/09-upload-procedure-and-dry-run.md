# 09: Upload procedure and --dry-run

**What to build:** The voice-pack update sequence from ticket 07 over the device link, with a range guard below it that refuses any erase or write outside the voice partition before it is sent. `upload --dry-run` runs the whole procedure against the real headphones with a built pack, except that erase and write commands are suppressed, and reports what it would have done.

**Blocked by:** 03 (Build an unchanged pack), 07 (Reverse-engineer the upload procedure), 08 (device-info)

**Status:** ready-for-agent

- [ ] The sequence sends only the steps the app sends, in the app's order
- [ ] The range guard rejects any out-of-partition erase or write, and cannot be bypassed by the procedure code
- [ ] Only packs that passed `build`'s full verification are accepted
- [ ] Dry-run sends no erase or write and prints the planned address ranges and chunk count
- [ ] Tests against a fake device link (local, fast): the command order matches the documented sequence; an out-of-range request is refused before sending; dry-run sends no erase or write; success is reported only after the fake device's checksum check passes, and a failed check is reported as failure
- [ ] `upload --dry-run` verified by hand against the user's headphones
