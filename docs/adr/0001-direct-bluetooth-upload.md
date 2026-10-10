# Upload directly over Bluetooth by replaying the app's update procedure

The only proven way to install a modified voice pack is through the Sony phone app, with a mitmproxy
CDN swap and a CA certificate trusted on the phone. We instead upload from the PC straight to the
headphones, replaying what the Sound Connect app does when it installs a voice-guidance language.
This keeps the workflow on one machine without the app, a proxy or a trusted CA.

Decompiling Sound Connect 12.4.2 showed what that procedure is (`docs/upload-protocol.md`). It runs
over Bluetooth Classic RFCOMM, not BLE: the Airoha update library refuses BLE updates for the
WH-1000XM4's chip. Two channels are used. Sony's MDR control channel switches the headphones'
"language update mode" on and off. The Airoha RACE channel carries the update itself: FOTA start and
queries, erase in 4 KB blocks, program in 256-byte pages (all-0xFF pages skipped), an integrity
check, and a commit. The app writes the pack file unchanged into the FOTA partition the headphones
report. It never writes the voice partition. After the commit, the headphones check, unpack and
install the pack themselves.

Firmware cannot be downgraded, and stray RACE flash writes are known to brick devices. So the
upload sends only the commands the app sends, in the app's order. It refuses any erase or write
outside the FOTA partition the headphones report, or beyond the pack's size. It only accepts a pack
that has passed a full unpack-and-check round trip.
