#![allow(clippy::size_of_in_element_count)]

use std::{
    collections::HashMap,
    error::Error,
    ffi::OsStr,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use kdeconnect::{
    device::{DeviceClient, DeviceConfig, DeviceHandler},
    packets::{
        Battery, ConnectivityReport, DeviceType, MousepadEcho, MousepadKeyboardState,
        MousepadRequest, MprisAction, MprisLoopStatus, MprisPlayer, MprisRequestAction, Ping,
        Presenter, RunCommandItem, ShareRequestFile, ShareRequestUpdate, SystemVolume,
        SystemVolumeRequest, SystemVolumeStream,
    },
    KdeConnectError,
};
use log::{error, info, warn};
use safer_ffi::prelude::*;
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWriteExt},
    sync::Mutex,
    task::JoinHandle,
    time::interval,
};
use tokio_stream::StreamExt;
use tokio_util::io::StreamReader;

use crate::{call_callback, call_callback_no_ret, STATE};

#[derive(Default)]
pub struct KConnectDeviceState {
    pub battery: Option<Battery>,
    pub clipboard: Option<String>,
    pub connectivity: Option<ConnectivityReport>,
    pub systemvolume: Option<Vec<SystemVolumeStream>>,
    pub players: HashMap<String, (MprisPlayer, Option<String>, Option<JoinHandle<()>>)>,
    pub commands: HashMap<String, RunCommandItem>,
}

pub struct KConnectDevice {
    pub client: Arc<DeviceClient>,
    pub config: DeviceConfig,
    pub state: Arc<Mutex<KConnectDeviceState>>,
}

pub struct KConnectHandler {
    state: Arc<Mutex<KConnectDeviceState>>,
    client: Arc<DeviceClient>,
    config: DeviceConfig,
    id: char_p::Box,
    verification_key: char_p::Box,
    documents_path: PathBuf,
}

impl KConnectHandler {
    pub fn new(
        state: Arc<Mutex<KConnectDeviceState>>,
        client: Arc<DeviceClient>,
        mut config: DeviceConfig,
        verification_key: String,
        documents_path: PathBuf,
    ) -> Self {
        // we don't need the cert
        config.certificate.take();
        Self {
            state,
            client,
            // this should never fail
            id: config.id.clone().try_into().unwrap(),
            config,
            // this should never fail
            verification_key: verification_key.try_into().unwrap(),
            documents_path,
        }
    }
}

impl KConnectHandler {
    fn get_player_join_handle(&self, player: &MprisPlayer) -> Option<JoinHandle<()>> {
        if player.is_playing.unwrap_or(false) {
            let state_clone = self.state.clone();
            let player_id = player.player.clone();
            let id = self.id.clone();
            Some(tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    let mut state_locked = state_clone.lock().await;
                    if let Some(ref mut player) = state_locked.players.get_mut(&player_id) {
                        if player.0.is_playing.unwrap_or(false) {
                            if let Some(ref mut pos) = player.0.pos {
                                *pos += 1000;
                                let id = id.clone();
                                call_callback_no_ret!(player_changed, id);
                            }
                        }
                    }
                    drop(state_locked);
                }
            }))
        } else {
            None
        }
    }

    fn maybe_request_more_mpris_info(&self, player: &MprisPlayer) {
        // we recieved some state change for a player we didn't know existed, attempt to request more info
        if player.title.is_none() {
            info!("attempting to request more info about {:?}", player.player);
            let client = self.client.clone();
            let player = player.player.clone();
            tokio::spawn(async move {
                if let Err(err) = client.request_mpris_info(player.clone(), None).await {
                    warn!("failed to request more info about {:?}: {:?}", player, err)
                }
            });
        }
    }
}

#[async_trait]
impl DeviceHandler for KConnectHandler {
    async fn handle_ping(&mut self, packet: Ping) {
        info!(
            "recieved ping: {:?} packet: {:#?}",
            self.config.name, packet
        );

        let id = self.id.clone();
        call_callback_no_ret!(ping_recieved, id);
    }

    async fn handle_pair_status_change(&mut self, pair_status: bool) {
        info!(
            "device {}: {:?}",
            if pair_status { "paired" } else { "unpaired" },
            self.config.name
        );

        let id = self.id.clone();
        call_callback_no_ret!(pair_status_changed, id, pair_status)
    }

