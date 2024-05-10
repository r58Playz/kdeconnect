use std::{error::Error, ffi::OsStr, path::PathBuf, pin::Pin, sync::Arc, time::SystemTime};

use async_trait::async_trait;
use kdeconnect::{
    device::{DeviceClient, DeviceConfig, DeviceHandler},
    packets::{
        Battery, ConnectivityReport, DeviceType, Ping, Presenter, ShareRequest, ShareRequestUpdate,
        SystemVolume, SystemVolumeRequest, SystemVolumeStream,
    },
    KdeConnectError,
};
use log::{error, info};
use safer_ffi::prelude::*;
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWriteExt},
    sync::Mutex,
};

use crate::{call_callback, call_callback_no_ret, STATE};

#[derive(Default)]
pub struct KConnectDeviceState {
    pub battery: Option<Battery>,
    pub clipboard: Option<String>,
    pub connectivity: Option<ConnectivityReport>,
    pub systemvolume: Option<Vec<SystemVolumeStream>>,
}

pub struct KConnectDevice {
    pub client: Arc<DeviceClient>,
    pub config: DeviceConfig,
    pub state: Arc<Mutex<KConnectDeviceState>>,
}

pub struct KConnectHandler {
    state: Arc<Mutex<KConnectDeviceState>>,
    config: DeviceConfig,
    id: char_p::Box,
    verification_key: char_p::Box,
    documents_path: PathBuf,
}

impl KConnectHandler {
    pub fn new(
        state: Arc<Mutex<KConnectDeviceState>>,
        mut config: DeviceConfig,
        verification_key: String,
        documents_path: PathBuf,
    ) -> Self {
        // we don't need the cert
        config.certificate.take();
        Self {
            state,
            // this should never fail
            id: config.id.clone().try_into().unwrap(),
            config,
            // this should never fail
            verification_key: verification_key.try_into().unwrap(),
            documents_path,
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
        if let Some(streams) = packet.sink_list {
            self.state.lock().await.systemvolume.replace(streams);
        } else if let Some(name) = packet.name {
            let mut state = self.state.lock().await;
            if let Some(stream) = state
                .systemvolume
                .as_mut()
                .and_then(|x| x.iter_mut().find(|x| x.name == name))
            {
                if let Some(enabled) = packet.enabled {
                    stream.enabled = Some(enabled);
                }
                if let Some(muted) = packet.muted {
                    stream.muted = muted;
                }
                if let Some(volume) = packet.volume {
                    stream.volume = volume;
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
        packet: ShareRequest,
        _size: i64,
        mut data: Pin<Box<dyn AsyncRead + Sync + Send>>,
    ) {
        let ret = async {
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("time went backwards")
                .as_millis();
            let mut path = self
                .documents_path
                .join(packet.filename.unwrap_or(current_time.to_string()));
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
            let _ = file.shutdown().await;
            let _ = file.sync_all().await;
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
