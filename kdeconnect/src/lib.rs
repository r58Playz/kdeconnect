#![feature(once_cell_try, let_chains, duration_constructors)]
pub mod config;
pub mod device;
pub mod packets;
mod util;

use std::{
    collections::HashMap,
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use config::ConfigProvider;
use device::{Device, DeviceClient};
use log::{debug, error, info};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use packets::{
    DeviceType, Identity, MousepadEcho, MousepadKeyboardState, MousepadRequest, Packet, PacketType,
};
use rcgen::KeyPair;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, UdpSocket},
    select,
    sync::{mpsc, oneshot, Mutex},
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
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream};

use crate::{
    device::create_device,
    packets::{
        Battery, BatteryRequest, Clipboard, ClipboardConnect, ConnectivityReport,
        ConnectivityReportRequest, FindPhone, Mpris, MprisRequest, Ping, Presenter, ShareRequest,
        SystemVolume, SystemVolumeRequest, PROTOCOL_VERSION,
    },
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
    #[error(transparent)]
    X509(#[from] x509_parser::nom::Err<x509_parser::error::X509Error>),
    #[error("Channel send error")]
    ChannelSendError,
    #[error("Channel recieve error")]
    ChannelRecvError,
    #[error("No peer certificates")]
    NoPeerCerts,
    #[error("Server task already started")]
    ServerAlreadyStarted,
    #[error("Failed to convert OsString to str")]
    OsStringConversionError,
    #[error("Failed to find port for payload transfer")]
    NoPayloadTransferPortFound,
    #[error("No filename")]
    NoFileName,
    #[error("Other")]
    Other,

    #[error("Device rejected pair")]
    DeviceRejectedPair,
    #[error("Already paired")]
    DeviceAlreadyPaired,
}

impl<T> From<mpsc::error::SendError<T>> for KdeConnectError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelSendError
    }
}

impl From<oneshot::error::RecvError> for KdeConnectError {
    fn from(_: oneshot::error::RecvError) -> Self {
        Self::ChannelRecvError
    }
}

type Result<T> = std::result::Result<T, KdeConnectError>;

const KDECONNECT_PORT: u16 = 1716;

enum KdeConnectAction {
    BroadcastIdentity(oneshot::Sender<Result<()>>),
}

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

    new_device_tx: mpsc::UnboundedSender<(Device, DeviceClient)>,
    client_rx: Mutex<mpsc::UnboundedReceiver<KdeConnectAction>>,
}

impl KdeConnect {
    pub async fn new(
        device_id: String,
        device_name: String,
        device_type: DeviceType,
        config: Arc<dyn ConfigProvider + Sync + Send>,
    ) -> Result<(
        Self,
        KdeConnectClient,
        impl Stream<Item = (Device, DeviceClient)>,
    )> {
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

        let verifier = Arc::new(NoCertificateVerification::new(default_provider()));

        // FIXME Verify certs
        let server_tls_config = Arc::new(
            ServerConfig::builder()
                .with_client_cert_verifier(verifier.clone())
                .with_single_cert(
                    vec![cert.clone()],
                    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(keypair.serialize_der())),
                )?,
        );

