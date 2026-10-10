# 02: Download originals

**What to build:** `download [--lang <name>]` fetches Sony's manifest and stock **voice pack** for the **base language** (English by default) from the public update server. It checks the pack against the manifest's size and SHA-1, unpacks it, and writes all 54 **clips** into `original/` as `NN-prompt-name.mp3`. `mpv original/00-power-on.mp3` then plays the stock "Power on" **prompt**.

**Blocked by:** 01 (Workspace and flake skeleton)

**Status:** ready-for-agent

- [ ] All ten base languages are selectable by name with `--lang`; English is the default
- [ ] The manifest is decoded (header lines, AES-128-ECB body, `Distribution` MAC and Size) and the downloaded pack is rejected if size or SHA-1 mismatch
- [ ] The pack is unpacked: outer SHA-256 checked, metadata block read, body decrypted (AES-128-CBC) and decompressed (raw LZMA1), unpacked-image SHA-256 checked, slot table parsed
- [ ] A slot catalogue names every slot by its prompt; unknown slots are named `unknown`
- [ ] 54 files are written to `original/`, sorted by slot number, and play in mpv
- [ ] The stock pack (and manifest) is kept so later builds can reuse its metadata without downloading again