    async fn handle_battery(&mut self, packet: Battery) {
        let mut state = self.state.lock().await;
        state.battery.replace(packet);
        drop(state);

        info!(
            "recieved battery data: {:?} packet: {:#?}",
            self.config.name, packet
        );

        let id = self.id.clone();
        call_callback_no_ret!(battery_changed, id);
    }

    async fn handle_clipboard_content(&mut self, content: String) {
        self.state.lock().await.clipboard.replace(content.clone());

        let id = self.id.clone();
        // this should never fail
        let content = content.try_into().unwrap();
        call_callback_no_ret!(clipboard_changed, id, content);
    }

    async fn handle_find_phone(&mut self) {
        // STATE will always be Some here
        let mut locked = STATE.lock().await;
        let state = locked.as_mut().unwrap();
        state.being_found = !state.being_found;
        if state.being_found {
            call_callback_no_ret!(find_requested,);
        }
    }

    async fn handle_connectivity_report(&mut self, packet: ConnectivityReport) {
        self.state.lock().await.connectivity.replace(packet);

        let id = self.id.clone();
        call_callback_no_ret!(connectivity_changed, id)
    }

    async fn handle_presenter(&mut self, _packet: Presenter) {
        // Ignore - not much use on iOS
    }

    async fn handle_system_volume(&mut self, packet: SystemVolume) {
        info!("system volume: {:?}", packet);
        match packet {
            SystemVolume::List { sink_list } => {
                self.state.lock().await.systemvolume.replace(sink_list);
            }
            SystemVolume::Update {
                name,
                enabled,
                muted,
                volume,
            } => {
                let mut state = self.state.lock().await;
                if let Some(stream) = state
                    .systemvolume
                    .as_mut()
                    .and_then(|x| x.iter_mut().find(|x| x.name == name))
                {
                    if let Some(enabled) = enabled {
                        stream.enabled = Some(enabled);
                    }
                    if let Some(muted) = muted {
                        stream.muted = muted;
                    }
                    if let Some(volume) = volume {
                        stream.volume = volume;
                    }
                }
            }
        }

        let id = self.id.clone();
        call_callback_no_ret!(volume_changed, id);
    }

    async fn handle_system_volume_request(&mut self, packet: SystemVolumeRequest) {
        // name & enabled do nothing on iOS as there's only one systemvolume stream: Core Audio
        if let Some(volume) = packet.volume {
            call_callback_no_ret!(volume_change_requested, volume);
        }
        if let Some(muted) = packet.muted {
            if muted {
                call_callback_no_ret!(volume_change_requested, 0)
            } else {
                // STATE is always Some here
                let vol = STATE.lock().await.as_ref().unwrap().current_volume;
                call_callback_no_ret!(volume_change_requested, vol);
            }
        }
    }

    async fn handle_multi_file_share(&mut self, _packet: ShareRequestUpdate) {
        // ignore
    }

    async fn handle_file_share(
        &mut self,
        packet: ShareRequestFile,
        _size: i64,
        mut data: Pin<Box<dyn AsyncRead + Sync + Send>>,
    ) {
        let ret = async {
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("time went backwards")
                .as_millis();
            let mut path = self.documents_path.join(packet.filename);
            if tokio::fs::try_exists(&path).await? {
                let current_name = path
                    .file_name()
                    .unwrap_or(OsStr::new(""))
                    .to_os_string()
                    .into_string()
                    .map_err(|_| KdeConnectError::OsStringConversionError)?;
                path.set_file_name(current_time.to_string() + &current_name);
            }
            let mut file = File::create(&path).await?;
            // kdeconnect-kde is weird sometimes and closes without properly closing TLS
            let _ = tokio::io::copy(&mut data, &mut file).await;
            file.shutdown().await?;
            file.sync_all().await?;
            Ok::<String, Box<dyn Error + Sync + Send>>(
                path.into_os_string()
                    .into_string()
                    .map_err(|_| KdeConnectError::OsStringConversionError)?,
            )
        }
        .await;

        match ret {
            Ok(path) => {
                // this should never fail
                call_callback_no_ret!(open_file, path.try_into().unwrap());
            }
            Err(err) => {
                error!("failed to save file from share: {:?}", err);
            }
        }
    }

    async fn handle_url_share(&mut self, url: String) {
        // this should never fail
        call_callback_no_ret!(open_url, url.try_into().unwrap());
    }

    async fn handle_text_share(&mut self, text: String) {
        // this should never fail
        call_callback_no_ret!(open_text, text.try_into().unwrap());
    }

