use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use event_listener::Event;
use log::{debug, error};
use serde::{Deserialize, Serialize};
use serde_json as json;
use tokio::{
    io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader, Lines, ReadHalf, WriteHalf},
    net::TcpStream,
    select,
    sync::{mpsc, oneshot, Mutex},
    time::timeout,
};
use tokio_rustls::TlsStream;

use crate::{
    config::ConfigProvider,
    make_packet, make_packet_str,
    packets::{
        Battery, Clipboard, ClipboardConnect, DeviceType, FindPhone, Identity, Packet, PacketType,
        Pair, Ping,
    },
    util::get_time_ms,
    KdeConnectError, Result,
};

#[derive(Clone)]
struct LockedDeviceWrite(Arc<Mutex<WriteHalf<TlsStream<BufReader<TcpStream>>>>>);

impl LockedDeviceWrite {
    fn new(stream: WriteHalf<TlsStream<BufReader<TcpStream>>>) -> Self {
        Self(Arc::new(Mutex::new(stream)))
    }

    async fn send(&self, packet: String) -> std::io::Result<()> {
        self.0.lock().await.write_all(packet.as_bytes()).await
    }
}

pub async fn create_device(
    identity: Identity,
    config_provider: Arc<dyn ConfigProvider + Sync + Send>,
    stream: TlsStream<BufReader<TcpStream>>,
    connected_clients: Arc<Mutex<Vec<String>>>,
) -> Result<(Device, DeviceClient)> {
    let device_config = config_provider
        .retrieve_device_config(&identity.device_id)
        .await
        .ok();

    let (client_tx, client_rx) = mpsc::unbounded_channel();

    let initiated_pair = Arc::new(AtomicBool::new(false));
    let pair_event = Arc::new(Event::new());

    Ok((
        Device::new(
            identity,
            device_config,
            config_provider,
            stream,
            connected_clients,
            client_rx,
            initiated_pair.clone(),
            pair_event.clone(),
        )
        .await?,
        DeviceClient::new(client_tx, initiated_pair, pair_event),
    ))
}

pub struct Device {
    pub config: DeviceConfig,
    config_provider: Arc<dyn ConfigProvider + Sync + Send>,
    connected_clients: Arc<Mutex<Vec<String>>>,

    stream_r: Lines<BufReader<ReadHalf<TlsStream<BufReader<TcpStream>>>>>,
    stream_w: LockedDeviceWrite,
    stream_cert: Vec<u8>,

    client_r: mpsc::UnboundedReceiver<DeviceAction>,

    initiated_pair: Arc<AtomicBool>,
    pair_event: Arc<Event>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceConfig {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub certificate: Option<Vec<u8>>,
}

pub(crate) enum DeviceAction {
    SendPacket(String, oneshot::Sender<Result<()>>),
    GetConfig(oneshot::Sender<DeviceConfig>),
    GetPaired(oneshot::Sender<bool>),
}

enum DeviceEvent {
    Stream(String),
    Client(DeviceAction),
}

impl Device {
    // basically whenever tcp connection is established identity packet gets sent
    // then tls starts, only if device is trusted does cert get verified
    // once in tls untrusted devices can be trusted by sending pair and then storing
    // device's cert to verify
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
        identity: Identity,
        conf: Option<DeviceConfig>,
        config_provider: Arc<dyn ConfigProvider + Sync + Send>,
        stream: TlsStream<BufReader<TcpStream>>,
        connected_clients: Arc<Mutex<Vec<String>>>,
        client_r: mpsc::UnboundedReceiver<DeviceAction>,
        initiated_pair: Arc<AtomicBool>,
        pair_event: Arc<Event>,
    ) -> Result<Self> {
        let cert = conf.and_then(|x| x.certificate);

        let stream_cert = stream
            .get_ref()
            .1
            .peer_certificates()
            .ok_or(KdeConnectError::NoPeerCerts)?[0]
            .to_vec();

        let (r, w) = split(stream);

        Ok(Self {
            config: DeviceConfig {
                id: identity.device_id,
                name: identity.device_name,
                device_type: identity.device_type,
                certificate: cert,
            },

            config_provider,
            connected_clients,

            stream_r: BufReader::new(r).lines(),
            stream_w: LockedDeviceWrite::new(w),
            stream_cert,

            client_r,

            initiated_pair,
            pair_event,
        })
    }

