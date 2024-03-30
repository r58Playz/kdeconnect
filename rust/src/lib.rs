#![feature(once_cell_try, let_chains, duration_constructors)]
mod config;
mod device;
mod ffi;
mod packets;
mod util;

use std::{
    collections::HashMap,
    error::Error,
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use config::ConfigProvider;
use device::Device;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use packets::{DeviceType, Identity, Packet, PacketType};
use rcgen::KeyPair;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, UdpSocket},
    select,
    sync::Mutex,
    time::sleep,
};

use serde_json as json;
use tokio_rustls::{
    rustls::{
        crypto::ring::default_provider,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
        ClientConfig, ServerConfig,
    },
    TlsAcceptor, TlsConnector,
};

use crate::{
    packets::{Ping, PROTOCOL_VERSION},
    util::NoCertificateVerification,
};

const KDECONNECT_PORT: u16 = 1716;

pub struct KdeConnect {
    pub device_type: DeviceType,
    pub device_name: String,
    pub device_id: String,
    udp_socket: UdpSocket,
    mdns: ServiceDaemon,
    server_tls_config: Arc<ServerConfig>,
    client_tls_config: Arc<ClientConfig>,
    config: Arc<dyn ConfigProvider + Sync + Send>,
}

impl KdeConnect {
    pub async fn new(
        device_id: String,
        device_name: String,
        device_type: DeviceType,
        config: Arc<dyn ConfigProvider + Sync + Send>,
    ) -> Result<Self, Box<dyn Error>> {
        let udp_socket =
            UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, KDECONNECT_PORT)).await?;
        udp_socket.set_broadcast(true)?;
        let mdns = ServiceDaemon::new()?;

        let keypair = match config
            .retrieve_server_keypair()
            .await
            .and_then(|x| KeyPair::try_from(x).map_err(io::Error::other))
        {
            Ok(pair) => pair,
            Err(_) => {
                let pair = KeyPair::generate()?;
                config.store_server_keypair(&pair.serialize_der()).await?;
                pair
            }
        };

        let cert = match config.retrieve_server_cert().await {
            Ok(cert) => CertificateDer::from(cert),
            Err(_) => {
                let cert = util::generate_server_cert(&keypair, &device_id)?;
                config.store_server_cert(cert.der()).await?;
                CertificateDer::from(cert)
            }
        };

        let server_tls_config = Arc::new(
            ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![cert.clone()],
                    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(keypair.serialize_der())),
                )?,
        );
        let client_tls_config = Arc::new(
            ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoCertificateVerification::new(
                    default_provider(),
                )))
                .with_client_auth_cert(
                    vec![cert.clone()],
                    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(keypair.serialize_der())),
                )?,
        );

        Ok(Self {
            device_id,
            device_name,
            device_type,
            udp_socket,
            mdns,
            config,
            server_tls_config,
            client_tls_config,
        })
    }

    fn make_identity(&self, tcp_port: Option<u16>) -> Packet {
        let ident = Identity {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            device_type: self.device_type,
            protocol_version: PROTOCOL_VERSION,
            incoming_capabilities: vec![Ping::TYPE.to_string()],
            outgoing_capabilities: vec![Ping::TYPE.to_string()],
            tcp_port,
        };
        make_packet!(ident)
    }

    pub async fn start_server(&self) -> Result<(), Box<dyn Error>> {
        self.publish_mdns().await?;
        select! {
            x = self.listen_for_identity() => x,
            x = self.listen_on_tcp() => x,
            x = self.send_identity() => x,
            x = self.discover_mdns() => x,
        }
    }

    async fn listen_on_tcp(&self) -> Result<(), Box<dyn Error>> {
        let tcp_listener =
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, KDECONNECT_PORT)).await?;
        let connected_clients: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        while let Ok((stream, addr)) = tcp_listener.accept().await {
            let mut stream = BufReader::new(stream);
            let mut identity = String::new();
            stream.read_line(&mut identity).await?;
            let packet: Packet = json::from_str(&identity)?;
            let identity: Identity = json::from_value(packet.body)?;
            println!("tcp packet addr {:?}: {:?} {:?}", addr, identity.device_id, identity.device_name);

            if connected_clients.lock().await.contains(&identity.device_id) {
                println!("ignoring reconnect");
                continue;
            }

            let device_config = self
                .config
                .retrieve_device_config(&identity.device_id)
                .await
                .ok();

            connected_clients
                .lock()
                .await
                .push(identity.device_id.clone());

            // dummy dns name, it doesn't get checked anyway
            let stream = TlsConnector::from(self.client_tls_config.clone())
                .connect("r58playz.dev".try_into()?, stream)
                .await?;

            let dev_id = identity.device_id.clone();
            let device = Device::new(identity, device_config, self.config.clone()).await;

            let connected_clients = connected_clients.clone();
            tokio::spawn(async move {
                let ret = device.task(stream.into()).await;
                connected_clients.lock().await.retain(|x| *x != dev_id);
                ret
            });
        }
        Ok(())
    }

    async fn listen_for_identity(&self) -> Result<(), Box<dyn Error>> {
        let connected_clients: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        loop {
            let mut buf = vec![0u8; 8192];
            let (len, mut addr) = self.udp_socket.recv_from(&mut buf).await?;
            let packet: Packet = json::from_slice(&buf[..len])?;
            if let Ok(identity) = json::from_value::<Identity>(packet.body)
                && identity.device_id != self.device_id
                && !connected_clients.lock().await.contains(&identity.device_id)
                && let Some(tcp_port) = identity.tcp_port
            {
                println!("udp packet addr {:?}: {:?} {:?}", addr, identity.device_id, identity.device_name);
                addr.set_port(tcp_port);

                connected_clients
                    .lock()
                    .await
                    .push(identity.device_id.clone());

                let mut stream = BufReader::new(TcpStream::connect(addr).await?);
                let own_identity = json::to_string(&self.make_identity(None))? + "\n";
                stream.write_all(own_identity.as_bytes()).await?;

                let stream = TlsAcceptor::from(self.server_tls_config.clone())
                    .accept(stream)
                    .await?;

                let device_config = self
                    .config
                    .retrieve_device_config(&identity.device_id)
                    .await
                    .ok();

                let dev_id = identity.device_id.clone();
                let device = Device::new(identity, device_config, self.config.clone()).await;

                let connected_clients = connected_clients.clone();
                tokio::spawn(async move {
                    let ret = device.task(stream.into()).await;
                    connected_clients.lock().await.retain(|x| *x != dev_id);
                    ret
                });
            }
        }
    }

    async fn send_identity(&self) -> Result<(), Box<dyn Error>> {
        loop {
            self.udp_socket
                .send_to(
                    &json::to_vec(&self.make_identity(Some(KDECONNECT_PORT)))?,
                    SocketAddrV4::new(Ipv4Addr::BROADCAST, KDECONNECT_PORT),
                )
                .await?;
            sleep(Duration::from_mins(1)).await;
        }
    }

    async fn publish_mdns(&self) -> Result<(), Box<dyn Error>> {
        let mut props = HashMap::new();
        props.insert("id".to_string(), self.device_id.clone());
        props.insert("name".to_string(), self.device_name.clone());
        props.insert("type".to_string(), self.device_type.to_string());
        props.insert("protocol".to_string(), PROTOCOL_VERSION.to_string());

        Ok(self.mdns.register(
            ServiceInfo::new(
                "_kdeconnect._udp.local.",
                &self.device_id,
                &self.device_id,
                (),
                KDECONNECT_PORT,
                props,
            )?
            .enable_addr_auto(),
        )?)
    }

    async fn discover_mdns(&self) -> Result<(), Box<dyn Error>> {
        let browser = self.mdns.browse("_kdeconnect._udp.local.")?;
        while let Ok(service) = browser.recv_async().await {
            if let ServiceEvent::ServiceResolved(info) = service
                && let Some(id) = info.get_property_val_str("id")
                && id != self.device_id
            {
                println!("resolved kde connect: {:#?}", info);
            }
        }
        Ok(())
    }
}
