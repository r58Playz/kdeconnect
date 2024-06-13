#![feature(once_cell_try, let_chains)]
#[macro_use]
mod utils;
mod callbacks;
mod device;

use std::{
    collections::HashMap,
    error::Error,
    io,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use callbacks::KConnectCallbacks;
use device::{
    KConnectCommand, KConnectConnectivitySignal, KConnectDevice, KConnectDeviceState,
    KConnectFfiDevice, KConnectFfiDeviceInfo, KConnectFfiDeviceState, KConnectFfiDeviceType,
    KConnectHandler, KConnectMousepadRequest, KConnectMprisPlayer, KConnectMprisPlayerAction,
    KConnectVolumeStream,
};
use kdeconnect::{
    config::FsConfig,
    device::DeviceFile,
    packets::{
        Battery, BatteryRequest, Clipboard, ClipboardConnect, ConnectivityReport,
        ConnectivityReportNetworkType, ConnectivityReportRequest, ConnectivityReportSignal,
        FindPhone, MousepadEcho, MousepadKeyboardState, MousepadRequest, MousepadSpecialKey, Mpris,
        MprisAction, MprisLoopStatus, MprisPlayer, MprisRequest, MprisRequestAction, Ping,
        Presenter, RunCommand, RunCommandRequest, ShareRequest, SystemVolume, SystemVolumeRequest,
    },
    KdeConnect, KdeConnectClient, KdeConnectError,
};
use log::info;
#[cfg(target_os = "ios")]
use log::LevelFilter;
#[cfg(target_os = "ios")]
use oslog::OsLogger;
use safer_ffi::{boxed::Box_, ffi_export, prelude::*};
#[cfg(target_os = "ios")]
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use tokio::{runtime::Runtime, sync::Mutex};
use tokio_stream::StreamExt;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static STATE: Mutex<Option<KConnectState>> = Mutex::const_new(None);
static CALLBACKS: Mutex<KConnectCallbacks> = Mutex::const_new(KConnectCallbacks::new());

struct KConnectState {
    client: KdeConnectClient,
    config: Arc<FsConfig>,
    devices: Vec<KConnectDevice>,
    current_battery: Battery,
    current_clipboard: String,
    current_signals: HashMap<String, ConnectivityReportSignal>,
    current_volume: i32,
    current_player: Option<MprisPlayer>,
    being_found: bool,
}

impl KConnectState {
    pub fn new(client: KdeConnectClient, config: Arc<FsConfig>) -> Self {
        Self {
            client,
            config,
            devices: Vec::new(),
            current_battery: Battery {
                charge: -1,
                is_charging: false,
                under_threshold: false,
            },
            current_clipboard: String::new(),
            current_signals: HashMap::new(),
            current_volume: 0,
            current_player: None,
            being_found: false,
        }
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_init() -> bool {
    #[cfg(target_os = "ios")]
    {
        let oslog = utils::IosLogWrapper(
            OsLogger::new("dev.r58playz.kdeconnectjb").level_filter(LevelFilter::Debug),
            LevelFilter::Debug,
        );
        let stdoutlog = TermLogger::new(
            LevelFilter::Debug,
            Config::default(),
            TerminalMode::Stdout,
            ColorChoice::Auto,
        );
        CombinedLogger::init(vec![Box::new(oslog), stdoutlog]).is_ok()
    }
    #[cfg(not(target_os = "ios"))]
    false
}

#[ffi_export]
pub extern "C" fn kdeconnect_start(
    device_id: char_p::Ref<'_>,
    device_name: char_p::Ref<'_>,
    device_type: KConnectFfiDeviceType,
    config_path: char_p::Ref<'_>,
    documents_path: char_p::Ref<'_>,
) -> bool {
    let documents_path = PathBuf::from(documents_path.to_string());
    if let Ok(rt) = build_runtime!() {
        let ret = rt.block_on(async move {
            if STATE.lock().await.is_some() {
                return Err::<(), Box<dyn Error + Sync + Send>>(Box::new(io::Error::other(
                    "Already started",
                )));
            }

            let config_provider = Arc::new(
                FsConfig::new(
                    config_path.to_string().into(),
                    "server_cert".into(),
                    "server_keypair".into(),
                    "devices".into(),
                )
                .await?,
            );
            let (kdeconnect, client, mut device_stream) = KdeConnect::new(
                device_id.to_string(),
                device_name.to_string(),
                device_type.into(),
                vec![
                    Ping::TYPE.to_string(),
                    Battery::TYPE.to_string(),
                    BatteryRequest::TYPE.to_string(),
                    Clipboard::TYPE.to_string(),
                    ClipboardConnect::TYPE.to_string(),
                    FindPhone::TYPE.to_string(),
                    ConnectivityReport::TYPE.to_string(),
                    ConnectivityReportRequest::TYPE.to_string(),
                    SystemVolume::TYPE.to_string(),
                    SystemVolumeRequest::TYPE.to_string(),
                    ShareRequest::TYPE.to_string(),
                    Mpris::TYPE.to_string(),
                    MprisRequest::TYPE.to_string(),
                    MousepadEcho::TYPE.to_string(),
                    MousepadKeyboardState::TYPE.to_string(),
                ],
                vec![
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
                    RunCommand::TYPE.to_string(),
                    RunCommandRequest::TYPE.to_string(),
                ],
                config_provider.clone(),
            )
            .await?;

            STATE
                .lock()
                .await
                .replace(KConnectState::new(client, config_provider));

            info!("created kdeconnect client");

            tokio::spawn(async move {
                info!(
                    "kdeconnect server ret {:?}",
                    kdeconnect.start_server().await
                )
            });

            call_callback_no_ret!(initialized,);

            info!("discovering");
            while let Some((mut dev, client)) = device_stream.next().await
                && let Ok(key) = dev.get_verification_key().await
            {
                info!(
                    "new device discovered: id {:?} name {:?} type {:?}",
                    dev.config.id, dev.config.name, dev.config.device_type
                );
                let state = Arc::new(Mutex::new(KConnectDeviceState::default()));
                let client = Arc::new(client);
                let config = dev.config.clone();

                #[allow(clippy::redundant_closure)]
                let handler = Box::new(KConnectHandler::new(
                    state.clone(),
                    client.clone(),
                    dev.config.clone(),
                    key,
                    documents_path.clone(),
                ));

                // this should never fail
                let id = dev.config.id.clone().try_into().unwrap();

                tokio::spawn(async move {
                    info!("handler task exited: {:?}", dev.task(handler).await);
                });

                // STATE will always be Some
                STATE
                    .lock()
                    .await
                    .as_mut()
                    .unwrap()
                    .devices
                    .push(KConnectDevice {
                        client,
                        state,
                        config,
                    });

                call_callback_no_ret!(discovered, id);
            }

            Ok::<(), Box<dyn Error + Sync + Send>>(())
        });
        info!("runtime ret {:?}", ret);

        ret.is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_free_string(str: char_p::Box) {
    drop(str)
}

#[ffi_export]
pub extern "C" fn kdeconnect_broadcast_identity() -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            STATE
                .lock()
                .await
                .as_ref()
                .ok_or(KdeConnectError::Other)?
                .client
                .broadcast_identity()
                .await
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_get_is_lost() -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            STATE
                .lock()
                .await
                .as_ref()
                .map(|x| x.being_found)
                .and_then(|x| if x { Some(()) } else { None })
        })
        .is_some()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_set_is_lost(is_lost: bool) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            STATE.lock().await.as_mut()?.being_found = is_lost;
            Some(())
        })
        .is_some()
    } else {
        false
    }
}

