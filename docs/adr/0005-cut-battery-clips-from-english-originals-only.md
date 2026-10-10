# Cut short battery clips from the English originals only, offered by `generate`

The battery prompts are whole sentences ("battery, about seventy percent", "battery, fully
charged"). `generate` can shorten them to the number ("seventy") or "full" by cutting that word out
of the original with ffmpeg, in `sounds/battery.sh`, next to the tone script. A word cut from a
sentence ends early, where it runs into the next word, so the script also stretches its last sound
and fades it out. The cut points were picked by ear for each clip and are times in the English
originals, so the script fits the English stock voice pack only.

`generate` therefore checks the **base language**, read from the stock voice pack:

- English originals: it asks whether to cut the battery clips, defaulting to no.
- Originals in another language: it says the clips can only be cut from English originals and
  skips them, without asking, even with `--battery`. The tones are generated as usual.
- No originals yet: it asks whether to download the English originals and cut the clips. With
  `--lang` naming another language, it skips them as above.

`--battery` answers yes and `--no-battery` answers no, so `generate` runs without questions. The
battery clips go through the same checks as the tones: a change already in `changes/` for one of
their slots is refused unless `-f` is given.
