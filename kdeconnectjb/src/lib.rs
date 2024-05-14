#![feature(once_cell_try, let_chains)]
#[macro_use]
mod utils;
mod device;

use std::{
    collections::HashMap,
    error::Error,
    io,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use device::{
    KConnectConnectivitySignal, KConnectDevice, KConnectDeviceState, KConnectFfiDevice,
    KConnectFfiDeviceInfo, KConnectFfiDeviceState, KConnectFfiDeviceType, KConnectHandler,
    KConnectVolumeStream,
};
use kdeconnect::{
    config::FsConfig,
    device::DeviceFile,
    packets::{
        Battery, ConnectivityReport, ConnectivityReportNetworkType, ConnectivityReportSignal,
        MprisLoopStatus, MprisPlayer, Presenter,
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
use tokio::{
    runtime::{Builder, Runtime},
    sync::Mutex,
};
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

struct KConnectCallbacks {
    pub initialized: Option<Arc<dyn Fn() + Sync + Send>>,
    pub discovered: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub gone: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,

    pub ping_recieved: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub pair_status_changed: Option<Arc<dyn Fn(char_p::Box, bool) + Sync + Send>>,
    pub battery_changed: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub clipboard_changed: Option<Arc<dyn Fn(char_p::Box, char_p::Box) + Sync + Send>>,
    pub pairing_requested: Option<Arc<dyn Fn(char_p::Box, char_p::Box) -> bool + Sync + Send>>,
    pub find_requested: Option<Arc<dyn Fn() + Sync + Send>>,
    pub connectivity_changed: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub volume_change_requested: Option<Arc<dyn Fn(i32) + Sync + Send>>,
    pub volume_changed: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub open_file: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub open_url: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub open_text: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
}

impl KConnectCallbacks {
    pub const fn new() -> Self {
        Self {
            initialized: None,
            discovered: None,
            gone: None,

            ping_recieved: None,
            pair_status_changed: None,
            battery_changed: None,
            clipboard_changed: None,
            pairing_requested: None,
            find_requested: None,
            connectivity_changed: None,
            volume_change_requested: None,
            volume_changed: None,
            open_file: None,
            open_url: None,
            open_text: None,
        }
    }
}

#[macro_export]
macro_rules! call_callback {
    ($name:ident, $($args:expr),*) => {
        if let Some(cb) = $crate::CALLBACKS.lock().await.$name.clone() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let _ = tx.send((cb)($($args),*));
            });
            rx.await.ok()
        } else {
            None
        }
    };
}

#[macro_export]
macro_rules! call_callback_no_ret {
    ($name:ident, $($args:expr),*) => {
        if let Some(cb) = $crate::CALLBACKS.lock().await.$name.clone() {
            std::thread::spawn(move || {
                (cb)($($args),*);
            });
        }
    };
}

macro_rules! callback {
    ($name:ident, $type:ty, $var:ident, $($args:expr),*) => {
        #[ffi_export]
        pub extern "C" fn $name(callback: $type) -> bool {
            if let Ok(rt) = build_runtime!() {
                rt.block_on(async {
                    #[allow(clippy::redundant_closure)]
                    CALLBACKS
                        .lock()
                        .await
                        .$var
                        .replace(Arc::new(move |$($args),*| (callback)($($args),*)));
                    true
                })
            } else {
                false
            }
        }
    };
}

callback!(
    kdeconnect_register_init_callback,
    extern "C" fn() -> (),
    initialized,
);

callback!(
    kdeconnect_register_discovered_callback,
    extern "C" fn(char_p::Box) -> (),
    discovered,
    x
);

callback!(
    kdeconnect_register_gone_callback,
    extern "C" fn(char_p::Box) -> (),
    gone,
    x
);

callback!(
    kdeconnect_register_ping_callback,
    extern "C" fn(char_p::Box) -> (),
    ping_recieved,
    x
);

callback!(
    kdeconnect_register_pair_status_changed_callback,
    extern "C" fn(char_p::Box, bool) -> (),
    pair_status_changed,
    x,
    y
);

callback!(
    kdeconnect_register_battery_callback,
    extern "C" fn(char_p::Box) -> (),
    battery_changed,
    x
);

callback!(
    kdeconnect_register_clipboard_callback,
    extern "C" fn(char_p::Box, char_p::Box) -> (),
    clipboard_changed,
    x,
    y
);

callback!(
    kdeconnect_register_pairing_callback,
    extern "C" fn(char_p::Box, char_p::Box) -> bool,
    pairing_requested,
    x,
    y
);

callback!(
    kdeconnect_register_find_callback,
    extern "C" fn() -> (),
    find_requested,
);

callback!(
    kdeconnect_register_connectivity_callback,
    extern "C" fn(char_p::Box) -> (),
    connectivity_changed,
    x
);

callback!(
    kdeconnect_register_device_volume_callback,
    extern "C" fn(char_p::Box) -> (),
    volume_changed,
    x
);

callback!(
    kdeconnect_register_volume_change_callback,
    extern "C" fn(i32) -> (),
    volume_change_requested,
    x
);

callback!(
    kdeconnect_register_open_file_callback,
    extern "C" fn(char_p::Box) -> (),
    open_file,
    x
);

callback!(
    kdeconnect_register_open_url_callback,
    extern "C" fn(char_p::Box) -> (),
    open_url,
    x
);

callback!(
    kdeconnect_register_open_text_callback,
    extern "C" fn(char_p::Box) -> (),
    open_text,
    x
);

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
                config_provider.clone(),
            )
            .await?;

            STATE
                .lock()
                .await
                .replace(KConnectState::new(client, config_provider));

            info!("created kdeconnect client");

            tokio::spawn(async move { kdeconnect.start_server().await });

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
                let _ = device.client.send_mpris_list(vec!["iPhone".to_string()]).await;
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
