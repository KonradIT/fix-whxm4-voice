# Install the pack as German by default, as English only after a warning

A voice pack says itself which language it is: the first u32 of the unpacked image is the language
code (1 English, 3 German, …), and nothing in the upload carries a language. Every stock pack except
English lays its slot table out at 0x380000, where the headphones keep an installed pack. The
English pack points at 0x80000 instead, at the English built into the headphones. So a pack
installed as English would still play the built-in clips, which matches what sony-xm4-voice-mod
found.

`build` therefore writes the **install language**'s code into the image and lays the slot table out
at 0x380000, whatever the base language. The result has the same layout as that language's stock
pack. German is the default. `--install-lang` picks another. English used to be refused; it is now
built after a warning that the headphones probably keep playing their built-in English, and a
confirmation, because that is a guess the user may want to test. `upload` keeps the language of the
existing build unless `--install-lang` asks for another, so the language is chosen when building and
shown in the upload's plan. A "custom" language is dropped. The headphones advertise a fixed list of languages, and
how their firmware treats an unknown code cannot be seen without risking the device.
