# Upload protocol notes: how the Sony app delivers a voice pack to the WH-1000XM4

Findings from ticket 07. The headphones were never written to. Contact with them was limited to:
- BlueZ's cached info and one SDP browse.
- With the user's go-ahead, a handful of read-only RACE and MDR queries (see
  [Read-only probe of the user's headphones](#read-only-probe-of-the-users-headphones)).

## Sources

- **Sound Connect 12.4.2** (`com.sony.songpal.mdr`, version code 120425009), from APKPure as an XAPK.
  The base APK is signed by `CN=mobile.link, O=Sony Corporation` (certificate SHA-256
  `4276919680f4aba6d41924031c20ac2624feea5e8ec68a36529b2acb664d911e`) and carries Google's source
  stamp. It was decompiled with `jadx --no-res` and never run. The bundled Airoha SDK reports
  itself as `Ver:1.5.7.2025052218`.
- **race-toolkit**, <https://github.com/auracast-research/race-toolkit>, commit
  `eec737d3cef3f6155c513c05eddadb07ee8e9197` (2026-01-15), mainly `librace/{fota,packets,constants,util}.py`.
- **The user's headphones**: name, UUIDs and SDP records only.
- **Gadgetbridge**, <https://codeberg.org/Freeyourgadget/Gadgetbridge>, for the MDR device-info query only
  (model name and firmware version), which the APK notes do not cover. Protocol facts only.
- **All fifteen stock packs** (`VGIDLPB0401`–`VGIDLPB0410` and `VGIDLPB04F0`), downloaded with this tool
  and unpacked to compare their headers.

Class names below are as jadx printed them. Sony's and Airoha's own packages keep their names; the
others (`p466p2.c`, `p590t2.g`, …) are obfuscated and only hold for this exact APK build.

## Summary

- The app uploads a voice pack with the same Airoha FOTA engine it uses for firmware
  (`AirohaFotaAdapterSony` → `FotaControl2811` → `p466p2.c`, logged as `AirohaRaceOtaMgrS`). The pack
  `.bin` is written **verbatim**, header included, into the device's **FOTA partition**, at the
  address the device reports. The app never addresses the voice partition. The headphones check the
  file, then unpack and install it themselves after the commit.
- The FOTA link for this chip is **Bluetooth Classic RFCOMM**, not BLE.
- The **install language is not sent** anywhere in the upload. It is the first u32 of the unpacked
  image (`language` in `src/pack.rs`). The app only sends a Sony "language update mode" on/off
  command, and afterwards checks the language the headphones report.
- These points contradicted the first ADR 0001 and `CONTEXT.md`. They have since been corrected;
  see [Corrections made after ticket 07](#corrections-made-after-ticket-07).

## Transport

### Which path the WH-1000XM4 takes

`p504q9.j.p()` builds the update library. For a device that is neither true-wireless nor using LE
Audio, it logs `[Ballet Lib init] OTHER` and creates `p504q9.c` with only the Classic address. That
calls `AirohaFotaAdapterSony.setBdAddress(addr)`, which sets `mIsBleFOTA = false`. The adapter reads
the chip name (below), maps it to a `CHIP_TYPE`, and falls back to `MT2811` for any name it does not
recognise. When BLE FOTA is requested for a chip other than MT2822/MT2833/MT2855, it logs
`"BLE FOTA is only supported on MT2822 and MT2833"` and stops.

### RFCOMM channel

The FOTA SPP controller (`p680w2.a`) always opens
`createRfcommSocketToServiceRecord(8901dfa8-5c7e-4d8f-9f0c-c2b70683f5f0)` for Sony
(`p311k2.a.f69529a` "isForSony" is true for `FotaControl2811`). The user's headphones publish that
UUID in SDP:

| SDP service name     | UUID                                   | RFCOMM channel |
|----------------------|----------------------------------------|----------------|
| `Airoha_APP`         | `8901dfa8-5c7e-4d8f-9f0c-c2b70683f5f0` | 21             |
| `Serial HPC`         | `96cc203e-5068-46ad-b32d-e316f5e069ba` | 9              |
| `Serial MC`          | `b9b213ce-eeab-49e4-8fd9-aa478ed1b26b` | 11             |
| `IAPSERVER`          | `00000000-deca-fade-deca-deafdecacaff` | 10             |
| `GSOUND_BT_CONTROL`  | `f8d1fbe4-7966-4334-8024-ff96c9330e15` | 19             |
| `GSOUND_BT_AUDIO`    | `81c2e72a-0591-443e-a1ff-05f988593351` | 20             |
| `Amazon Alexa`       | `931c7e8a-540f-4686-b798-e8df0a2ad9f7` | 22             |

Look the channel up by UUID through SDP rather than hard-coding 21. race-toolkit lists the same UUID
as `SONY_SPP_UUID`.

The Sony GATT UUIDs exist in both codebases: service `dc405470-a351-4a59-97d8-2e2e3b207fbb`, TX (write)
`bfd869fa-a3f2-4c2f-bcff-3eb1ec80cead`, RX (notify) `2a6b6575-faf6-418c-923f-ccd63a56d955`. The app
uses them only on the LE path, which this chip does not take. Whether the headphones expose them over
LE was not checked, because that needs an LE connection.

## RACE framing

All values are little-endian.

```
+0  u8   head    0x15 for every FOTA-engine packet (0x05 | 0x10); some other SDK commands use 0x05
+1  u8   type    0x5A command expecting a reply, 0x5B response, 0x5C command without reply, 0x5D indication
+2  u16  length  number of bytes after this field: 2 (the ID) + payload length
+4  u16  id      command ID
+6  ...  payload
```

- Source: `p342l2.a.e()` writes `head | 0x05`, type, length, ID and payload. `f()` passes `0x10`, giving `0x15`.
- In every reply the app reads, `payload[0]` (byte 6) is the status, and 0 means success.
- The FOTA receive handler (`p466p2.b.a.a`) ignores any packet whose first byte lacks `0x10`.
- Replies can arrive split across reads. race-toolkit reassembles them from the length field.
- race-toolkit's `RaceHeader` uses the same layout.

## The voice-pack update sequence

### Order of steps

The update is driven by `MtkUpdateController` (`Target.VOICE_GUIDANCE`). Single device,
`isTwsMode = false`. Steps marked **MDR** go over Sony's own protocol; see
[Sony MDR commands](#sony-mdr-commands-around-the-transfer). Everything else is RACE over the
`Airoha_APP` channel.

1. **MDR** "language update mode" → ENABLE (`MtkUpdateController.p0`, `f53262t.d(true)`).
2. Chip name: NV-key read `0x0A00`.
3. Query, `querySingleFotaInfo()`: `0x1C08` FOTA start → `0x1C1C` transmit interval → `0x1C00`
   partition info → `0x1C04` FOTA state.
4. If the state is `0x0211` (integrity already passed), the app skips straight to "transferred".
   Otherwise it runs the transfer, `startResumableEraseProgramFotaV2StorageExt` (`p466p2.b.r0`):
   `0x1C19` (see below) → `0x0433` erase status → `0x0431` compare →
   `0x1C0A` start transaction → `0x1C06` state `0x0200` → `0x0404` erase → `0x1C06` `0x0201` →
   `0x1C06` `0x0210` → `0x0402` page program → `0x1C01` integrity check → `0x1C06` `0x0211` →
   `0x1C04` query state.
5. "Transferred". **MDR** update mode → DISABLE.
6. Install, `MtkUpdateController.n0`: **MDR** update mode → ENABLE, then `startCommitProcess()` →
   `0x1C02` commit. The app treats the link dropping as success: `05_Commit` waits up to 15 s for
   the disconnect.
7. After the headphones reconnect, the app checks the language
   (`com.sony.songpal.mdr.j2objc.application.update.mtk.b.e`, with a 240 s timeout):
   `INSTALL_COMPLETED` if the language the headphones report equals the expected one, otherwise
   `INSTALL_FAILED`. Then **MDR** update mode → DISABLE.

`0x1C19` is queued by `p466p2.b.U()`:
- One `0x1C19 00` when the single-device flag is set (`f74683P`, set in foreground, non-TWS mode).
- Two (`00` and `01`) when the TWS flag is set (`f74682O`).
- None in background mode, where both flags are cleared.

### Stage skips

The erase-status and compare results let the app skip stages (`IAirohaFotaStage.SKIP_TYPE`):

- Every 4 KB block already erased: skip compare, the `0x0200` state write and erase.
- Every block's SHA-256 already matches the pack: skip `0x0200`, erase, `0x0201`, `0x0210` and
  program, but still run `0x1C0A` and the integrity check.
- Some blocks match: those blocks are not erased or programmed.

### Progress

The integrity check is assigned the 99 → 100 % step (`p466p2.c.H0`: `new e(99, 1)`). This matches the
known "stalls at 99 %" symptom of a wrong outer SHA-256.

### Commands

`storage` is the storage-type byte from the `0x1C00` reply. "Reply" means the packet that completes
the stage: type `0x5B`, or `0x5D` where noted.

- **`0x0A00` read NV key** (reads the chip name)
  - Sends: `02 10 E8 03` (NV key `0x1002`, max length 1000)
  - Reply `0x5B`: [6..8) u16 length, then the chip name as ASCII.
- **`0x1C08` FOTA start** (`FotaStage_00_Start`)
  - Sends: `01` for a single device (`03` for TWS)
  - Reply `0x5D`: status only. Non-zero means `FOTA_START_FAIL`.
- **`0x1C1C` query transmit interval** (`FotaStage_07`)
  - Sends: `01 00`
  - Reply `0x5D`: [8] role, [9..11) u16 interval in ms. The app skips this stage on timeout.
- **`0x1C00` partition info** (`RACE_FOTA_PARTITION_INFO_QUERY`)
  - Sends: `00`
  - Reply `0x5B`: [7] partition ID, [8] storage type, [9..13) u32 FOTA partition address,
    [13..17) u32 length.
- **`0x1C04` query state** (`RACE_FOTA_QUERY_STATE`)
  - Sends: nothing
  - Reply `0x5B`: [7..9) u16 state. `0xFFFF` means no state yet; `0x0211` means the integrity check passed.
- **`0x1C19` active FOTA** (`TwsActiveFota`)
  - Sends: `00`
  - Reply `0x5D`: [7] agent or client.
- **`0x0433` erase status** (`GetPartitionEraseStatusStorage`)
  - Sends: `storage, 00, u32 addr, u32 len`, at most 2 MiB per request
  - Reply `0x5D`: [8] role, [9..13) addr, [13..17) len, [17..19) u16 bitmap size, then the bitmap:
    one bit per 4 KB block, MSB first, 1 = erased.
- **`0x0431` compare** (`ComparePartitionV2Storage`)
  - Sends: `storage, 00, u32 addr, u32 len`
  - Reply `0x5D`: [8] role, [9..13) addr, [17..49) SHA-256 of that flash range.
- **`0x1C0A` start transaction** (`RACE_FOTA_START_TRANSCATION`)
  - Sends: nothing
  - Reply `0x5B`: status only.
- **`0x1C06` write state** (`RACE_FOTA_WRITE_STATE`)
  - Sends: u16 state, one of `0x0200`, `0x0201`, `0x0210` or `0x0211`
  - Reply `0x5B`: status only.
- **`0x0404` erase** (`DiffFlashPartitionEraseStorage`)
  - Sends: `storage, u32 len = 0x1000, u32 addr`, one command per 4 KB block
  - Reply `0x5D`: [8..12) len, [12..16) addr.
- **`0x0402` page program** (`RACE_STORAGE_PAGE_PROGRAM`)
  - Sends: `storage, n, n × (u8 crc, u32 addr, 256 data bytes)`. The app always sends n = 1.
  - Reply `0x5B`: [7] storage, [8] n, then n × u32 addr.
- **`0x1C01` integrity check** (`FotaStage_04_CheckIntegrity`)
  - Sends: `01, 00, storage`
  - Reply `0x5D`: [7] recipient count, [8] recipient, [9] storage. Status 0 means the check passed.
- **`0x1C02` commit** (`05_Commit`)
  - Sends: `00`
  - A non-zero status means `COMMIT_FAIL`. Success is the link dropping.
- **`0x1C03` cancel**
  - App → device: `07, 01, reason`.
  - The device can also send `0x1C03` (type `0x5A`). Its byte [8] is the reason: 0 cancel, 1 fail,
    2 timeout, 3 partner loss, 4 not allowed.

Read-only, as far as the code shows: `0x0A00`, `0x1C00`, `0x1C04`, `0x1C1C`, `0x0433` and `0x0431`.
`0x1C08` opens a FOTA session and is not read-only, even though the app sends it during its "query"
phase.

### Data layout, chunking and page skips

- The pack is split into 4 KB blocks from the FOTA partition address upwards: block *k* sits at
  `addr + 0x1000·k` (`p590t2.h`). The last block is as long as the data left and is not padded.
- **Erase** works per 4 KB block (`0x0404`, length `0x1000`). Only blocks that differ and are not
  already erased are erased. This is not a whole-partition erase.
- **Program** works per 256-byte page. Each block is cut into 256-byte pages, and a short last page
  is padded with `0xFF`. A page made entirely of `0xFF` bytes is **skipped** (`B2.b.a`). In
  foreground mode each `0x0402` command carries one page.
- **Page checksum**: table-driven CRC-8, polynomial `0x31`, seeded with 0. The result is finally
  bit-reversed: `rev4[crc >> 4] | rev4[crc & 15] << 4`, with `rev4` reversing 4 bits (`B2.a`).
  race-toolkit's `fota_checksum` uses the same two tables. A bad checksum makes the device answer status `0x0A`.
- **Flow control** (foreground, the non-background default): 4 commands are sent at first, and one
  more after each reply of the expected type. After 9 s without replies the app resends the
  unanswered commands, at most 3 times (`CMD_RETRY_FAIL`). In background mode several commands share
  one RFCOMM write with a 200 ms delay. Whether the WH-1000XM4 reports background mode is a device
  capability that was not read.

## Sony MDR commands around the transfer

"Language update mode" is an MDR v1 table-2 message on data type `DATA_MDR_NO2` (`0x0E`). It is built
in `Uj.c.d()` (the table-set-1 voice-guidance implementation) and `Pl.x`.

- **Set**: `VOICE_GUIDANCE_SET_STATUS` = `44 01 02 xx`, where `01` is the voice-guidance setting, `02` is
  the LANGUAGE status type, and `xx` is `00` ENABLE or `01` DISABLE.
- **Reply**: `VOICE_GUIDANCE_NTFY_STATUS` = `45 01 02 xx`. The app treats the change as done when `xx`
  matches what it set.
- **Framing** (`Dl.b.e`): `3E`, then the escaped bytes `type, seq, u32 BE length, payload, checksum`,
  then `3C`. The checksum is the low byte of the sum of `type`, `seq`, the length bytes and the
  payload. The bytes `3C`, `3D` and `3E` are escaped as `3D` followed by the byte with bit `0x10`
  cleared. ACK frames are `3E 01 <seq> 00 00 00 00 <sum> 3C`.

The code does not say which RFCOMM channel carries MDR. The probe confirmed it is `Serial HPC`
(`96cc203e-…`, channel 9 on the user's unit).

Read-only MDR queries the app uses, all `VOICE_GUIDANCE_GET_*` with inquired type `01`:

| Request | Meaning | Reply |
|---|---|---|
| `42 01 02` | language-update status | `43 01 02 <CommonStatus>` |
| `46 01 02` | current language | `47 01 02 <MdrLanguage>` |
| `46 01 04` | server and languages | `47 01 04 01`, category ID, serial, n × (lang, service ID); see below |

The last reply is how the app learns the category ID (`HP002`) and each language's service ID
(`VGIDLPB04xx`): the **headphones** supply the list of languages. Its exact layout (`Pl.k`) is
`47 01 04 01, u8 catLen, category ID, u8 serialLen, serial, u8 n`, then n entries of
`u8 MdrLanguage, u8 idLen, service ID`.

`MdrLanguage` codes (`com.sony.songpal.tandemfamily.mdr.param.MdrLanguage`):

| Code | Language             | Code | Language            |
|------|----------------------|------|---------------------|
| 0x00 | undefined            | 0x09 | Finnish             |
| 0x01 | English              | 0x0A | Russian             |
| 0x02 | French               | 0x0B | Japanese            |
| 0x03 | German               | 0x0C | Simplified Chinese  |
| 0x04 | Spanish              | 0x0D | Brazilian Portuguese |
| 0x05 | Italian              | 0x0E | Traditional Chinese |
| 0x06 | Portuguese           | 0x0F | Korean              |
| 0x07 | Dutch                | 0x10 | Turkish             |
| 0x08 | Swedish              | 0xF0 | Chinese             |

The `xx` in `VGIDLPB04xx` is this code in hex. Turkish is `0x10`, and so `VGIDLPB0410`.

Read-only MDR queries on data type `DATA_MDR` (`0x0C`). These are not taken from the APK but from
the MDR v1 device-info query as Gadgetbridge sends it (`CONNECT_GET_DEVICE_INFO`). Ticket 08 confirmed
them on the user's unit:

| Request | Meaning | Reply |
|---|---|---|
| `00 00` | protocol info | `01 00 70 00` |
| `04 01` | model name | `05 01 <len>` + ASCII, `WH-1000XM4` |
| `04 02` | firmware version | `05 02 <len>` + ASCII, `3.0.1` |

**Sequence numbers.** Each side acknowledges every data frame with an ACK whose seq is `1 − seq` of
that frame. The sender's next command uses the seq of the last ACK it received. The headphones
follow this rule (see the log in ticket 08).

`device-info` sends exactly these MDR queries: protocol info, model name, firmware version,
voice-guidance capability (`40 01`), update-mode status, current language, and server and languages.
Its only RACE command is the `0x0A00` chip-name read.

## The install language

The app sends no language to the headphones. The only language-related message is the
update-mode toggle above, which has no language field. The expected language is used only by the app's
own after-reboot check (`mtk.b.i()` / `mtk.b.e()`).

The language is part of the pack. The first u32 of the unpacked image equals the pack's `MdrLanguage`
code in all fifteen stock packs:

| Pack                 | image u32 @ 0 | first slot offset | slot base (first offset − 0x1B8) |
|----------------------|---------------|-------------------|----------------------------------|
| English  `0401`      | `0x01`        | 524728            | **`0x080000`**                   |
| French   `0402`      | `0x02`        | 3670456           | `0x380000`                       |
| German   `0403`      | `0x03`        | 3670456           | `0x380000`                       |
| Spanish  `0404`      | `0x04`        | 3670456           | `0x380000`                       |
| Italian  `0405`      | `0x05`        | 3670456           | `0x380000`                       |
| Portuguese `0406`    | `0x06`        | 3670456           | `0x380000`                       |
| Dutch    `0407`      | `0x07`        | 3670456           | `0x380000`                       |
| Swedish  `0408`      | `0x08`        | 3670456           | `0x380000`                       |
| Finnish  `0409`      | `0x09`        | 3670456           | `0x380000`                       |
| Russian  `040A`      | `0x0A`        | 3670456           | `0x380000`                       |
| Japanese `040B`      | `0x0B`        | 2621880           | **`0x280000`**                   |
| Brazilian Portuguese `040D` | `0x0D` | 3670456         | `0x380000`                       |
| Korean   `040F`      | `0x0F`        | 3670456           | `0x380000`                       |
| Turkish  `0410`      | `0x10`        | 3670456           | `0x380000`                       |
| Chinese  `04F0`      | `0xF0`        | 1573304           | **`0x180000`**                   |

Sony publishes no pack for Simplified Chinese (`0x0C`) or Traditional Chinese (`0x0E`): their
`info.xml` returns 404, although the headphones list them. Japanese and Chinese count their slots from
another base than the `0x380000` that `build` writes, and where the headphones keep a pack installed in
those languages is not known. So they are base languages only: `--install-lang` refuses them. For the
other packs at `0x380000`, an unchanged build reproduces the stock unpacked image exactly.

All fifteen packs share the same TLV `0x0012` (`01000000 00100000 00001000 0000380C`): version 1, header
size `0x1000`, unpacked size `0x100000`, and a last field `0x0C380000`.

### A "custom" language code: unknown, probably not safe

- The headphones advertise their own fixed list of languages and service IDs (`46 01 04`). The app
  only offers languages from that list.
- In the app, `VoiceGuidanceLanguage.fromByteCode` maps any code outside the table to
  `UNDEFINED_LANGUAGE`. So a pack carrying an unknown code would show as "undefined" in the app, and
  the app's own check would report the install as failed. That does not matter for an upload from
  the PC.
- How the **firmware** handles an unknown code in the image header is not visible from the app.
  Selecting prompts, NVDM and fallback could all depend on it, so it stays **unknown**. Settling it
  needs firmware reverse engineering or a test the user accepts the risk of.

## The voice partition's address range: not settled

The app does not need this, because it never writes the voice partition. The evidence available:

- **Image placement.** TLV `0x0012`'s last field is `0x0C380000` in every pack. The 1 MiB image size
  and the non-English slot base `0x380000` fit the reading "the unpacked image is installed at flash
  `0x0C380000`, and slot offsets are flash offsets below the `0x0C000000` mapping".
- **English is different.** Its slot table points at `0x080000`, outside that 1 MiB. That looks like
  a factory English bank, so an English pack would reference built-in clips rather than its own.
- **A conflicting claim.** sony-xm4-voice-mod's `NOTES.md` gives a voice partition of 6 MB at
  `0x0C510000` and FOTA staging of 2 MB at `0x081B9000`. These are second-hand (attributed to
  firmware reverse engineering and HelgeSverre's write-up), not a device read.
- **Reading flash is refused.** A RACE page read (`0x0403`) of the partition table at address 0
  returned status `0x03` and leftover buffer contents, so this firmware blocks flash reads. The
  `0x1C00` partition query is only answered in language update mode (see the probe below).

## Clues about the "same language is ignored" check

- **The install language is in the pack** (image u32 @ 0). The firmware therefore knows whether a
  new pack claims the active language. A check of "same language → keep the current bank / default
  prompts" would live there. This is not proven.
- **English packs point at a separate bank.** A pack built from the English base used to keep that
  pointer, so its changed clips would never be referenced. sony-xm4-voice-mod independently found
  that modding English on an English device fails while Spanish-from-Italian works. Its notes call
  the image field a constant `version (1)`; the table above shows it is the language code. `build`
  now writes the install language and the base `0x380000` (ADR 0003).
- **A staged image can be committed again.** If `0x1C04` already reports `0x0211` (a staged image
  passed its check but was not committed), the app skips the whole transfer and commits whatever
  is staged. An interrupted earlier stock install could make a later "install" re-apply the old
  image. This is speculative.

## Encryption of writes

The ticket asks about writes to the voice partition. The app makes none, so the answer here covers
the writes it does make, which go to the FOTA partition.

No layer of the upload adds encryption. RACE payloads are plaintext, and the pack bytes are written
as they are. The pack body is already AES-128-CBC encrypted by the pack format, and the headphones
decrypt it on install. Each page carries only the CRC-8 above. Link-level Bluetooth encryption comes
from the existing pairing.

## race-toolkit cross-check and licence

**Licence.** The repository has **no licence file**, and neither its README nor its `pyproject.toml`
declares one. By default this means all rights are reserved, so its **code may not be copied** into
this project. Protocol facts such as IDs, layouts and the CRC tables are not copyrightable. They
appear here as observed in the APK and only cross-checked against race-toolkit.

Agreement between race-toolkit and the app: the command IDs, the header layout, the `0x1C00` reply
layout (its "???" u16 is the partition ID and storage type), the erase payload
`storage, len, addr`, the page payload `crc, addr, 256 bytes`, the CRC tables, integrity payload
`01 00 00`, and commit `00`.

Where race-toolkit differs from the app:

| Topic | race-toolkit | Sound Connect 12.4.2 |
|---|---|---|
| Erase | whole FOTA partition in one `0x0404` | per 4 KB block, only blocks that differ |
| Page program | `chunks_per_write` pages per command, writes `0xFF` pages | 1 page per command, skips `0xFF` pages |
| Program address | advances by a fixed `0x300` (right only for 3 pages) | advances page by page |
| States | `0x0201`, `0x0210`, erase, … `0x0211` | `0x0200`, erase, `0x0201`, `0x0210`, program, … `0x0211` |
| `0x1C08` payload | `01 00` | `01` |
| Head byte | `0x05` for `0x1C00` and erase | `0x15` for everything |
| Pre-checks | none | `0x1C1C`, `0x1C04`, `0x1C19`, erase status, compare |
| Tested on | WH-CH720N, WH-1000XM6 (firmware, not voice packs) | — |

## Read-only probe of the user's headphones

All queries were sent over RFCOMM with a throwaway script that let only read and query commands
through. Commands in hex, replies abridged.

RACE on channel 21 (`Airoha_APP`):

| Sent | Reply | Meaning |
|---|---|---|
| `0x0A00` NV key `0x1002` | `05 5b 05 00 00 0a 00 00 00` | empty chip name; the app falls back to MT2811 |
| `0x1C00` partition info | `15 5a 05 00 03 1c 00 80 04` | `0x1C03` cancel, reason 4: not allowed |
| `0x1C04` FOTA state | same `0x1C03`, reason 4 | not allowed either |
| `0x0403` read 256 B at 0 | status `0x03` plus stale buffer bytes | flash reads are blocked |

So FOTA queries are refused until the MDR "language update mode" is enabled. That is a set command,
so it was not sent. The FOTA partition's address therefore stays unknown until the first upload.

MDR on channel 9 (`Serial HPC`), data type `0x0E` (`0x0C` for the first query):

| Sent | Reply | Meaning |
|---|---|---|
| `00 00` (protocol info) | `01 00 70 00` | MDR answers; table set 1 commands work |
| `40 01` (voice-guidance capability) | `41 01 01 01 0f` + 15 codes | languages 0x01–0x0B, 0x0D, 0x0F, 0x10, 0xF0 |
| `42 01 02` (update-mode status) | `43 01 02 01` | language update mode is off |
| `46 01 02` (current language) | `47 01 02 01` | English is active |
| `46 01 04` (server, languages) | `HP002`, serial number, 15 service IDs | `xx` = code in hex |

This confirms the MDR framing, the channel, the language codes and the service-ID rule. It also
confirms that a German pack is not the active language on these headphones.

## Corrections made after ticket 07

The findings contradicted the first plan. These have been corrected:
- **Transport and target.** ADR 0001 now describes Bluetooth Classic RFCOMM and writes into the FOTA
  partition the headphones report. Before, it said BLE and the voice partition.
- **Procedure.** The MDR update-mode switch is now part of the procedure. Erase works in 4 KB blocks
  and writes in 256-byte pages.
- **Install language.** `CONTEXT.md` and ADR 0003 now define the install language as the language
  code in the pack: German by default, English refused, no "custom" language.
- **The image header.** `src/pack.rs` now calls the image's first u32 `language`, and `build` lays
  the slot table out at `0x380000`.

`info.md` remains reference material and still says "version" and "typically 0x80000".

Not confirmed: ADR 0001's "integrity check". The app only triggers `0x1C01` and reads a status. That
it checks the outer SHA-256 is inferred from the 99 % stall when that hash is wrong (see Progress).

## Dry run on the user's headphones (ticket 09)

`upload --dry-run` on 2026-10-10, firmware `3.0.1`, English active. It switched language update mode on,
sent the queries up to compare, and switched update mode off. Replies, as payloads (status first):

| Sent | Reply | Meaning |
|---|---|---|
| `0x0A00` NV key `0x1002` | `00 00 31` | differs from the `00 00 00` outside update mode; not understood |
| `0x1C08` `01` | `0x5B` `00`, then `0x5D` `00` | FOTA start: a response, then the indication that completes it |
| `0x1C1C` `01 00` | `0x5B` `00`, then `0x5D` `00 01 00 c8 00` | role 0, interval 200 ms |
| `0x1C00` `00` | `00 00 01 00 00 51 00 00 f0 5f 00` | partition ID `0x00`, storage `0x01`, address `0x00510000`, length `0x5FF000` |
| `0x1C04` | `00 01 01` | state `0x0101`, which the APK notes do not name |
| `0x1C19` `00` | `0x5B` `00`, then `0x5D` `00 00` | |
| `0x0433` `01 00`, `0x510000`, `0xA2000` | `0x5D`: echo of the range, bitmap size 21, `ff`×20 `c0` | all 162 blocks erased |

- **The FOTA partition and the voice partition.** `0x00510000` plus about 6 MB matches
  sony-xm4-voice-mod's second-hand voice partition, 6 MB at `0x0C510000`, read below the `0x0C000000`
  mapping (see [The voice partition's address range](#the-voice-partitions-address-range-not-settled)).
  So in language update mode, the partition the headphones report for FOTA looks like that region. This
  contradicts "the app never addresses the voice partition" in the summary and ADR 0001. The upload
  still writes only where the headphones point it.
- **Every block of the pack's range is erased.** With English active, that fits English playing from
  the built-in bank and the installed-pack area being empty.
- **The state `0x0101` stays unexplained.** `0x1C08` came before the state query, so it may have
  set it.
- **Replies arrive in pairs.** Several replies came as a `0x5B` and a `0x5D` in one read.

## Still unknown

- The meaning of FOTA state `0x0101`, and whether `0x1C08` sets it.
- Whether the headphones report background or resumable transfer.
- The voice partition's address range: flash reads are blocked.
- Firmware behaviour for an unknown language code.
- The firmware's same-language rule.
