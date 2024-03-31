use std::{error::Error, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json as json;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    select,
    sync::Mutex,
};
use tokio_rustls::TlsStream;

use crate::{
    config::ConfigProvider,
    make_packet, make_packet_str,
    packets::{DeviceType, Identity, Packet, PacketType, Pair, Ping},
};

pub struct Device {
    pub config: DeviceConfig,
    provider: Arc<dyn ConfigProvider + Sync + Send>,
    stream: BufReader<TlsStream<BufReader<TcpStream>>>,
    connected_clients: Arc<Mutex<Vec<String>>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeviceConfig {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub certificate: Option<Vec<u8>>,
}

impl Device {
    // basically whenever tcp connection is established identity packet gets sent
    // then tls starts, only if device is trusted does cert get verified
    // once in tls untrusted devices can be trusted by sending pair and then storing
    // device's cert to verify
    pub(crate) async fn new(
        identity: Identity,
        conf: Option<DeviceConfig>,
        provider: Arc<dyn ConfigProvider + Sync + Send>,
        stream: TlsStream<BufReader<TcpStream>>,
        connected_clients: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        let cert = conf.and_then(|x| x.certificate);
        Self {
            provider,
            config: DeviceConfig {
                id: identity.device_id,
                name: identity.device_name,
                device_type: identity.device_type,
                certificate: cert,
            },
            stream: BufReader::new(stream),
            connected_clients,
        }
    }

    pub async fn task(
        &mut self,
        handler: Box<dyn DeviceHandler + Sync + Send>,
    ) -> Result<(), Box<dyn Error + Sync + Send>> {
        let ret = select! {
            x = self.tls_task(handler) => x
        };
        self.connected_clients
            .lock()
            .await
            .retain(|x| *x != self.config.id);
        ret
    }

    async fn tls_task(
        &mut self,
        mut handler: Box<dyn DeviceHandler + Sync + Send>,
    ) -> Result<(), Box<dyn Error + Sync + Send>> {
        loop {
            let mut buf = String::new();
            self.stream.read_line(&mut buf).await?;
            let packet: Packet = json::from_str(&buf)?;
            match packet.packet_type.as_str() {
                Ping::TYPE => {
                    let body: Ping = json::from_value(packet.body)?;
                    self.stream
                        .write_all(make_packet_str!(body)?.as_bytes())
                        .await?;
                }
                Pair::TYPE => {
                    let body: Pair = json::from_value(packet.body)?;
                    if self.config.certificate.is_some() && !body.pair {
                        // already paired and asking to unpair?
                        self.config.certificate.take();
                        let pair_packet = Pair { pair: false };
                        println!("unpairing");
                        self.stream
                            .write_all(make_packet_str!(pair_packet)?.as_bytes())
                            .await?;
                    } else if self.config.certificate.is_none()
                        && body.pair
                        && handler.handle_pairing_request(self)
                    {
                        // unpaired and asking to pair?
                        let tls_state = self.stream.get_ref().get_ref().1;
                        // FIXME error enum
                        self.config
                            .certificate
                            .replace(tls_state.peer_certificates().unwrap()[0].to_vec());
                        let pair_packet = Pair { pair: true };
                        self.stream
                            .write_all(make_packet_str!(pair_packet)?.as_bytes())
                            .await?;
                    } else {
                        // just forward it?
                        self.stream
                            .write_all(make_packet_str!(body)?.as_bytes())
                            .await?;
                    }
                    self.provider.store_device_config(&self.config).await?;
                }
                _ => println!("unknown type: {:?} {:?}", packet.packet_type, packet.body),
            }
        }
    }
}

pub trait DeviceHandler {
    fn handle_pairing_request(&mut self, device: &Device) -> bool;
}