    async fn handle_mpris_player_list(&mut self, players: Vec<String>) {
        info!("got player list {:?}", players);
        self.state
            .lock()
            .await
            .players
            .retain(|x, _| players.contains(x));

        let client = self.client.clone();

        tokio::spawn(async move {
            for player in players {
                let _ = client.request_mpris_info(player, None).await;
            }
        });
    }

    async fn handle_mpris_player_info(&mut self, player: MprisPlayer) {
        info!("got mpris {:?}", player);
        if let Some(art_url) = player.album_art_url.clone()
            && art_url.starts_with("file:")
        {
            let client = self.client.clone();
            let player = player.player.clone();
            tokio::spawn(async move {
                // request album art (kdeconnect-kde)
                let _ = client.request_mpris_info(player, Some(art_url)).await;
            });
        } else if let Some(art_url) = player.album_art_url.clone() {
            let player = player.player.clone();
            // maybe don't block the device task
            if let Ok(art) = reqwest::get(&art_url).await {
                let reader =
                    StreamReader::new(art.bytes_stream().map(|x| x.map_err(std::io::Error::other)));

                self.handle_mpris_player_album_art(player, Box::pin(reader))
                    .await;
            }
            info!("finished mpris download");
        }

        let mut locked = self.state.lock().await;

        if let Some(state_player) = locked.players.get_mut(&player.player) {
            macro_rules! assign {
                ($item:ident) => {
                    if let Some($item) = player.$item {
                        state_player.0.$item = Some($item);
                    }
                };
            }

            assign!(title);
            assign!(artist);
            assign!(album);
            assign!(is_playing);
            assign!(can_pause);
            assign!(can_play);
            assign!(can_go_next);
            assign!(can_go_previous);
            assign!(can_seek);
            assign!(loop_status);
            assign!(shuffle);
            assign!(pos);
            assign!(length);
            assign!(volume);
            assign!(url);

            match (
                state_player.0.is_playing.unwrap_or(false),
                state_player.2.is_some(),
            ) {
                // not playing & no future or playing & a future, do nothing
                (false, false) | (true, true) => {}
                // not playing & a future, abort
                (false, true) => {
                    // we know that it's some because of the is_some call
                    state_player.2.take().unwrap().abort();
                }
                (true, false) => {
                    state_player.2 = self.get_player_join_handle(&state_player.0);
                }
            }

            self.maybe_request_more_mpris_info(&state_player.0);
        } else {
            let join_handle = self.get_player_join_handle(&player);
            self.maybe_request_more_mpris_info(&player);
            locked
                .players
                .insert(player.player.clone(), (player, None, join_handle));
        }

        drop(locked);

        let id = self.id.clone();
        call_callback_no_ret!(player_changed, id);
    }

    async fn handle_mpris_player_album_art(
        &mut self,
        player: String,
        mut data: Pin<Box<dyn AsyncRead + Sync + Send>>,
    ) {
        let album_file_name =
            URL_SAFE_NO_PAD.encode(format!("{}__{}", self.config.id, &player)) + ".png";
        let mut path = self.documents_path.join("album_art/");
        let state = self.state.clone();
        let id = self.id.clone();
        tokio::spawn(async move {
            let ret = async move {
                if !path.is_dir() {
                    if path.exists() {
                        tokio::fs::remove_file(&path).await?;
                    }
                    tokio::fs::create_dir(&path).await?;
                }
                path.push(album_file_name);
                let mut file = File::create(&path).await?;
                // kdeconnect-kde is weird sometimes and closes without properly closing TLS
                let _ = tokio::io::copy(&mut data, &mut file).await;
                file.shutdown().await?;
                file.sync_all().await?;
                path.into_os_string()
                    .into_string()
                    .map_err(|_| KdeConnectError::OsStringConversionError)
            }
            .await;
            match ret {
                Ok(path) => {
                    if let Some(player) = state.lock().await.players.get_mut(&player) {
                        player.1.replace(path);
                        call_callback_no_ret!(player_changed, id);
                        info!("saved album art for player {:?}", player);
                    }
                }
                Err(err) => {
                    error!(
                        "failed to save album art for player {:?}: {:?}",
                        player, err
                    );
                }
            }
        });
    }

