use std::{
    future::Future,
    net::IpAddr,
    os::unix::fs::MetadataExt,
    path::Path,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use event_listener::Event;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json as json;
use sha2::{Digest, Sha256};
use tokio::{
    fs::File,
    io::{split, AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader, Lines, ReadHalf, WriteHalf},
    net::TcpStream,
    select,
    sync::{mpsc, oneshot, Mutex},
    time::timeout,
};
use tokio_rustls::{
    rustls::{ClientConfig, ServerConfig},
    TlsStream,
};

use crate::{
    config::ConfigProvider,
    make_packet, make_packet_payload, make_packet_str, make_packet_str_payload,
    packets::{
        Battery, BatteryRequest, Clipboard, ClipboardConnect, ConnectivityReport,
        ConnectivityReportRequest, DeviceType, FindPhone, Identity, MousepadEcho,
        MousepadKeyboardState, MousepadRequest, Mpris, MprisPlayer, MprisRequest,
        MprisRequestAction, Packet, PacketPayloadTransferInfo, PacketType, Pair, Ping, Presenter,
        ShareRequest, ShareRequestFile, ShareRequestUpdate, SystemVolume, SystemVolumeRequest,
        SystemVolumeStream,
    },
    util::{create_payload, get_payload, get_public_key, get_time_ms},
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

pub(crate) async fn create_device(
    identity: Identity,
    config_provider: Arc<dyn ConfigProvider + Sync + Send>,
    stream: TlsStream<BufReader<TcpStream>>,
    connected_clients: Arc<Mutex<Vec<String>>>,
    server_config: Arc<ServerConfig>,
    client_config: Arc<ClientConfig>,
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
            client_config,
            server_config.clone(),
        )
        .await?,
        DeviceClient::new(client_tx, initiated_pair, pair_event, server_config),
    ))
}

pub struct Device {
    pub config: DeviceConfig,
    config_provider: Arc<dyn ConfigProvider + Sync + Send>,
    connected_clients: Arc<Mutex<Vec<String>>>,

    server_config: Arc<ServerConfig>,
    client_config: Arc<ClientConfig>,

    stream_r: Lines<BufReader<ReadHalf<TlsStream<BufReader<TcpStream>>>>>,
    stream_w: LockedDeviceWrite,
    stream_cert: Vec<u8>,

    client_r: mpsc::UnboundedReceiver<DeviceAction>,
    ip: IpAddr,

    initiated_pair: Arc<AtomicBool>,
    pair_event: Arc<Event>,

