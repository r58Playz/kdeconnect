use std::{error::Error, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json as json;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpStream,
};
use tokio_rustls::TlsStream;

use crate::{
    config::ConfigProvider,
    packets::{DeviceType, Identity, Packet, Pair},
};

pub struct Device {
    provider: Arc<dyn ConfigProvider + Sync + Send>,
    config: DeviceConfig,
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
        }
    }

    pub(crate) async fn task(
        &self,
        stream: TlsStream<BufReader<TcpStream>>,
    ) -> Result<(), Box<dyn Error + Sync + Send>> {
        let mut stream = BufReader::new(stream);
        loop {
            let mut buf = String::new();
            stream.read_line(&mut buf).await?;
            let packet: Packet = json::from_str(&buf)?;
            match packet.packet_type.as_str() {
                Pair::TYPE => {
                    let body: Pair = json::from_value(packet.body)?;
                    println!("pairing request recieved: {:?}", body);
                }
                _ => println!("unknown type: {:?} {:?}", packet.packet_type, packet.body),
            }
        }
    }
}