    async fn handle_mpris_player_action(&mut self, action: MprisRequestAction) {
        use KConnectMprisPlayerAction as A;
        if let Some(seek) = action.seek {
            call_callback_no_ret!(player_change_requested, A::Seek, seek);
        } else if let Some(loop_status) = action.set_loop_status {
            let action = match loop_status {
                MprisLoopStatus::None => A::LoopStatusNone,
                MprisLoopStatus::Track => A::LoopStatusTrack,
                MprisLoopStatus::Playlist => A::LoopStatusPlaylist,
            };
            call_callback_no_ret!(player_change_requested, action, 0);
        } else if let Some(position) = action.set_position {
            call_callback_no_ret!(player_change_requested, A::Position, position);
        } else if let Some(shuffle) = action.set_shuffle {
            call_callback_no_ret!(
                player_change_requested,
                A::Shuffle,
                if shuffle { 1 } else { 0 }
            );
        } else if let Some(action) = action.action {
            let action = match action {
                MprisAction::Play => A::Play,
                MprisAction::Pause => A::Pause,
                MprisAction::PlayPause => A::PlayPause,
                MprisAction::Stop => A::Stop,
                MprisAction::Next => A::Next,
                MprisAction::Previous => A::Previous,
            };
            call_callback_no_ret!(player_change_requested, action, 0);
        }
    }

    // Ignore as there is not much use
    async fn handle_mousepad_request(&mut self, _: MousepadRequest) {}
    async fn handle_mousepad_keyboard_state(&mut self, _: MousepadKeyboardState) {}
    async fn handle_mousepad_echo(&mut self, _: MousepadEcho) {}

    async fn handle_command_list(&mut self, command_list: HashMap<String, RunCommandItem>) {
        self.state.lock().await.commands = command_list;
        let id = self.id.clone();
        call_callback_no_ret!(commands_changed, id);
    }

    async fn handle_command_request(&mut self, _: String) {
        // TODO
    }

    async fn handle_pairing_request(&mut self) -> bool {
        info!("recieved pair from {:?}", self.config);
        let id = self.id.clone();
        let key = self.verification_key.clone();
        let res = call_callback!(pairing_requested, id, key).unwrap_or(false);

        info!(
            "pair {} from {:?}",
            if res { "accepted" } else { "rejected" },
            self.config.name
        );
        res
    }

    async fn get_battery(&mut self) -> Battery {
        // STATE will always be Some here
        STATE.lock().await.as_ref().unwrap().current_battery
    }

    async fn get_clipboard_content(&mut self) -> String {
        // STATE will always be Some here
        STATE
            .lock()
            .await
            .as_ref()
            .unwrap()
            .current_clipboard
            .clone()
    }

    async fn get_connectivity_report(&mut self) -> ConnectivityReport {
        // STATE will always be Some here
        ConnectivityReport {
            signal_strengths: STATE.lock().await.as_ref().unwrap().current_signals.clone(),
        }
    }

    async fn get_system_volume(&mut self) -> Vec<SystemVolumeStream> {
        // STATE will always be Some here
        let vol = STATE.lock().await.as_ref().unwrap().current_volume;

        vec![SystemVolumeStream {
            name: "coreaudio".to_string(),
            description: "Core Audio".to_string(),
            muted: vol == 0,
            volume: vol,
            max_volume: Some(100),
            enabled: None,
        }]
    }

    async fn get_mpris_player_list(&mut self) -> Vec<String> {
        // STATE will always be Some here
        STATE
            .lock()
            .await
            .as_ref()
            .unwrap()
            .current_player
            .as_ref()
            .map_or_else(Vec::new, |x| vec![x.player.clone()])
    }

    async fn get_mpris_player(&mut self, player: String) -> Option<MprisPlayer> {
        let locked = STATE.lock().await;
        // STATE will always be Some here
        if let Some(current_player) = locked.as_ref().unwrap().current_player.as_ref()
            && current_player.player == player
        {
            Some(current_player.clone())
        } else {
            None
        }
    }

    async fn handle_exit(&mut self) {
        // STATE will always be Some here
        STATE
            .lock()
            .await
            .as_mut()
            .unwrap()
            .devices
            .retain(|x| x.config.id != self.config.id);
        let id = self.id.clone();
        call_callback_no_ret!(gone, id);
    }

    async fn get_command_list(&mut self) -> HashMap<String, RunCommandItem> {
        // TODO
        HashMap::new()
    }
}