    pub async fn task(&mut self, mut handler: Box<dyn DeviceHandler + Sync + Send>) -> Result<()> {
        self.send_paired_data(&mut handler).await?;
        let ret = self.inner_task(&mut handler).await;
        handler.handle_exit().await;
        self.connected_clients
            .lock()
            .await
            .retain(|x| *x != self.config.id);
        ret
    }

    async fn send_paired_data(
        &self,
        handler: &mut Box<dyn DeviceHandler + Sync + Send>,
    ) -> Result<()> {
        if self.config.certificate.is_some() {
            let battery = handler.get_battery().await;
            self.stream_w.send(make_packet_str!(battery)?).await?;

            let clipboard = ClipboardConnect {
                content: handler.get_clipboard_content().await,
                // TODO: Do we want to ask handler for clipboard last updated? timestamp seems to
                // be last updated according to:
                // https://invent.kde.org/network/kdeconnect-android/-/blob/master/src/org/kde/kdeconnect/Plugins/ClibpoardPlugin/ClipboardPlugin.java?ref_type=heads#L78
                timestamp: get_time_ms(),
            };
            self.stream_w.send(make_packet_str!(clipboard)?).await?;
        }
        Ok(())
    }

    fn is_paired(&self) -> bool {
        self.config.certificate.is_some()
    }

