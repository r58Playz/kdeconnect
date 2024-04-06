#![feature(once_cell_try)]
#[macro_use]
mod utils;
mod device;

use std::{
    error::Error,
    io,
    sync::{Arc, OnceLock},
};

use device::{
    KConnectDevice, KConnectDeviceState, KConnectFfiDevice, KConnectFfiDeviceState,
    KConnectFfiDeviceType, KConnectHandler,
};
use kdeconnect::{
    config::FsConfig, packets::Battery, KdeConnect, KdeConnectClient, KdeConnectError,
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
    devices: Vec<KConnectDevice>,
    current_battery: Battery,
    current_clipboard: String,
}

impl KConnectState {
    pub fn new(client: KdeConnectClient) -> Self {
        Self {
            client,
            devices: Vec::new(),
            current_battery: Battery {
                charge: -1,
                is_charging: false,
                under_threshold: false,
            },
            current_clipboard: String::new(),
        }
    }
}

struct KConnectCallbacks {
    pub initialized: Option<Arc<dyn Fn() + Sync + Send>>,
    pub discovered: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub ping_recieved: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub pair_status_changed: Option<Arc<dyn Fn(char_p::Box, bool) + Sync + Send>>,
    pub battery_changed: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub clipboard_changed: Option<Arc<dyn Fn(char_p::Box, char_p::Box) + Sync + Send>>,
    pub pairing_requested: Option<Arc<dyn Fn(char_p::Box) -> bool + Sync + Send>>,
}

impl KConnectCallbacks {
    pub const fn new() -> Self {
        Self {
            initialized: None,
            discovered: None,
            ping_recieved: None,
            pair_status_changed: None,
            battery_changed: None,
            clipboard_changed: None,
            pairing_requested: None,
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
    extern "C" fn(char_p::Box) -> bool,
    pairing_requested,
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
) -> bool {
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
                )
                .await?,
            );
            let (kdeconnect, client, mut device_stream) = KdeConnect::new(
                device_id.to_string(),
                device_name.to_string(),
                device_type.into(),
                config_provider,
            )
            .await?;

            STATE.lock().await.replace(KConnectState::new(client));

            info!("created kdeconnect client");

            tokio::spawn(async move { kdeconnect.start_server().await });

            call_callback_no_ret!(initialized,);

            info!("discovering");
            while let Some((mut dev, client)) = device_stream.next().await {
                info!(
                    "new device discovered: id {:?} name {:?} type {:?}",
                    dev.config.id, dev.config.name, dev.config.device_type
                );
                let state = Arc::new(Mutex::new(KConnectDeviceState::default()));
                let config = dev.config.clone();

                #[allow(clippy::redundant_closure)]
                let handler = Box::new(KConnectHandler::new(state.clone(), dev.config.clone()));

                // this should never fail
                let id = dev.config.id.clone().try_into().unwrap();

                tokio::spawn(async move { dev.task(handler).await });
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

/// Device lists must be freed with kdeconnect_free_device_list. Calling kdeconnect_free_device to
/// free a device from a device list is UB.
#[ffi_export]
pub extern "C" fn kdeconnect_get_device_list() -> repr_c::Vec<KConnectFfiDevice> {
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
pub extern "C" fn kdeconnect_free_device_list(devices: repr_c::Vec<KConnectFfiDevice>) {
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
        rt.block_on(async { device.state.state.lock().await.battery_level.unwrap_or(-1) })
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
                .battery_charging
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
                .battery_under_threshold
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
                device.client.send_battery_update(battery_state).await?;
            }
            Ok::<(), KdeConnectError>(())
        })
        .is_ok()
    } else {
        false
    }
}

#[ffi_export]
pub extern "C" fn kdeconnect_on_clipboard_event(content: char_p::Box) -> bool {
    info!("recieved clipboard data: {:?}", content);

    let content = content.into_string();

    if let Ok(rt) = build_runtime!() {
        rt.block_on(async {
            let mut locked = STATE.lock().await;
            let state = locked.as_mut().ok_or(KdeConnectError::Other)?;
            state.current_clipboard.clone_from(&content);

            for device in state.devices.iter() {
                device.client.send_clipboard_update(content.clone()).await?;
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
