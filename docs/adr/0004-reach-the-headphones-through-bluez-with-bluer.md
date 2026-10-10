# Reach the headphones through BlueZ with bluer, letting BlueZ find the channels

The tool talks to the headphones over two RFCOMM channels, Sony's MDR channel (`Serial HPC`) and
Airoha's RACE channel (`Airoha_APP`). Their channel numbers must be looked up by SDP UUID rather
than hard-coded. We use `bluer`, the official Rust bindings for BlueZ's D-Bus API. For each channel
we register a client profile with the channel's UUID, and ask BlueZ to connect that profile on the
headphones. BlueZ does the SDP lookup, connects, and hands over the socket. The headphones are the
connected device whose cached UUIDs include the MDR channel.

This costs tokio and libdbus (and so `pkg-config` and `dbus` in the flake), and makes the
Bluetooth code async behind a synchronous interface. It gives us device names, connection state and
plain errors when BlueZ is not running, with no SDP client of our own. The alternatives were raw
`AF_BLUETOOTH` sockets with a hand-written SDP client, which gives no device names and no way to tell
whether BlueZ runs, and shelling out to `bluetoothctl` and `sdptool`, which is deprecated and
missing on many distributions.

The protocols sit behind a `Transport` (bytes) and a `Link` (send a command, await its reply), so
everything above the socket is tested with fakes.