    async fn inner_task(
        &mut self,
        handler: &mut Box<dyn DeviceHandler + Sync + Send>,
    ) -> Result<()> {
        while let Some(evt) = select! {
            x = self.stream_r.next_line() => x?.map(DeviceEvent::Stream),
            x = self.client_r.recv() => x.map(DeviceEvent::Client),
        } {
            match evt {
                DeviceEvent::Stream(buf) => {
                    let packet: Packet = json::from_str(&buf)?;

                    match packet.packet_type.as_str() {
                        Ping::TYPE => {
                            let body: Ping = json::from_value(packet.body)?;
                            debug!("recieved ping: {:?}", body);
                            handler.handle_ping(body.clone()).await;
                            self.stream_w.send(make_packet_str!(body)?).await?;
                        }
                        Pair::TYPE => {
                            let body: Pair = json::from_value(packet.body)?;
                            if self.is_paired() && !body.pair {
                                // already paired and asking to unpair?
                                self.config.certificate.take();
                                let pair_packet = Pair { pair: false };
                                self.stream_w.send(make_packet_str!(pair_packet)?).await?;
                                debug!("unpaired from {:?}", self.config.id);
                                handler.handle_pair_status_change(false).await;
                            } else if !self.is_paired() && body.pair {
                                // unpaired and asking to pair?
                                // > By convention the request times out after 30 seconds.
                                // https://valent.andyholmes.ca/documentation/protocol.html#kdeconnectpair
                                let initiated_pair = self.initiated_pair.load(Ordering::Acquire);
                                let should_pair = initiated_pair
                                    || timeout(
                                        Duration::from_secs(30),
                                        handler.handle_pairing_request(),
                                    )
                                    .await
                                    .unwrap_or(false);

                                if should_pair {
                                    self.config.certificate.replace(self.stream_cert.clone());
                                }

                                if !initiated_pair {
                                    let pair_packet = Pair { pair: should_pair };
                                    self.stream_w.send(make_packet_str!(pair_packet)?).await?;
                                }

                                if should_pair {
                                    handler.handle_pair_status_change(true).await;
                                    self.initiated_pair.store(false, Ordering::Release);
                                }

                                self.send_paired_data(handler).await?;

                                self.pair_event.notify(usize::MAX);

                                debug!(
                                    "{} pair request from {:?}",
                                    if should_pair { "accepted" } else { "refused" },
                                    self.config.id
                                );
                            } else if !self.is_paired()
                                && self.initiated_pair.load(Ordering::Acquire)
                                && !body.pair
                            {
                                // rejected a pair request
                                self.pair_event.notify(usize::MAX);
                            }
                            self.config_provider
                                .store_device_config(&self.config)
                                .await?;
                        }
                        Battery::TYPE => {
                            handler.handle_battery(json::from_value(packet.body)?).await;
                        }
                        Clipboard::TYPE => {
                            let clipboard: Clipboard = json::from_value(packet.body)?;
                            handler.handle_clipboard_content(clipboard.content).await;
                        }
                        ClipboardConnect::TYPE => {
                            let connect: ClipboardConnect = json::from_value(packet.body)?;
                            if connect.timestamp != 0 {
                                handler.handle_clipboard_content(connect.content).await;
                            }
                        }
                        FindPhone::TYPE => {
                            handler.handle_find_phone().await;
                        }
                        _ => error!(
                            "unknown type {:?}, ignoring: {:#?}",
                            packet.packet_type, packet.body
                        ),
                    }
                }
                DeviceEvent::Client(action) => {
                    use DeviceAction as A;
                    match action {
                        A::SendPacket(packet, response) => {
                            let _ = response.send(
                                self.stream_w
                                    .send(packet)
                                    .await
                                    .map_err(KdeConnectError::from),
                            );
                        }
                        A::GetConfig(response) => {
                            let _ = response.send(self.config.clone());
                        }
                        A::GetPaired(response) => {
                            let _ = response.send(self.is_paired());
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct DeviceClient {
    client_w: mpsc::UnboundedSender<DeviceAction>,
    initiated_pair: Arc<AtomicBool>,

    pair_event: Arc<Event>,
}

impl DeviceClient {
    pub(crate) fn new(
        client_w: mpsc::UnboundedSender<DeviceAction>,
        initiated_pair: Arc<AtomicBool>,
        pair_event: Arc<Event>,
    ) -> Self {
        Self {
            client_w,

            initiated_pair,
            pair_event,
        }
    }

    async fn send_packet(&self, packet: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.client_w.send(DeviceAction::SendPacket(packet, tx))?;
        rx.await?
    }

    pub async fn send_ping(&self, message: Option<String>) -> Result<()> {
        let ping = Ping { message };
        self.send_packet(make_packet_str!(ping)?).await
    }

    pub async fn send_battery_update(&self, packet: Battery) -> Result<()> {
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn send_clipboard_update(&self, content: String) -> Result<()> {
        let packet = Clipboard { content };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn get_config(&self) -> Result<DeviceConfig> {
        let (tx, rx) = oneshot::channel();
        self.client_w.send(DeviceAction::GetConfig(tx))?;
        Ok(rx.await?)
    }

    pub async fn is_paired(&self) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.client_w.send(DeviceAction::GetPaired(tx))?;
        Ok(rx.await?)
    }

    pub async fn pair(&self) -> Result<()> {
        if self.is_paired().await? {
            return Err(KdeConnectError::AlreadyPaired);
        }
        let pair = Pair { pair: true };
        self.send_packet(make_packet_str!(pair)?).await?;
        self.initiated_pair.store(true, Ordering::Release);
        self.pair_event.listen().await;
        self.is_paired().await.and_then(|x| {
            if x {
                Ok(())
            } else {
                Err(KdeConnectError::DeviceRejectedPair)
            }
        })
    }

    pub async fn toggle_find_phone(&self) -> Result<()> {
        let packet = FindPhone {};
        self.send_packet(make_packet_str!(packet)?).await
    }
}

#[async_trait::async_trait]
pub trait DeviceHandler {
    async fn handle_ping(&mut self, packet: Ping);
    async fn handle_pair_status_change(&mut self, pair_status: bool);
    async fn handle_battery(&mut self, packet: Battery);
    async fn handle_clipboard_content(&mut self, content: String);
    async fn handle_find_phone(&mut self);

    async fn handle_pairing_request(&mut self) -> bool;

    async fn get_battery(&mut self) -> Battery;
    async fn get_clipboard_content(&mut self) -> String;

    async fn handle_exit(&mut self);
}
