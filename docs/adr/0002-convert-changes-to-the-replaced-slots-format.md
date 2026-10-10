# Convert each change to the format of the clip it replaces

The stock clips do not share one format. In the English pack, all are MPEG-1 Layer III, 48 kHz,
mono, constant bitrate, but 39 are 64 kbit/s and 15 are 48 kbit/s, and 29 start with an ID3v2 tag.
So a change is converted to the format of the stock clip in the slot it replaces, measured by
reading that clip's MP3 frame headers, rather than to one format for the whole pack. ffmpeg writes
bare frames (no ID3 tag, no Xing/Info frame), and every clip it makes is measured again and refused
unless it matches.

A change is always re-encoded, even when it is already an MP3 in the right format. Re-encoding an
MP3 costs a little quality, but the result is always exactly the stock format.