    mpris_supports_album_art: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceConfig {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub certificate: Option<Vec<u8>>,
}

impl DeviceConfig {
    pub fn is_paired(&self) -> bool {
        self.certificate.is_some()
    }
}

pub(crate) enum DeviceAction {
    SendPacket(String, oneshot::Sender<Result<()>>),
    GetConfig(oneshot::Sender<DeviceConfig>),
    GetKey(oneshot::Sender<Result<String>>),
    GetPaired(oneshot::Sender<bool>),
    Unpair,
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
        client_config: Arc<ClientConfig>,
        server_config: Arc<ServerConfig>,
    ) -> Result<Self> {
        let cert = conf.and_then(|x| x.certificate);

        let stream_cert = stream
            .get_ref()
            .1
            .peer_certificates()
            .ok_or(KdeConnectError::NoPeerCerts)?[0]
            .to_vec();

        let ip = stream.get_ref().0.get_ref().peer_addr()?.ip();

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

            server_config,
            client_config,

            stream_r: BufReader::new(r).lines(),
            stream_w: LockedDeviceWrite::new(w),
            stream_cert,

            client_r,
            ip,

            initiated_pair,
            pair_event,

            mpris_supports_album_art: false,
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

    pub async fn get_verification_key(&self) -> Result<String> {
        let mut own_key = get_public_key(&self.config_provider.retrieve_server_cert().await?)?;
        let mut device_key = get_public_key(&self.stream_cert)?;
        let mut sha256 = Sha256::new();
        if own_key < device_key {
            (own_key, device_key) = (device_key, own_key);
        }
        sha256.update(own_key);
        sha256.update(device_key);
        let digest = sha256.finalize();
        Ok(hex::encode(digest))
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
                timestamp: get_time_ms(),
            };
            self.stream_w.send(make_packet_str!(clipboard)?).await?;

            let connectivity = handler.get_connectivity_report().await;
            self.stream_w.send(make_packet_str!(connectivity)?).await?;

            let system_volume = SystemVolume::List {
                sink_list: handler.get_system_volume().await,
            };
            self.stream_w.send(make_packet_str!(system_volume)?).await?;
        }
        Ok(())
    }

    fn is_paired(&self) -> bool {
        self.config.is_paired()
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
                            self.send_paired_data(handler).await?;
                        }
                        Pair::TYPE => {
                            let body: Pair = json::from_value(packet.body)?;
                            let initiated_pair = self.initiated_pair.load(Ordering::Acquire);
                            if self.is_paired() && body.pair {
                                warn!("{} asking to pair when already paired??", self.config.id);
                            } else if initiated_pair && !self.is_paired() && !body.pair {
                                // if we initiated pair notify the client
                                self.initiated_pair.store(false, Ordering::Release);
                                self.pair_event.notify(usize::MAX);
                            } else if !self.is_paired() && !body.pair {
                                warn!(
                                    "{} asking to unpair when already unpaired??",
                                    self.config.id
                                );
                            } else if !self.is_paired() && body.pair {
                                // pairing

                                let should_pair = initiated_pair
                                    || timeout(
                                        Duration::from_secs(30),
                                        handler.handle_pairing_request(),
                                    )
                                    .await
                                    .unwrap_or(false);

                                self.initiated_pair.store(false, Ordering::Release);

                                if !initiated_pair {
                                    // send response if other side requested pair
                                    let pair_packet = Pair { pair: should_pair };
                                    self.stream_w.send(make_packet_str!(pair_packet)?).await?;
                                }

                                if should_pair {
                                    // if we initiated pair and they accepted or they initiated
                                    // pair and we accepted, finish pairing process
                                    self.config.certificate.replace(self.stream_cert.clone());
                                    self.config_provider
                                        .store_device_config(&self.config)
                                        .await?;
                                    handler.handle_pair_status_change(true).await;
                                    self.send_paired_data(handler).await?;
                                }

                                if initiated_pair {
                                    // if we initiated pair notify the client
                                    self.pair_event.notify(usize::MAX);
                                }

                                debug!(
                                    "{} pair request from {:?}",
                                    if should_pair { "accepted" } else { "refused" },
                                    self.config.id
                                );
                            } else if self.is_paired() && !body.pair {
                                // unpair

                                self.config.certificate.take();
                                self.config_provider
                                    .store_device_config(&self.config)
                                    .await?;
                                handler.handle_pair_status_change(false).await;
                            }
                        }
                        Battery::TYPE => {
                            handler.handle_battery(json::from_value(packet.body)?).await;
                        }
                        BatteryRequest::TYPE => {
                            let battery = handler.get_battery().await;
                            self.stream_w.send(make_packet_str!(battery)?).await?;
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
                        ConnectivityReport::TYPE => {
                            handler
                                .handle_connectivity_report(json::from_value(packet.body)?)
                                .await;
                        }
                        ConnectivityReportRequest::TYPE => {
                            let connectivity = handler.get_connectivity_report().await;
                            self.stream_w.send(make_packet_str!(connectivity)?).await?;
                        }
                        Presenter::TYPE => {
                            handler
                                .handle_presenter(json::from_value(packet.body)?)
                                .await;
                        }
                        SystemVolume::TYPE => {
                            handler
                                .handle_system_volume(json::from_value(packet.body)?)
                                .await;
                        }
                        SystemVolumeRequest::TYPE => {
                            let request: SystemVolumeRequest = json::from_value(packet.body)?;
                            if request.request_sinks.unwrap_or(false) {
                                let system_volume = SystemVolume::List {
                                    sink_list: handler.get_system_volume().await,
                                };
                                self.stream_w.send(make_packet_str!(system_volume)?).await?;
                            } else {
                                handler.handle_system_volume_request(request).await;
                            }
                        }
                        ShareRequestUpdate::TYPE => {
                            let update: ShareRequestUpdate = json::from_value(packet.body)?;
                            handler.handle_multi_file_share(update).await;
                        }
                        ShareRequest::TYPE => {
                            // weird bug, fails to deser ShareRequestFile variant so we do it
                            // manually
                            let request: ShareRequest =
                                if let Ok(request_file) = json::from_value(packet.body.clone()) {
                                    ShareRequest::File(request_file)
                                } else {
                                    json::from_value(packet.body)?
                                };
                            if let Some(transfer_info) = packet.payload_transfer_info
                                && let Some(size) = packet.payload_size
                                && let ShareRequest::File(file) = request
                            {
                                handler
                                    .handle_file_share(
                                        file,
                                        size,
                                        get_payload(
                                            self.ip,
                                            transfer_info,
                                            self.client_config.clone(),
                                        )
                                        .await?,
                                    )
                                    .await;
                            } else {
                                match request {
                                    ShareRequest::Text { text } => {
                                        handler.handle_text_share(text).await;
                                    }
                                    ShareRequest::Url { url } => {
                                        handler.handle_url_share(url).await;
                                    }
                                    ShareRequest::File(_) => {} // ignore - no payload transfer info
                                }
                            }
                        }
                        Mpris::TYPE => {
                            let mpris: Mpris = json::from_value(packet.body)?;
                            match mpris {
                                Mpris::List {
                                    player_list,
                                    supports_album_art_payload,
                                } => {
                                    self.mpris_supports_album_art = supports_album_art_payload;
                                    handler.handle_mpris_player_list(player_list).await;
                                }
                                Mpris::TransferringArt {
                                    player,
                                    album_art_url: _,
                                    transferring_album_art,
                                } => {
                                    if transferring_album_art
                                        && let Some(transfer_info) = packet.payload_transfer_info
                                    {
                                        handler
                                            .handle_mpris_player_album_art(
                                                player,
                                                get_payload(
                                                    self.ip,
                                                    transfer_info,
                                                    self.client_config.clone(),
                                                )
                                                .await?,
                                            )
                                            .await;
                                    }
                                }
                                Mpris::Info(player) => {
                                    handler.handle_mpris_player_info(player).await;
                                }
                            }
                        }
                        MprisRequest::TYPE => {
                            let req: MprisRequest = json::from_value(packet.body)?;
                            match req {
                                MprisRequest::List { .. } => {
                                    let packet = Mpris::List {
                                        player_list: handler.get_mpris_player_list().await,
                                        supports_album_art_payload: true,
                                    };
                                    self.stream_w.send(make_packet_str!(packet)?).await?;
                                }
                                MprisRequest::PlayerRequest {
                                    player,
                                    request_album_art,
                                    ..
                                } => {
                                    if let Some(player_info) =
                                        handler.get_mpris_player(player.clone()).await
                                    {
                                        if let Some(url) = request_album_art
                                            && url.starts_with("file://")
                                            && player_info
                                                .album_art_url
                                                .as_ref()
                                                .map(|x| *x == url)
                                                .unwrap_or(false)
                                        {
                                            let server_conf = self.server_config.clone();
                                            let ret = async {
                                                let art =
                                                    File::open(url.trim_start_matches("file://"))
                                                        .await?;
                                                let size = art.metadata().await?.size();
                                                let (port, fut) =
                                                    create_payload(art, server_conf).await?;
                                                let packet = Mpris::TransferringArt {
                                                    player,
                                                    album_art_url: url,
                                                    transferring_album_art: true,
                                                };
                                                self.stream_w
                                                    .send(make_packet_str_payload!(
                                                        packet,
                                                        size as i64,
                                                        port
                                                    )?)
                                                    .await?;
                                                tokio::spawn(fut);
                                                Ok::<(), KdeConnectError>(())
                                            }
                                            .await;
                                            if let Err(e) = ret {
                                                error!("failed to send album art: {:?}", e);
                                            }
                                        }
                                        let packet = Mpris::Info(player_info);
                                        self.stream_w.send(make_packet_str!(packet)?).await?;
                                    }
                                }
                                MprisRequest::Action(action) => {
                                    handler.handle_mpris_player_action(action).await;
                                }
                            }
                        },
                        MousepadRequest::TYPE => {
                            handler.handle_mousepad_request(json::from_value(packet.body)?).await;
                        },
                        MousepadEcho::TYPE => {
                            handler.handle_mousepad_echo(json::from_value(packet.body)?).await;
                        },
                        MousepadKeyboardState::TYPE => {
                            handler.handle_mousepad_keyboard_state(json::from_value(packet.body)?).await;
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
                            info!("packet {:?}", packet);
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
                        A::GetKey(response) => {
                            let _ = response.send(self.get_verification_key().await);
                        }
                        A::GetPaired(response) => {
                            let _ = response.send(self.is_paired());
                        }
                        A::Unpair => {
                            self.config.certificate.take();
                            handler.handle_pair_status_change(false).await;
                            self.config_provider
                                .store_device_config(&self.config)
                                .await?;
                            let pair_packet = Pair { pair: false };
                            self.stream_w.send(make_packet_str!(pair_packet)?).await?;
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
    server_config: Arc<ServerConfig>,

    pair_event: Arc<Event>,
}

impl DeviceClient {
    pub(crate) fn new(
        client_w: mpsc::UnboundedSender<DeviceAction>,
        initiated_pair: Arc<AtomicBool>,
        pair_event: Arc<Event>,
        server_config: Arc<ServerConfig>,
    ) -> Self {
        Self {
            client_w,

            initiated_pair,
            pair_event,
            server_config,
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

    pub async fn send_connectivity_report(&self, packet: ConnectivityReport) -> Result<()> {
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn send_presenter_update(&self, packet: Presenter) -> Result<()> {
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn send_volume_update(&self, streams: Vec<SystemVolumeStream>) -> Result<()> {
        let packet = SystemVolume::List { sink_list: streams };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn send_volume_stream_update(
        &self,
        name: String,
        enabled: Option<bool>,
        muted: Option<bool>,
        volume: Option<i32>,
    ) -> Result<()> {
        let packet = SystemVolume::Update {
            name,
            enabled,
            muted,
            volume,
        };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn request_volume_list(&self) -> Result<()> {
        let packet = SystemVolumeRequest {
            request_sinks: Some(true),
            name: None,
            enabled: None,
            muted: None,
            volume: None,
        };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn send_volume_request(
        &self,
        name: String,
        enabled: Option<bool>,
        muted: Option<bool>,
        volume: Option<i32>,
    ) -> Result<()> {
        let packet = SystemVolumeRequest {
            request_sinks: None,
            name: Some(name),
            enabled,
            muted,
            volume,
        };
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

    pub async fn change_pair_state(&self, new_state: bool) -> Result<()> {
        // trying to pair and already paired?
        if new_state && self.is_paired().await? {
            return Err(KdeConnectError::DeviceAlreadyPaired);
        }
        let pair = Pair { pair: new_state };
        self.send_packet(make_packet_str!(pair)?).await?;
        // trying to pair? if so wait for pair response
        if new_state {
            self.initiated_pair.store(true, Ordering::Release);
            self.pair_event.listen().await;
            self.is_paired().await.and_then(|x| {
                if x {
                    Ok(())
                } else {
                    Err(KdeConnectError::DeviceRejectedPair)
                }
            })
        } else {
            self.client_w.send(DeviceAction::Unpair)?;
            Ok(())
        }
    }

    pub async fn toggle_find_phone(&self) -> Result<()> {
        let packet = FindPhone {};
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn get_verification_key(&self) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.client_w.send(DeviceAction::GetKey(tx))?;
        rx.await?
    }

    pub async fn share_text(&self, text: String) -> Result<()> {
        let packet = ShareRequest::Text { text };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn share_url(&self, url: String) -> Result<()> {
        let packet = ShareRequest::Url { url };
        self.send_packet(make_packet_str!(packet)?).await
    }

    async fn share_file_internal(
        &self,
        file: DeviceFile<impl AsyncRead + Sync + Send + Unpin>,
        open: bool,
        number_of_files: Option<i32>,
        total_payload_size: Option<i64>,
    ) -> Result<()> {
        let (port, fut) = create_payload(file.buf, self.server_config.clone()).await?;
        let packet = ShareRequest::File(ShareRequestFile {
            filename: file.name,
            creation_time: file.creation_time,
            last_modified: file.last_modified,
            open: Some(open),
            number_of_files,
            total_payload_size,
        });
        self.send_packet(make_packet_str_payload!(packet, file.size, port)?)
            .await?;
        fut.await;
        Ok(())
    }

    pub async fn share_file(
        &self,
        file: DeviceFile<impl AsyncRead + Sync + Send + Unpin>,
        open: bool,
    ) -> Result<()> {
        self.share_file_internal(file, open, None, None).await
    }

    pub async fn share_files_manual<'a>(
        &'a self,
        files: Vec<DeviceFile<impl AsyncRead + Sync + Send + Unpin + 'a>>,
        open: bool,
    ) -> Result<Vec<impl Future<Output = Result<()>> + Sync + Send + 'a>> {
        let mut total_size: i64 = files.iter().map(|x| x.size).sum();
        let mut file_cnt = files.len() as i32;
        let multi_packet = ShareRequestUpdate {
            number_of_files: Some(file_cnt),
            total_payload_size: Some(total_size),
        };
        self.send_packet(make_packet_str!(multi_packet)?).await?;
        let mut futs = Vec::with_capacity(files.len());
        for file in files {
            let file_size = file.size;
            futs.push(self.share_file_internal(file, open, Some(file_cnt), Some(total_size)));
            file_cnt -= 1;
            total_size -= file_size;
        }
        Ok(futs)
    }

    pub async fn share_files(
        &self,
        files: Vec<DeviceFile<impl AsyncRead + Sync + Send + Unpin>>,
        open: bool,
    ) -> Result<()> {
        for fut in self.share_files_manual(files, open).await? {
            fut.await?;
        }
        Ok(())
    }

    pub async fn send_mpris_list(&self, list: Vec<String>) -> Result<()> {
        let packet = Mpris::List {
            player_list: list,
            supports_album_art_payload: true,
        };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn send_mpris_album_art(
        &self,
        player: String,
        url: String,
        art: DevicePayload<impl AsyncRead + Sync + Send + Unpin>,
    ) -> Result<()> {
        let (port, fut) = create_payload(art.buf, self.server_config.clone()).await?;
        let packet = Mpris::TransferringArt {
            player,
            album_art_url: url,
            transferring_album_art: true,
        };
        self.send_packet(make_packet_str_payload!(packet, art.size, port)?)
            .await?;
        fut.await;
        Ok(())
    }

    pub async fn send_mpris_info(&self, player: MprisPlayer) -> Result<()> {
        let packet = Mpris::Info(player);
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn request_mpris_list(&self) -> Result<()> {
        let packet = MprisRequest::List {
            request_player_list: true,
        };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn request_mpris_info(
        &self,
        player: String,
        album_art: Option<String>,
    ) -> Result<()> {
        let packet = MprisRequest::PlayerRequest {
            player,
            request_now_playing: Some(true),
            request_volume: Some(true),
            request_album_art: album_art,
        };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn request_mpris_action(&self, action: MprisRequestAction) -> Result<()> {
        let packet = MprisRequest::Action(action);
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn request_mousepad_action(&self, action: MousepadRequest) -> Result<()> {
        self.send_packet(make_packet_str!(action)?).await
    }

    pub async fn send_mousepad_keyboard_state(&self) -> Result<()> {
        let packet = MousepadKeyboardState { state: true };
        self.send_packet(make_packet_str!(packet)?).await
    }

    pub async fn send_mousepad_echo(&self, echo: MousepadRequest) -> Result<()> {
        self.send_packet(make_packet_str!(echo)?).await
    }
}

#[async_trait::async_trait]
pub trait DeviceHandler {
    async fn handle_ping(&mut self, packet: Ping);
    async fn handle_pair_status_change(&mut self, pair_status: bool);
    async fn handle_battery(&mut self, packet: Battery);
    async fn handle_clipboard_content(&mut self, content: String);
    async fn handle_find_phone(&mut self);
    async fn handle_connectivity_report(&mut self, packet: ConnectivityReport);
    async fn handle_presenter(&mut self, packet: Presenter);
    async fn handle_system_volume(&mut self, packet: SystemVolume);
    async fn handle_system_volume_request(&mut self, packet: SystemVolumeRequest);
    async fn handle_multi_file_share(&mut self, packet: ShareRequestUpdate);
    async fn handle_file_share(
        &mut self,
        packet: ShareRequestFile,
        size: i64,
        data: Pin<Box<dyn AsyncRead + Sync + Send>>,
    );
    async fn handle_url_share(&mut self, url: String);
    async fn handle_text_share(&mut self, text: String);
    async fn handle_mpris_player_list(&mut self, list: Vec<String>);
    async fn handle_mpris_player_info(&mut self, player: MprisPlayer);
    async fn handle_mpris_player_album_art(
        &mut self,
        player: String,
        art: Pin<Box<dyn AsyncRead + Sync + Send>>,
    );
    async fn handle_mpris_player_action(&mut self, action: MprisRequestAction);
    async fn handle_mousepad_request(&mut self, action: MousepadRequest);
    async fn handle_mousepad_keyboard_state(&mut self, state: MousepadKeyboardState);
    async fn handle_mousepad_echo(&mut self, echo: MousepadEcho);

    async fn handle_pairing_request(&mut self) -> bool;

    async fn get_battery(&mut self) -> Battery;
    async fn get_clipboard_content(&mut self) -> String;
    async fn get_connectivity_report(&mut self) -> ConnectivityReport;
    async fn get_system_volume(&mut self) -> Vec<SystemVolumeStream>;
    async fn get_mpris_player_list(&mut self) -> Vec<String>;
    async fn get_mpris_player(&mut self, player: String) -> Option<MprisPlayer>;

    async fn handle_exit(&mut self);
}

pub struct DeviceFile<S: AsyncRead + Sync + Send + Unpin> {
    pub buf: S,
    pub size: i64,
    pub name: String,
    pub creation_time: Option<u128>,
    pub last_modified: Option<u128>,
}

impl DeviceFile<File> {
    pub async fn try_from_tokio(file: File, name: String) -> Result<Self> {
        file.sync_all().await?;
        let metadata = file.metadata().await?;
        Ok(DeviceFile {
            buf: file,
            size: metadata.size().try_into().map_err(std::io::Error::other)?,
            name,
            creation_time: Some(
                metadata
                    .created()?
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("time went backwards")
                    .as_millis(),
            ),
            last_modified: Some(
                metadata
                    .modified()?
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("time went backwards")
                    .as_millis(),
            ),
        })
    }

    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path: &Path = path.as_ref();
        Self::try_from_tokio(
            File::open(path).await?,
            path.file_name()
                .ok_or(KdeConnectError::NoFileName)?
                .to_os_string()
                .into_string()
                .map_err(|_| KdeConnectError::OsStringConversionError)?,
        )
        .await
    }
}

pub struct DevicePayload<S: AsyncRead + Sync + Send + Unpin> {
    pub buf: S,
    pub size: i64,
}

impl<S: AsyncRead + Sync + Send + Unpin> From<DeviceFile<S>> for DevicePayload<S> {
    fn from(file: DeviceFile<S>) -> Self {
        Self {
            buf: file.buf,
            size: file.size,
        }
    }
}
