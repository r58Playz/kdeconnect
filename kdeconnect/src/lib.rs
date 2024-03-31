#![feature(once_cell_try, let_chains, duration_constructors)]
pub mod config;
pub mod device;
pub mod packets;
mod util;

use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use config::ConfigProvider;
use device::Device;
use log::{debug, info};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use packets::{DeviceType, Identity, Packet, PacketType};
use rcgen::KeyPair;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, UdpSocket},
    select,
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
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

#[derive(Error, Debug)]
pub enum KdeConnectError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Mdns(#[from] mdns_sd::Error),
    #[error(transparent)]
    Rcgen(#[from] rcgen::Error),
    #[error(transparent)]
    Rustls(#[from] tokio_rustls::rustls::Error),
    #[error(transparent)]
    InvalidDnsName(#[from] tokio_rustls::rustls::pki_types::InvalidDnsNameError),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error("Channel send error")]
    ChannelSendError,
    #[error("No peer certificates")]
    NoPeerCerts,
}

impl<T> From<mpsc::error::SendError<T>> for KdeConnectError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelSendError
    }
}

type Result<T> = std::result::Result<T, KdeConnectError>;

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

    connected_clients: Arc<Mutex<Vec<String>>>,

    new_device_tx: UnboundedSender<Device>,
}

impl KdeConnect {
    pub async fn new(
        device_id: String,
        device_name: String,
        device_type: DeviceType,
        config: Arc<dyn ConfigProvider + Sync + Send>,
    ) -> Result<(Self, KdeConnectClient)> {
        let udp_socket =
            UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, KDECONNECT_PORT)).await?;
        udp_socket.set_broadcast(true)?;
        let mdns = ServiceDaemon::new()?;

        let keypair = match config
            .retrieve_server_keypair()
            .await
            .and_then(|x| KeyPair::try_from(x).map_err(|x| x.into()))
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

        // FIXME Verify certs based on common name, not SNI
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

        let (new_device_tx, new_device_rx) = mpsc::unbounded_channel();

        info!(
            "initialized kde connect device id: {:?} name: {:?} type: {:?}",
            device_id, device_name, device_type
        );

        Ok((
            Self {
                device_id,
                device_name,
                device_type,

                udp_socket,
                mdns,

                config,
                server_tls_config,
                client_tls_config,

                connected_clients: Arc::new(Mutex::new(Vec::new())),

                new_device_tx,
            },
            KdeConnectClient { new_device_rx },
        ))
    }

    pub async fn start_server(&self) -> Result<()> {
        let fullname = self.publish_mdns().await?;
        info!("published mdns service");
        let ret = select! {
            x = self.listen_for_identity() => x,
            x = self.listen_on_tcp() => x,
            x = self.send_identity() => x,
            x = self.discover_mdns() => x,
        };
        self.mdns.unregister(&fullname)?;
        info!("unpublished mdns service");
        ret
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

    async fn listen_on_tcp(&self) -> Result<()> {
        let tcp_listener =
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, KDECONNECT_PORT)).await?;
        info!("listening on tcp");
        while let Ok((stream, _)) = tcp_listener.accept().await {
            let mut stream = BufReader::new(stream);
            let mut identity = String::new();
            stream.read_line(&mut identity).await?;
            let packet: Packet = json::from_str(&identity)?;
            let identity: Identity = json::from_value(packet.body)?;

            if self
                .connected_clients
                .lock()
                .await
                .contains(&identity.device_id)
            {
                debug!("ignoring reconnect from client {:?}", identity.device_id);
                continue;
            }

            let device_config = self
                .config
                .retrieve_device_config(&identity.device_id)
                .await
                .ok();

            self.connected_clients
                .lock()
                .await
                .push(identity.device_id.clone());

            // dummy dns name, it doesn't get checked anyway
            let stream = TlsConnector::from(self.client_tls_config.clone())
                .connect("r58playz.dev".try_into()?, stream)
                .await?;

            info!("new device via tcp: {:#?}", identity);

            let device = Device::new(
                identity,
                device_config,
                self.config.clone(),
                stream.into(),
                self.connected_clients.clone(),
            )
            .await;

            self.new_device_tx.send(device)?;
        }
        Ok(())
    }

    async fn listen_for_identity(&self) -> Result<()> {
        info!("listening on udp");
        loop {
            let mut buf = vec![0u8; 8192];
            let (len, mut addr) = self.udp_socket.recv_from(&mut buf).await?;
            let packet: Packet = json::from_slice(&buf[..len])?;
            if let Ok(identity) = json::from_value::<Identity>(packet.body)
                && identity.device_id != self.device_id
                && let Some(tcp_port) = identity.tcp_port
            {
                if self
                    .connected_clients
                    .lock()
                    .await
                    .contains(&identity.device_id)
                {
                    debug!("ignoring reconnect from client {:?}", identity.device_id);
                    continue;
                }
                addr.set_port(tcp_port);

                self.connected_clients
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

                info!("new device via udp: {:#?}", identity);

                let device = Device::new(
                    identity,
                    device_config,
                    self.config.clone(),
                    stream.into(),
                    self.connected_clients.clone(),
                )
                .await;

                self.new_device_tx.send(device)?;
            }
        }
    }

    async fn send_identity(&self) -> Result<()> {
        info!("broadcasting on udp");
        loop {
            self.udp_socket
                .send_to(
                    &json::to_vec(&self.make_identity(Some(KDECONNECT_PORT)))?,
                    SocketAddrV4::new(Ipv4Addr::BROADCAST, KDECONNECT_PORT),
                )
                .await?;
            debug!("broadcasted identity over udp");
            sleep(Duration::from_mins(1)).await;
        }
    }

    async fn publish_mdns(&self) -> Result<String> {
        let mut props = HashMap::new();
        props.insert("id".to_string(), self.device_id.clone());
        props.insert("name".to_string(), self.device_name.clone());
        props.insert("type".to_string(), self.device_type.to_string());
        props.insert("protocol".to_string(), PROTOCOL_VERSION.to_string());
        let conf = ServiceInfo::new(
            "_kdeconnect._udp.local.",
            &self.device_id,
            &self.device_id,
            (),
            KDECONNECT_PORT,
            props,
        )?
        .enable_addr_auto();
        let fullname = conf.get_fullname().to_string();
        self.mdns.register(conf)?;
        Ok(fullname)
    }

    async fn discover_mdns(&self) -> Result<()> {
        let browser = self.mdns.browse("_kdeconnect._udp.local.")?;
        while let Ok(service) = browser.recv_async().await {
            if let ServiceEvent::ServiceResolved(info) = service
                && let Some(id) = info.get_property_val_str("id")
                && id != self.device_id
            {
                info!("resolved kde connect: {:#?}", info);
                // TODO should this also be hooked into the new device discovery?
            }
        }
        Ok(())
    }
}

pub struct KdeConnectClient {
    new_device_rx: UnboundedReceiver<Device>,
}

impl KdeConnectClient {
    pub async fn discover_devices(&mut self) -> Option<Device> {
        self.new_device_rx.recv().await
    }
}