#[derive_ReprC]
#[repr(u8)]
pub enum KConnectFfiDeviceType {
    Desktop,
    Laptop,
    Phone,
    Tablet,
    Tv,
}

impl From<DeviceType> for KConnectFfiDeviceType {
    fn from(value: DeviceType) -> Self {
        use DeviceType as D;
        match value {
            D::Desktop => Self::Desktop,
            D::Laptop => Self::Laptop,
            D::Phone => Self::Phone,
            D::Tablet => Self::Tablet,
            D::Tv => Self::Tv,
        }
    }
}

impl From<KConnectFfiDeviceType> for DeviceType {
    fn from(value: KConnectFfiDeviceType) -> Self {
        use KConnectFfiDeviceType as D;
        match value {
            D::Desktop => Self::Desktop,
            D::Laptop => Self::Laptop,
            D::Phone => Self::Phone,
            D::Tablet => Self::Tablet,
            D::Tv => Self::Tv,
        }
    }
}

#[derive_ReprC]
#[repr(C)]
pub struct KConnectFfiDeviceInfo {
    pub id: char_p::Box,
    pub name: char_p::Box,
    pub dev_type: KConnectFfiDeviceType,
}

#[derive_ReprC]
#[repr(C)]
pub struct KConnectFfiDevice {
    pub id: char_p::Box,
    pub name: char_p::Box,
    pub dev_type: KConnectFfiDeviceType,
    pub state: repr_c::Box<KConnectFfiDeviceState>,
}

#[derive_ReprC]
#[repr(opaque)]
pub struct KConnectFfiDeviceState {
    pub(crate) state: Arc<Mutex<KConnectDeviceState>>,
    pub(crate) client: Arc<DeviceClient>,
}

#[derive_ReprC]
#[repr(C)]
pub struct KConnectConnectivitySignal {
    pub id: char_p::Box,
    pub signal_type: char_p::Box,
    pub strength: i32,
}

#[derive_ReprC]
#[repr(C)]
pub struct KConnectVolumeStream {
    pub name: char_p::Box,
    pub description: char_p::Box,

    pub has_enabled: bool,
    pub enabled: bool,

    pub muted: bool,

    pub has_max_volume: bool,
    pub max_volume: i32,

    pub volume: i32,
}

#[derive_ReprC]
#[repr(u8)]
pub enum KConnectMprisLoopStatus {
    None,
    Track,
    Playlist,
}

impl From<MprisLoopStatus> for KConnectMprisLoopStatus {
    fn from(value: MprisLoopStatus) -> Self {
        match value {
            MprisLoopStatus::None => KConnectMprisLoopStatus::None,
            MprisLoopStatus::Track => KConnectMprisLoopStatus::Track,
            MprisLoopStatus::Playlist => KConnectMprisLoopStatus::Playlist,
        }
    }
}

#[derive_ReprC]
#[repr(C)]
pub struct KConnectMprisPlayer {
    pub player: char_p::Box,
    pub title: char_p::Box,
    pub artist: char_p::Box,
    pub album: char_p::Box,
    pub is_playing: bool,
    pub can_pause: bool,
    pub can_play: bool,
    pub can_go_next: bool,
    pub can_go_previous: bool,
    pub can_seek: bool,
    pub loop_status: KConnectMprisLoopStatus,
    pub shuffle: bool,
    pub pos: i32,
    pub length: i32,
    pub volume: i32,
    pub album_art_url: char_p::Box,
    // undocumented kdeconnect-kde field
    pub url: char_p::Box,
}

#[derive_ReprC]
#[repr(u8)]
pub enum KConnectMprisPlayerAction {
    Seek,
    Volume,
    LoopStatusNone,
    LoopStatusTrack,
    LoopStatusPlaylist,
    Position,
    Shuffle,
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
}

#[derive_ReprC]
#[repr(C)]
pub struct KConnectMousepadRequest<'a> {
    pub key: char_p::Ref<'a>,
    pub special_key: u8,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,

    pub dx: f32,
    pub dy: f32,
    pub scroll: bool,
    pub singleclick: bool,
    pub doubleclick: bool,
    pub middleclick: bool,
    pub rightclick: bool,
    pub singlehold: bool,
    pub singlerelease: bool,

    pub send_ack: bool,
}

#[derive_ReprC]
#[repr(C)]
pub struct KConnectCommand {
    pub id: char_p::Box,
    pub name: char_p::Box,
    pub command: char_p::Box,
}
