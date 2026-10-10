# Task: Tool to replace Sony WH-1000XM4 voice prompts with tones

## Goal
Build a CLI tool, run on Linux (NixOS, flakes), that:
1. downloads a stock Sony voice pack;
2. replaces voice prompts with short tones or silence;
3. repacks the pack in Sony's format;
4. serves the modified pack to the Sony phone app via a local HTTPS proxy, so the app installs it on the headphones.

## Reference implementations (read these first; reuse code where possible)
- https://github.com/SantiMoncada/sony-xm4-voice-mod — working end-to-end mod (pack + manifest
  forge + mitmproxy addon). Confirmed working on firmware 3.0.1, tested with an iPhone.
  - `pack_voice_pack.py`: builds the patched pack.
  - `pack_info_xml.py`: forges the manifest.
  - `swap_vp.py`: mitmproxy addon that serves the patched files.
  - `extract_slots.py`: dumps the MP3 slots.
  - `NOTES.md`: full format notes and dead ends.
- https://github.com/HelgeSverre/sony-vp-extract — extractor plus `docs/WRITEUP.md` (format and key derivation).
  - The WRITEUP calls the first 32 header bytes "random". This is WRONG: they are a SHA-256 checksum (see below).

## 1. Retrieving files (public CDN, no authentication)
- Voice pack: `https://info.update.sony.net/HP002/VGIDLPB04XX/contents/0002/VP_<lang>_UPG_03.bin`
- Manifest template: `https://info.update.sony.net/HP002/VGIDLPB04XX/info/info.xml`
- Language codes (XX → lang):
  - 01 english, 02 french, 03 german, 04 spanish, 05 italian
  - 06 portuguese, 07 dutch, 08 swedish, 09 finnish, 10 turkish

## 2. Voice pack (.bin) format
```
0x0000-0x001F  SHA-256(file[0x100:end])  -- checked by the device at install; must be recomputed
0x0100+        TLV metadata: compression_type=2, body_size, decompressed_size,
               SHA-256 of decompressed image (tag 0x0013 = "verion_string")
0x1000+        body: AES-128-CBC encrypted LZMA1 stream
```
- AES-128-CBC: key `eibohjeCh6uegahf`, IV `miefeinuShu9eilo` (ASCII). Identical for all units and all packs.
- LZMA1 raw stream:
  - 13-byte header: props `0x5D` (lc=3, lp=0, pb=2), dict_size 16384.
- Decompressed image (~1 MB):
  - Header: u32 version, u32 num_entries (54).
  - Entry table: 54 × (u32 size, u32 absolute_offset).
  - Absolute offsets refer to the 6 MB voice partition. Base = first_offset − (8 + 54·8), typically 0x80000.
  - Payload: MP3s.
- MP3 requirements: 48 kHz, mono, ~64 kbps (`ffmpeg -i in -ar 48000 -ac 1 -b:a 64k out.mp3`).
- Unknown details: exact TLV tag numbers, AES padding, alignment. Take them from `pack_voice_pack.py`.
- Keep the total image size ≤ the original (safe assumption).

## 3. Slot map (index → prompt)
| Slots | Prompt |
|---|---|
| 0, 1 | Power on, Power off |
| 2 | Bluetooth pairing |
| 3, 4 | Bluetooth connected, Bluetooth disconnected |
| 5 | Recharge / power off |
| 6, 7, 8 | Noise canceling, ASC off, Ambient sound |
| 9 | Assistant-related |
| 15 | Battery fully charged |
| 16–18, 28–33 | Battery percentages |
| 34 | Low battery |
| 35, 36 | Optimizer start, Optimizer finished |
| 43, 44 | Speak-to-chat on/off |
| 47–53 | Per-device connect/disconnect |
| 10–14, 19–27 (mostly), 37, 39–42, 45, 46 | Existing tones |

Goal mapping: replace every speech slot with an existing tone clip (or 50–100 ms of silence). Make
this configurable via a mapping file, e.g. TOML: `slot -> source (tone slot | mp3 path | silence)`.

## 4. Manifest (info.xml) format
- Plaintext header lines, then a blank line, then the body:
  ```
  eaid:ENC0003
  daid:HAS0003
  digest:<sha1 hex>

  <body>
  ```
- Body: AES-128-ECB, key `4fa27999ffd08b1fe4d260d57b6d3c17` (hex), no IV.
- Decrypted XML: `<Distribution ID="FW" URI="..." MAC="<sha1 of .bin>" Size="<bytes>" Version="3"/>`.
- After building the patched pack, update `MAC` (SHA-1 of the patched .bin) and `Size`, re-encrypt,
  and recompute `digest`. Follow `pack_info_xml.py` for the exact digest input.
- The manifest also references `disclaimer.xml`. Always serve the English disclaimer, whose SHA-1
  matches the inherited MAC.

## 5. Upload (delivery to the headphones)
1. Run mitmproxy with an addon that intercepts requests to `info.update.sony.net` and serves:
   - the forged `info.xml` for the target language path;
   - the patched `.bin`;
   - the English `disclaimer.xml`.
2. Phone: set the Wi-Fi proxy to `<laptop-ip>:8080`, open `http://mitm.it`, and install and fully trust the CA.
   - iOS: tested.
   - Android: apps ignore user CAs since Android 7; this needs a patched app (apk-mitm) or root. Untested.
3. In the Sony app (Sound Connect / Headphones Connect), switch the voice-guidance language to a
   DIFFERENT language than the one currently active. Install the custom pack under that language
   code.
   - Switching to the same language reports success, but the device keeps playing the stock prompts.
4. Wait until the install completes. If the outer SHA-256 is wrong, it stalls at 99%.

## 6. Safety / risks
- This uses Sony's normal language-switch path, not raw flash writes. Do NOT use RACE flash-write
  tools (known to brick devices).
- Firmware cannot be downgraded.
- Before upload, validate every pack by round-trip: decrypt → decompress → verify SHA-256s →
  parse table → extract all MP3s.

## 7. Deliverables
- `flake.nix` with a devShell providing: python3 (pycryptodome), uv, ffmpeg, mitmproxy.
- CLI subcommands:
  - `download <lang>`
  - `extract <bin> <dir>`
  - `build --base <lang> --target-lang <code> --map map.toml`
  - `serve` (mitmproxy addon)
  - `verify <bin>`
- Tests: round-trip of an unmodified pack must reproduce identical decompressed content and valid checksums.
