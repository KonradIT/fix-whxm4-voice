# 03: Build an unchanged pack

**What to build:** `build` with an empty `changes/` packs the originals back into a voice pack in `build/`. It recomputes every checksum and verifies the result by unpacking it again: decrypt, decompress, check every SHA-256, parse the slot table, read every clip. The rebuilt unpacked image is identical to the stock one. This ticket also adds the pack codec's automated tests.

**Blocked by:** 02 (Download originals)

**Status:** ready-for-agent

- [ ] Packing writes the metadata block (tags 0x0011–0x0014) with correct sizes and the unpacked-image SHA-256, the AES-128-CBC body padded with 0xFF, and the outer SHA-256 over 0x100..end, computed last
- [ ] The unpacked image is zero-padded to exactly 1 MiB, with slot offsets relative to the voice partition base taken from the stock pack
- [ ] Every built pack is verified by a full unpack; a failure aborts the build with a clear error
- [ ] With no changes, the rebuilt unpacked image matches the stock image byte for byte
- [ ] Tests (local, fast, no network or Sony files): a generated pack round-trips; tampering with any checksummed region is detected; an image over 1 MiB is rejected
