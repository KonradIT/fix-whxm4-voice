# Upload directly over Bluetooth by replaying the app's FOTA procedure

The only proven way to install a modified voice pack is through the Sony phone app, with a mitmproxy
CDN swap and a CA certificate trusted on the phone. We instead upload from the PC straight to the
headphones over BLE, replaying the Airoha FOTA sequence that the app itself uses for a language
switch (erase, 4 KB chunk writes, on-device SHA-256 check). This keeps the workflow on one machine
without a proxy or a trusted CA. The cost is that the write protocol must be reverse-engineered: it
comes from decompiling the Android Sound Connect app, cross-checked against race-toolkit. No phone
capture is available, and the procedure has no confirmed open-source implementation for the
WH-1000XM4.

Because firmware cannot be downgraded and stray RACE flash writes are known to brick devices, the
upload refuses any erase or write outside the voice partition, writes only the steps the app
performs, and requires a pack that has passed a full unpack-and-check round trip. The app-plus-proxy
route is kept as a fallback only if this reverse engineering fails.