/// Device lists must be freed with kdeconnect_free_paired_device_list. Calling kdeconnect_free_device to
/// free a device from a device list is UB.
#[ffi_export]
pub extern "C" fn kdeconnect_get_paired_device_list() -> repr_c::Vec<KConnectFfiDeviceInfo> {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let devices = STATE
                .lock()
                .await
                .as_ref()
                .ok_or(KdeConnectError::Other)?
                .config
                .retrieve_all_device_configs()
                .await?;

            let mut out = Vec::new();

            for device in devices.iter().filter(|x| x.is_paired()) {
                out.push(KConnectFfiDeviceInfo {
                    dev_type: device.device_type.into(),
                    // this should never fail
                    id: device.id.clone().try_into().unwrap(),
                    // this should never fail
                    name: device.name.clone().try_into().unwrap(),
                })
            }

            Ok::<Vec<KConnectFfiDeviceInfo>, KdeConnectError>(out)
        })
        .unwrap_or_default()
        .into()
    } else {
        vec![].into()
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_free_paired_device_list(devices: repr_c::Vec<KConnectFfiDeviceInfo>) {
    drop(devices);
}

/// Device lists must be freed with kdeconnect_free_connected_device_list. Calling kdeconnect_free_device to
/// free a device from a device list is UB.
#[ffi_export]
pub extern "C" fn kdeconnect_get_connected_device_list() -> repr_c::Vec<KConnectFfiDevice> {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;

            let mut out = Vec::new();

            for device in state.devices.iter() {
                out.push(KConnectFfiDevice {
                    dev_type: device.config.device_type.into(),
                    // this should never fail
                    id: device.config.id.clone().try_into().unwrap(),
                    // this should never fail
                    name: device.config.name.clone().try_into().unwrap(),
                    state: Box_::new(KConnectFfiDeviceState {
                        state: device.state.clone(),
                        client: device.client.clone(),
                    }),
                })
            }

            Ok::<Vec<KConnectFfiDevice>, KdeConnectError>(out)
        })
        .unwrap_or_default()
        .into()
    } else {
        vec![].into()
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_free_connected_device_list(devices: repr_c::Vec<KConnectFfiDevice>) {
    drop(devices);
}

#[ffi_export]
pub extern "C" fn kdeconnect_get_device_by_id(id: char_p::Ref<'_>) -> *mut KConnectFfiDevice {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;

            let mut out = None;

            for device in state.devices.iter() {
                if device.config.id == id.to_str() {
                    out.replace(KConnectFfiDevice {
                        dev_type: device.config.device_type.into(),
                        // this should never fail
                        id: device.config.id.clone().try_into().unwrap(),
                        // this should never fail
                        name: device.config.name.clone().try_into().unwrap(),
                        state: Box_::new(KConnectFfiDeviceState {
                            state: device.state.clone(),
                            client: device.client.clone(),
                        }),
                    });
                }
            }

            Ok::<*mut KConnectFfiDevice, KdeConnectError>(Box::into_raw(Box::new(
                out.ok_or(KdeConnectError::Other)?,
            )))
        })
        .unwrap_or(std::ptr::null_mut())
    } else {
        std::ptr::null_mut()
    }
}

