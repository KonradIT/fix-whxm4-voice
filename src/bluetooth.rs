//! The real device link: RFCOMM channels to the headphones through BlueZ.
//!
//! BlueZ finds each channel by its SDP UUID: we register a client profile for the UUID and ask BlueZ
//! to connect it, and BlueZ hands over the connected socket. See ADR 0004.

use std::io;
use std::time::Duration;

use bluer::rfcomm::{Profile, ProfileHandle, Role, Stream};
use bluer::{Device, Session, Uuid};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::{Handle, Runtime};
use tokio::time::timeout;

use crate::link::{Transport, hex};

/// Sony's MDR control channel, `Serial HPC` in SDP.
pub const MDR_UUID: Uuid = Uuid::from_u128(0x96cc203e_5068_46ad_b32d_e316f5e069ba);
/// Airoha's RACE channel, `Airoha_APP` in SDP.
pub const RACE_UUID: Uuid = Uuid::from_u128(0x8901dfa8_5c7e_4d8f_9f0c_c2b70683f5f0);

/// How long BlueZ may take to open a channel.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Connected headphones and the BlueZ session they are reached through.
pub struct Headphones {
    pub name: String,
    pub address: String,
    device: Device,
    session: Session,
    // Dropped last: everything above needs the runtime to shut down cleanly.
    runtime: Runtime,
}

impl Headphones {
    /// Finds the headphones among the devices connected to the default adapter: the one that
    /// offers Sony's MDR channel.
    pub fn find() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        let (session, device, name) = runtime.block_on(find())?;
        Ok(Headphones {
            name,
            address: device.address().to_string(),
            device,
            session,
            runtime,
        })
    }

    /// Opens the channel with SDP UUID `uuid`, `label` naming it in errors and logs. With
    /// `verbose`, every byte sent and received is logged to stderr.
    pub fn open(&self, uuid: Uuid, label: &'static str, verbose: bool) -> io::Result<Channel> {
        let (profile, stream) = self
            .runtime
            .block_on(open(&self.session, &self.device, uuid))
            .map_err(|e| {
                // BlueZ reports this when the link is busy, which is usually audio playing.
                let hint = if e.to_string().contains("br-connection-busy") {
                    "\nThis often means the headphones are, or just were, playing audio. If audio is playing, \
                     stop it, wait a few seconds and try again; if not, try again right away."
                } else {
                    ""
                };
                io::Error::new(
                    e.kind(),
                    format!("cannot open the {label} channel ({uuid}): {e}{hint}"),
                )
            })?;
        Ok(Channel {
            label,
            verbose,
            stream,
            runtime: self.runtime.handle().clone(),
            _profile: profile,
        })
    }
}

async fn find() -> io::Result<(Session, Device, String)> {
    let session = Session::new()
        .await
        .map_err(|e| io::Error::other(format!("cannot reach D-Bus, which BlueZ needs: {e}")))?;
    // Without bluetoothd, D-Bus has nobody to ask for the adapter, so both causes end up here.
    let adapter = session.default_adapter().await.map_err(|e| {
        io::Error::other(format!(
            "no Bluetooth adapter found, or BlueZ is not running: {e}\n\
             If BlueZ is not running, start the bluetooth service."
        ))
    })?;
    if !adapter.is_powered().await.map_err(io::Error::other)? {
        return Err(io::Error::other(format!(
            "Bluetooth adapter {} is off\nSwitch it on, e.g. with `bluetoothctl power on`.",
            adapter.name()
        )));
    }

    let mut found = Vec::new();
    for address in adapter.device_addresses().await.map_err(io::Error::other)? {
        let device = adapter.device(address).map_err(io::Error::other)?;
        let connected = device.is_connected().await.unwrap_or(false);
        let uuids = device.uuids().await.ok().flatten().unwrap_or_default();
        if connected && uuids.contains(&MDR_UUID) {
            let name = device.name().await.ok().flatten().unwrap_or_default();
            found.push((device, name));
        }
    }
    match found.len() {
        0 => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no headphones found\n\
             Connect the WH-1000XM4 to this PC first.",
        )),
        1 => {
            let (device, name) = found.pop().unwrap();
            Ok((session, device, name))
        }
        _ => {
            let names: Vec<String> = found
                .iter()
                .map(|(d, name)| format!("\n  {name} ({})", d.address()))
                .collect();
            Err(io::Error::other(format!(
                "several connected devices offer Sony's control channel:{}\n\
                 Disconnect all but the WH-1000XM4.",
                names.concat()
            )))
        }
    }
}

async fn open(
    session: &Session,
    device: &Device,
    uuid: Uuid,
) -> io::Result<(ProfileHandle, Stream)> {
    let mut profile = session
        .register_profile(Profile {
            uuid,
            role: Some(Role::Client),
            require_authentication: Some(false),
            require_authorization: Some(false),
            auto_connect: Some(false),
            ..Default::default()
        })
        .await
        .map_err(io::Error::other)?;

    // BlueZ looks the channel up by SDP and connects; the call returns once we accept the socket.
    let mut connecting = tokio::spawn({
        let device = device.clone();
        async move { device.connect_profile(&uuid).await }
    });
    let outcome = tokio::select! {
        request = profile.next() => request.ok_or_else(|| io::Error::other("BlueZ dropped the profile")),
        result = &mut connecting => Err(io::Error::other(match result {
            Ok(Ok(())) => "BlueZ connected without handing the channel over".to_string(),
            Ok(Err(e)) => e.to_string(),
            Err(e) => e.to_string(),
        })),
        _ = tokio::time::sleep(CONNECT_TIMEOUT) => {
            Err(io::Error::new(io::ErrorKind::TimedOut, "BlueZ did not connect it in time"))
        }
    };
    let request = outcome.inspect_err(|_| connecting.abort())?;
    let stream = request.accept().map_err(io::Error::other)?;
    connecting
        .await
        .map_err(io::Error::other)?
        .map_err(io::Error::other)?;
    Ok((profile, stream))
}

/// One RFCOMM channel to the headphones.
pub struct Channel {
    label: &'static str,
    verbose: bool,
    stream: Stream,
    runtime: Handle,
    // Unregistering the profile would make BlueZ close the channel.
    _profile: ProfileHandle,
}

impl Transport for Channel {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.verbose {
            eprintln!("{} > {}", self.label, hex(bytes));
        }
        self.runtime.block_on(self.stream.write_all(bytes))
    }

    fn receive(&mut self, wait: Duration) -> io::Result<Vec<u8>> {
        let mut buffer = vec![0; 1024];
        let read = self
            .runtime
            .block_on(async { timeout(wait, self.stream.read(&mut buffer)).await });
        let n = read.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "timed out"))??;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("the headphones closed the {} channel", self.label),
            ));
        }
        buffer.truncate(n);
        if self.verbose {
            eprintln!("{} < {}", self.label, hex(&buffer));
        }
        Ok(buffer)
    }
}
