# fix-whxm4-voice

Replaces the voice prompts on Sony WH-1000XM4 headphones with the user's own audio, by building a
modified voice pack and getting the Sony phone app to install it.

## Language

### Sony's side

**Voice pack**:
The per-language bundle of audio that the headphones play for status events, as distributed by Sony.
_Avoid_: firmware, VP bin, voice files

**Slot**:
A numbered position (0–53) in a voice pack; each slot belongs to one prompt.
_Avoid_: entry, index

**Prompt**:
The headphone event a slot announces, such as "Power on" or "Noise canceling".
_Avoid_: voice prompt, message

**Clip**:
The audio stored in one slot.
_Avoid_: voice file, sound, MP3

**Manifest**:
Sony's description of an available voice pack, which the phone app reads before downloading the pack.
_Avoid_: info.xml, metadata

### The user's side

**Workspace**:
The directory a user dedicates to this tool, holding their originals, changes and keys.
_Avoid_: project dir, working dir

**Original**:
A stock Sony clip, saved in the workspace so it can be listened to and used as the naming reference.
_Avoid_: stock file, default

**Change**:
A user-supplied clip that overrides the original whose slot number its file name starts with. An
empty change means the slot is silent.
_Avoid_: override, replacement, custom file

**Base language**:
The language whose stock voice pack the originals were downloaded from.
_Avoid_: source language, download language

**Install language**:
The language the headphones and app consider the uploaded voice pack to be; ideally a dedicated
"custom" language, otherwise any language other than the active one.
_Avoid_: target language, pack language

**Upload**:
Transferring a built voice pack from the PC straight to the headphones over Bluetooth, without the phone app.
_Avoid_: install, flash, push, sync