        // FIXME Verify certs
        let client_tls_config = Arc::new(
            ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(verifier.clone())
                .with_client_auth_cert(
                    vec![cert.clone()],
                    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(keypair.serialize_der())),
                )?,
        );

        let (new_device_tx, new_device_rx) = mpsc::unbounded_channel();
        let (client_tx, client_rx) = mpsc::unbounded_channel();

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
                client_rx: Mutex::new(client_rx),
            },
            KdeConnectClient { client_tx },
            UnboundedReceiverStream::new(new_device_rx),
        ))
    }

    fn make_identity(&self, tcp_port: Option<u16>) -> Packet {
        let ident = Identity {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            device_type: self.device_type,
            protocol_version: PROTOCOL_VERSION,
            incoming_capabilities: vec![
                Ping::TYPE.to_string(),
                Battery::TYPE.to_string(),
                BatteryRequest::TYPE.to_string(),
                Clipboard::TYPE.to_string(),
                ClipboardConnect::TYPE.to_string(),
                FindPhone::TYPE.to_string(),
                ConnectivityReport::TYPE.to_string(),
                ConnectivityReportRequest::TYPE.to_string(),
                Presenter::TYPE.to_string(),
                SystemVolume::TYPE.to_string(),
                SystemVolumeRequest::TYPE.to_string(),
                ShareRequest::TYPE.to_string(),
                Mpris::TYPE.to_string(),
                MprisRequest::TYPE.to_string(),
                MousepadRequest::TYPE.to_string(),
                MousepadEcho::TYPE.to_string(),
                MousepadKeyboardState::TYPE.to_string(),
            ],
            outgoing_capabilities: vec![
                Ping::TYPE.to_string(),
                Battery::TYPE.to_string(),
                BatteryRequest::TYPE.to_string(),
                Clipboard::TYPE.to_string(),
                ClipboardConnect::TYPE.to_string(),
                FindPhone::TYPE.to_string(),
                ConnectivityReport::TYPE.to_string(),
                ConnectivityReportRequest::TYPE.to_string(),
                Presenter::TYPE.to_string(),
                SystemVolume::TYPE.to_string(),
                SystemVolumeRequest::TYPE.to_string(),
                ShareRequest::TYPE.to_string(),
                Mpris::TYPE.to_string(),
                MprisRequest::TYPE.to_string(),
                MousepadRequest::TYPE.to_string(),
                MousepadEcho::TYPE.to_string(),
                MousepadKeyboardState::TYPE.to_string(),
            ],
            tcp_port,
        };
        make_packet!(ident)
    }

    pub async fn start_server(&self) -> Result<()> {
        let fullname = self.publish_mdns().await?;
        info!("published mdns service");
        let ret = select! {
            x = self.listen_on_udp() => x,
            x = self.send_on_udp() => x,
            x = self.listen_on_tcp() => x,
            x = self.discover_mdns() => x,
            _ = self.respond_to_client() => Ok(()),
        };
        self.mdns.unregister(&fullname)?;
        info!("unpublished mdns service");
        ret
    }

    async fn respond_to_client(&self) {
        while let Some(evt) = self.client_rx.lock().await.recv().await {
            use KdeConnectAction as A;
            let _ = match evt {
                A::BroadcastIdentity(respond) => respond.send(self.send_identity_once().await),
            };
        }
    }

    async fn listen_on_tcp(&self) -> Result<()> {
        let tcp_listener =
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, KDECONNECT_PORT)).await?;
        info!("listening on tcp");
        while let Ok((stream, _)) = tcp_listener.accept().await {
            let mut stream = BufReader::new(stream);
            let mut identity = String::new();
            stream.read_line(&mut identity).await?;
            if let Ok(packet) = json::from_str::<Packet>(&identity)
                && let Ok(identity) = json::from_value::<Identity>(packet.body)
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

                let dev_id = identity.device_id.clone();

                let ret = async {
                    self.connected_clients
                        .lock()
                        .await
                        .push(identity.device_id.clone());

                    // dummy dns name, it doesn't get checked anyway
                    let stream = TlsConnector::from(self.client_tls_config.clone())
                        .connect(identity.device_id.clone().try_into()?, stream)
                        .await?;

                    info!("new device via tcp: {:#?}", identity);

                    let device_tuple = create_device(
                        identity,
                        self.config.clone(),
                        stream.into(),
                        self.connected_clients.clone(),
                        self.server_tls_config.clone(),
                        self.client_tls_config.clone(),
                    )
                    .await?;

                    self.new_device_tx
                        .send(device_tuple)
                        .map_err(KdeConnectError::from)
                }
                .await;
                if let Err(err) = ret {
                    error!("error while accepting device via tcp: {:?}", err);
                    self.connected_clients.lock().await.retain(|x| *x != dev_id);
                }
            }
        }
        Ok(())
    }

    async fn listen_on_udp(&self) -> Result<()> {
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
                    debug!("ignoring reconnect to client {:?}", identity.device_id);
                    continue;
                }

                let dev_id = identity.device_id.clone();

                let ret = async {
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

                    info!("new device discovered through udp: {:#?}", identity);

                    let device_tuple = create_device(
                        identity,
                        self.config.clone(),
                        stream.into(),
                        self.connected_clients.clone(),
                        self.server_tls_config.clone(),
                        self.client_tls_config.clone(),
                    )
                    .await?;

                    self.new_device_tx
                        .send(device_tuple)
                        .map_err(KdeConnectError::from)
                }
                .await;
                if let Err(err) = ret {
                    error!(
                        "error while connecting to device discovered through udp: {:?}",
                        err
                    );
                    self.connected_clients.lock().await.retain(|x| *x != dev_id);
                }
            }
        }
    }

    async fn send_identity_once(&self) -> Result<()> {
        self.udp_socket
            .send_to(
                &json::to_vec(&self.make_identity(Some(KDECONNECT_PORT)))?,
                SocketAddrV4::new(Ipv4Addr::BROADCAST, KDECONNECT_PORT),
            )
            .await?;
        debug!("broadcasted identity over udp");
        Ok(())
    }

    async fn send_on_udp(&self) -> Result<()> {
        info!("broadcasting on udp");
        // wait until everything else is ready
        sleep(Duration::from_secs(1)).await;
        loop {
            self.send_identity_once().await?;
            sleep(Duration::from_secs(30)).await;
        }
    }

    async fn publish_mdns(&self) -> Result<String> {
        let mut props = HashMap::new();
        props.insert("id".to_string(), self.device_id.clone());
        props.insert("name".to_string(), self.device_name.clone());
        props.insert("type".to_string(), self.device_type.to_string());
        props.insert("protocol".to_string(), PROTOCOL_VERSION.to_string());
        // local_ip_addr correctly pulls in the ip address for ios
        let conf = ServiceInfo::new(
            "_kdeconnect._udp.local.",
            &self.device_id,
            &self.device_id,
            local_ip_addr::get_local_ip_address()
                .map_or(vec![], |x| vec![x])
                .as_slice(),
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
                && let Some(addr) = info.get_addresses().iter().next()
            {
                info!(
                    "new device discovered through mdns, sending identity: {:?}",
                    info.get_fullname()
                );
                let addr = SocketAddr::new(*addr, info.get_port());
                let bind_addr = if addr.is_ipv4() {
                    SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0)
                } else {
                    SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0)
                };

                let ret = async {
                    UdpSocket::bind(bind_addr)
                        .await?
                        .send_to(
                            &json::to_vec(&self.make_identity(Some(KDECONNECT_PORT)))?,
                            addr,
                        )
                        .await
                }
                .await;
                if let Err(err) = ret {
                    error!("error while sending identity to mdns device: {:?}", err);
                } else {
                    info!("sent identity to mdns device: {:?}", info.get_fullname());
                }
            }
        }
        Ok(())
    }
}

pub struct KdeConnectClient {
    client_tx: mpsc::UnboundedSender<KdeConnectAction>,
}

impl KdeConnectClient {
    pub async fn broadcast_identity(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.client_tx
            .send(KdeConnectAction::BroadcastIdentity(tx))?;
        rx.await?
    }
}