/// # Safety
/// Must be valid pointer
#[ffi_export]
pub unsafe extern "C" fn kdeconnect_free_device(device: *mut KConnectFfiDevice) {
    if !device.is_null() {
        drop(Box::from_raw(device));
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_get_battery_level(device: &KConnectFfiDevice) -> i32 {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .state
                .lock()
                .await
                .battery
                .map(|x| x.charge)
                .unwrap_or(-1)
        })
    } else {
        -1
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_get_battery_charging(device: &KConnectFfiDevice) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .state
                .lock()
                .await
                .battery
                .map(|x| x.is_charging)
                .unwrap_or(false)
        })
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_get_battery_under_threshold(
    device: &KConnectFfiDevice,
) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .state
                .lock()
                .await
                .battery
                .map(|x| x.under_threshold)
                .unwrap_or(false)
        })
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_get_clipboard_content(
    device: &KConnectFfiDevice,
) -> char_p::Box {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .state
                .lock()
                .await
                .clipboard
                .clone()
                .unwrap_or("".to_string())
        })
        .try_into()
        .unwrap()
    } else {
        "".to_string().try_into().unwrap()
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_get_connectivity_report(
    device: &KConnectFfiDevice,
) -> repr_c::Vec<KConnectConnectivitySignal> {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut out = Vec::new();

            if let Some(connectivity) = device.state.state.lock().await.connectivity.as_ref() {
                for signal in connectivity.signal_strengths.iter() {
                    out.push(KConnectConnectivitySignal {
                        // this should never fail
                        id: signal.0.clone().try_into().unwrap(),
                        // this should never fail
                        signal_type: signal.1.network_type.to_string().try_into().unwrap(),
                        strength: signal.1.signal_strength,
                    })
                }
            }

            Ok::<Vec<_>, KdeConnectError>(out)
        })
        .unwrap_or_default()
        .into()
    } else {
        vec![].into()
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_free_connectivity_report(
    report: repr_c::Vec<KConnectConnectivitySignal>,
) {
    drop(report);
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_send_ping(device: &KConnectFfiDevice) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.send_ping(None).await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_is_paired(device: &KConnectFfiDevice) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device.state.client.is_paired().await.and_then(|x| {
                if x {
                    Ok(())
                } else {
                    Err(KdeConnectError::Other)
                }
            })
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_pair(device: &KConnectFfiDevice, state: bool) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.change_pair_state(state).await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_send_find(device: &KConnectFfiDevice) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.toggle_find_phone().await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_send_presenter(
    device: &KConnectFfiDevice,
    dx: f32,
    dy: f32,
) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .client
                .send_presenter_update(Presenter::Move { dx, dy })
                .await
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_stop_presenter(device: &KConnectFfiDevice) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .client
                .send_presenter_update(Presenter::Stop { stop: true })
                .await
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_request_volume(device: &KConnectFfiDevice) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.request_volume_list().await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_get_volume(
    device: &KConnectFfiDevice,
) -> repr_c::Vec<KConnectVolumeStream> {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut out = Vec::new();

            if let Some(volume) = device.state.state.lock().await.systemvolume.as_ref() {
                for stream in volume.iter() {
                    out.push(KConnectVolumeStream {
                        // this should never fail
                        name: stream.name.clone().try_into().unwrap(),
                        // this should never fail
                        description: stream.description.clone().try_into().unwrap(),

                        has_enabled: stream.enabled.is_some(),
                        enabled: stream.enabled.unwrap_or(false),

                        has_max_volume: stream.max_volume.is_some(),
                        max_volume: stream.max_volume.unwrap_or(-1),

                        muted: stream.muted,
                        volume: stream.volume,
                    });
                }
            }

            Ok::<Vec<_>, KdeConnectError>(out)
        })
        .unwrap_or_default()
        .into()
    } else {
        vec![].into()
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_free_volume(report: repr_c::Vec<KConnectVolumeStream>) {
    drop(report);
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_send_volume_update(
    device: &KConnectFfiDevice,
    name: char_p::Ref<'_>,
    enabled: bool,
    muted: bool,
    volume: i32,
) -> bool {
    let name = name.to_string();
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .client
                .send_volume_request(name, Some(enabled), Some(muted), Some(volume))
                .await
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_share_text(
    device: &KConnectFfiDevice,
    text: char_p::Ref<'_>,
) -> bool {
    let text = text.to_string();
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.share_text(text).await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_share_url(
    device: &KConnectFfiDevice,
    url: char_p::Ref<'_>,
) -> bool {
    let url = url.to_string();
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.share_url(url).await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_share_file(
    device: &KConnectFfiDevice,
    path: char_p::Ref<'_>,
    open: bool,
) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .client
                .share_file(DeviceFile::open(path.to_str()).await?, open)
                .await
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_share_files(
    device: &KConnectFfiDevice,
    paths: c_slice::Ref<'_, char_p::Ref<'_>>,
    open: bool,
) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let slice = paths.as_slice();
            let mut paths = Vec::with_capacity(slice.len());
            for path in slice {
                paths.push(DeviceFile::open(path.to_str()).await?);
            }
            device.state.client.share_files(paths, open).await
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_request_players(device: &KConnectFfiDevice) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.request_mpris_list().await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_request_player(
    device: &KConnectFfiDevice,
    player: char_p::Ref<'_>,
) -> bool {
    let player = player.to_string();
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.request_mpris_info(player, None).await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_get_players(
    device: &KConnectFfiDevice,
) -> repr_c::Vec<KConnectMprisPlayer> {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut out = Vec::new();

            for stream in device
                .state
                .state
                .lock()
                .await
                .players
                .values()
                .map(|x| (x.0.clone(), x.1.clone()))
            {
                let MprisPlayer {
                    player,
                    title,
                    artist,
                    album,
                    is_playing,
                    can_pause,
                    can_play,
                    can_go_next,
                    can_go_previous,
                    can_seek,
                    loop_status,
                    shuffle,
                    pos,
                    length,
                    volume,
                    album_art_url: _,
                    url,
                } = stream.0;

                out.push(KConnectMprisPlayer {
                    player: player.try_into().unwrap(),
                    title: title.unwrap_or("".to_string()).try_into().unwrap(),
                    artist: artist.unwrap_or("".to_string()).try_into().unwrap(),
                    album: album.unwrap_or("".to_string()).try_into().unwrap(),
                    is_playing: is_playing.unwrap_or(false),
                    can_pause: can_pause.unwrap_or(false),
                    can_play: can_play.unwrap_or(false),
                    can_go_next: can_go_next.unwrap_or(false),
                    can_go_previous: can_go_previous.unwrap_or(false),
                    can_seek: can_seek.unwrap_or(false),
                    loop_status: loop_status.unwrap_or(MprisLoopStatus::None).into(),
                    shuffle: shuffle.unwrap_or(false),
                    pos: pos.unwrap_or(-1),
                    length: length.unwrap_or(-1),
                    volume: volume.unwrap_or(100),
                    album_art_url: stream.1.unwrap_or("".to_string()).try_into().unwrap(),
                    url: url.unwrap_or("".to_string()).try_into().unwrap(),
                });
            }

            Ok::<Vec<_>, KdeConnectError>(out)
        })
        .unwrap_or_default()
        .into()
    } else {
        vec![].into()
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_free_players(report: repr_c::Vec<KConnectMprisPlayer>) {
    drop(report);
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_request_player_action(
    device: &KConnectFfiDevice,
    player: char_p::Ref<'_>,
    action: KConnectMprisPlayerAction,
    val: i64,
) -> bool {
    let player = player.to_string();
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            use KConnectMprisPlayerAction as A;
            let action = match action {
                A::Seek => MprisRequestAction {
                    player,
                    seek: Some(val),
                    ..Default::default()
                },
                A::Volume => MprisRequestAction {
                    player,
                    set_volume: Some(val),
                    ..Default::default()
                },
                A::LoopStatusNone => MprisRequestAction {
                    player,
                    set_loop_status: Some(MprisLoopStatus::None),
                    ..Default::default()
                },
                A::LoopStatusTrack => MprisRequestAction {
                    player,
                    set_loop_status: Some(MprisLoopStatus::Track),
                    ..Default::default()
                },
                A::LoopStatusPlaylist => MprisRequestAction {
                    player,
                    set_loop_status: Some(MprisLoopStatus::Playlist),
                    ..Default::default()
                },
                A::Position => MprisRequestAction {
                    player,
                    set_position: Some(val),
                    ..Default::default()
                },
                A::Shuffle => MprisRequestAction {
                    player,
                    set_shuffle: Some(val == 1),
                    ..Default::default()
                },
                A::Play => MprisRequestAction {
                    player,
                    action: Some(MprisAction::Play),
                    ..Default::default()
                },
                A::Pause => MprisRequestAction {
                    player,
                    action: Some(MprisAction::Pause),
                    ..Default::default()
                },
                A::PlayPause => MprisRequestAction {
                    player,
                    action: Some(MprisAction::PlayPause),
                    ..Default::default()
                },
                A::Stop => MprisRequestAction {
                    player,
                    action: Some(MprisAction::Stop),
                    ..Default::default()
                },
                A::Next => MprisRequestAction {
                    player,
                    action: Some(MprisAction::Next),
                    ..Default::default()
                },
                A::Previous => MprisRequestAction {
                    player,
                    action: Some(MprisAction::Previous),
                    ..Default::default()
                },
            };
            device.state.client.request_mpris_action(action).await
        })
        .is_ok()
    } else {
        false
    }
}

fn translate_u8_to_special_key(key: u8) -> Option<MousepadSpecialKey> {
    use MousepadSpecialKey as K;
    match key {
        1 => Some(K::Backspace),
        2 => Some(K::Tab),
        4 => Some(K::DpadLeft),
        5 => Some(K::DpadUp),
        6 => Some(K::DpadRight),
        7 => Some(K::DpadDown),
        8 => Some(K::PageUp),
        9 => Some(K::PageDown),
        10 => Some(K::Home),
        11 => Some(K::End),
        12 => Some(K::Enter),
        13 => Some(K::Delete),
        14 => Some(K::Escape),
        15 => Some(K::SysRq),
        16 => Some(K::ScrollLock),
        21 => Some(K::F1),
        22 => Some(K::F2),
        23 => Some(K::F3),
        24 => Some(K::F4),
        25 => Some(K::F5),
        26 => Some(K::F6),
        27 => Some(K::F7),
        28 => Some(K::F8),
        29 => Some(K::F9),
        30 => Some(K::F10),
        31 => Some(K::F11),
        32 => Some(K::F12),
        _ => None,
    }
}

fn bool_to_option(bool: bool) -> Option<bool> {
    if bool {
        Some(true)
    } else {
        None
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_request_mousepad(
    device: &KConnectFfiDevice,
    mousepad: KConnectMousepadRequest<'_>,
) -> bool {
    let key = mousepad.key.to_string();
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            device
                .state
                .client
                .request_mousepad_action(MousepadRequest {
                    key: if key.is_empty() { None } else { Some(key) },
                    special_key: translate_u8_to_special_key(mousepad.special_key),
                    alt: bool_to_option(mousepad.alt),
                    ctrl: bool_to_option(mousepad.ctrl),
                    shift: bool_to_option(mousepad.shift),
                    dx: if mousepad.dx == 0.0 {
                        None
                    } else {
                        Some(mousepad.dx)
                    },
                    dy: if mousepad.dy == 0.0 {
                        None
                    } else {
                        Some(mousepad.dy)
                    },
                    scroll: bool_to_option(mousepad.scroll),
                    singleclick: bool_to_option(mousepad.singleclick),
                    doubleclick: bool_to_option(mousepad.doubleclick),
                    middleclick: bool_to_option(mousepad.middleclick),
                    rightclick: bool_to_option(mousepad.rightclick),
                    singlehold: bool_to_option(mousepad.singlehold),
                    singlerelease: bool_to_option(mousepad.singlerelease),
                    send_ack: bool_to_option(mousepad.send_ack),
                })
                .await
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_request_commands(device: &KConnectFfiDevice) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.request_command_list().await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_get_commands(
    device: &KConnectFfiDevice,
) -> repr_c::Vec<KConnectCommand> {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut out = Vec::new();

            for command in device.state.state.lock().await.commands.iter() {
                out.push(KConnectCommand {
                    id: command.0.clone().try_into().unwrap(),
                    name: command.1.name.clone().try_into().unwrap(),
                    command: command.1.command.clone().try_into().unwrap(),
                });
            }

            Ok::<Vec<_>, KdeConnectError>(out)
        })
        .unwrap_or_default()
        .into()
    } else {
        vec![].into()
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_device_run_command(
    device: &KConnectFfiDevice,
    id: char_p::Ref<'_>,
) -> bool {
    let id = id.to_string();
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async { device.state.client.run_command(id).await })
            .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_on_battery_event(
    level: i32,
    charging: i32,
    within_threshold: i32,
) -> bool {
    let is_charging = charging == 1;
    let under_threshold = within_threshold == 1;
    info!(
        "recieved battery event: {:?}, {:?}, {:?}",
        level, is_charging, under_threshold
    );

    let battery_state = Battery {
        charge: level,
        is_charging,
        under_threshold,
    };

    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;
            state.current_battery = battery_state;
            for device in state.devices.iter() {
                let _ = device.client.send_battery_update(battery_state).await;
            }
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_on_clipboard_event(content: char_p::Ref<'_>) -> bool {
    let content = content.to_string();

    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;
            state.current_clipboard.clone_from(&content);

            for device in state.devices.iter() {
                let _ = device.client.send_clipboard_update(content.clone()).await;
            }
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

fn string_to_connectivity_network_type(net_type: &str) -> ConnectivityReportNetworkType {
    use ConnectivityReportNetworkType as C;
    match net_type {
        "kCTRadioAccessTechnologyGPRS" => C::Gprs,
        "kCTRadioAccessTechnologyEdge" => C::Edge,
        // "kCTRadioAccessTechnologyWCDMA" => doesn't match any
        "kCTRadioAccessTechnologyHSDPA" => C::Hspa,
        "kCTRadioAccessTechnologyHSUPA" => C::Hspa,
        "kCTRadioAccessTechnologyCDMA1x" => C::Cdma,
        // "kCTRadioAccessTechnologyCDMAEVDORev0" => doesn't match any
        // "kCTRadioAccessTechnologyCDMAEVDORevA" => doesn't match any
        // "kCTRadioAccessTechnologyCDMAEVDORevB" => doesn't match any
        // "kCTRadioAccessTechnologyeHRPD" => doesn't match any
        "kCTRadioAccessTechnologyLTE" => C::Lte,
        "kCTRadioAccessTechnologyNR" => C::FiveG,
        "kCTRadioAccessTechnologyNRNSA" => C::FiveG,
        _ => C::Unknown,
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_clear_connectivity_signals() -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            STATE
                .lock()
                .await
                .as_mut()
                .ok_or(KdeConnectError::Other)?
                .current_signals
                .clear();
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_add_connectivity_signal(
    id: char_p::Ref<'_>,
    net_type: char_p::Ref<'_>,
    signal: i32,
) -> bool {
    let id = id.to_string();
    let net_type = net_type.to_string();

    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            STATE
                .lock()
                .await
                .as_mut()
                .ok_or(KdeConnectError::Other)?
                .current_signals
                .insert(
                    id,
                    ConnectivityReportSignal {
                        network_type: string_to_connectivity_network_type(&net_type),
                        signal_strength: signal,
                    },
                );
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_send_connectivity_update() -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;

            for device in state.devices.iter() {
                let _ = device
                    .client
                    .send_connectivity_report(ConnectivityReport {
                        signal_strengths: state.current_signals.clone(),
                    })
                    .await;
            }
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_set_volume(vol: i32) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;

            state.current_volume = vol;

            for device in state.devices.iter() {
                let _ = device
                    .client
                    .send_volume_stream_update(
                        "coreaudio".to_string(),
                        Some(true),
                        Some(vol == 0),
                        Some(vol),
                    )
                    .await;
            }
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_remove_player() -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;
            state.current_player = None;
            for device in state.devices.iter() {
                let _ = device.client.send_mpris_list(vec![]).await;
            }
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_add_player(
    title: char_p::Ref<'_>,
    artist: char_p::Ref<'_>,
    album: char_p::Ref<'_>,
    is_playing: bool,
    pos: i32,
    len: i32,
    album_art: char_p::Ref<'_>,
) -> bool {
    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let album_art = album_art.to_str();
            let title = title.to_string();
            let artist = artist.to_string();
            let album = album.to_string();
            let player = MprisPlayer {
                player: "iPhone".to_string(),
                title: if title.is_empty() { None } else { Some(title) },
                artist: if artist.is_empty() {
                    None
                } else {
                    Some(artist)
                },
                album: if album.is_empty() { None } else { Some(album) },
                is_playing: Some(is_playing),
                pos: Some(pos),
                length: Some(len),
                album_art_url: if album_art.is_empty() {
                    None
                } else {
                    Some("file://".to_string() + album_art)
                },

                can_pause: Some(true),
                can_play: Some(true),
                can_go_next: Some(true),
                can_go_previous: Some(true),
                can_seek: Some(true),
                // ios doesn't provide shuffle afaik (@bomberfish please fix)
                shuffle: Some(false),
                // nor does it provide repeat (@bomberfish please fix)
                loop_status: Some(MprisLoopStatus::None),
                volume: Some(100),
                url: None,
            };
            info!("player: {:?}", player);
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;
            state.current_player = Some(player);
            for device in state.devices.iter() {
                let _ = device
                    .client
                    .send_mpris_list(vec!["iPhone".to_string()])
                    .await;
            }
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

#[cfg(feature = "headers")]
pub fn generate_headers() -> io::Result<()> {
    let builder = safer_ffi::headers::builder();
    if let Some(filename) = std::env::args_os().nth(1) {
        builder.to_file(&filename)?.generate()
    } else {
        builder.to_writer(std::io::stdout()).generate()
    }
}
