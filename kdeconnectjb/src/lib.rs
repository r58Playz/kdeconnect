#![feature(once_cell_try)]
#[macro_use]
mod utils;

use std::{
    error::Error,
    ffi::{c_char, c_double, c_int, CStr},
    io,
    sync::{Arc, OnceLock},
    time::Duration,
};

use async_trait::async_trait;
use kdeconnect::{
    config::FsConfig,
    device::{Device, DeviceClient, DeviceHandler},
    packets::{Battery, DeviceType, Ping},
    KdeConnect, KdeConnectClient, KdeConnectError,
};
use log::{debug, info, warn, LevelFilter};
use oslog::OsLogger;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use tokio::{
    io::{stdin, AsyncReadExt},
    runtime::{Builder, Runtime},
    sync::Mutex,
    time::timeout,
};
use tokio_stream::StreamExt;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static STATE: Mutex<Option<KConnectState>> = Mutex::const_new(None);

struct KConnectIosHandler(pub Arc<Mutex<KConnectDeviceState>>);

#[async_trait]
impl DeviceHandler for KConnectIosHandler {
    async fn handle_ping(&mut self, device: &Device, packet: Ping) {
        warn!(
            "recieved ping: {:?} packet: {:#?}",
            device.config.name, packet
        );
    }

    async fn handle_battery(&mut self, device: &Device, packet: Battery) {
        let mut state = self.0.lock().await;
        state.battery_level.replace(packet.charge);
        state.battery_charging.replace(packet.is_charging);
        state
            .battery_under_threshold
            .replace(packet.under_threshold);
        info!(
            "recieved battery data: {:?} packet: {:#?}",
            device.config.name, packet
        );
    }

    async fn handle_pairing_request(&mut self, device: &Device) -> bool {
        info!("recieved pair from {:?}", device.config);
        let res = timeout(Duration::from_secs(5), stdin().read(&mut [0; 128]))
            .await
            .map_err(io::Error::other)
            .and_then(|x| x)
            .is_ok_and(|x| x > 0);
        warn!(
            "pair {} from {:?}",
            if res { "accepted" } else { "rejected" },
            device.config.name
        );
        res
    }

    async fn get_battery(&mut self, _: &Device) -> Battery {
        debug!("requested battery data");
        // STATE will always be Some here
        STATE.lock().await.as_ref().unwrap().current_battery
    }
}

struct KConnectState {
    client: KdeConnectClient,
    devices: Vec<KConnectDevice>,
    current_battery: Battery,
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
        }
    }
}

#[derive(Default)]
struct KConnectDeviceState {
    battery_level: Option<i32>,
    battery_charging: Option<bool>,
    battery_under_threshold: Option<bool>,
}

struct KConnectDevice {
    client: DeviceClient,
    state: Arc<Mutex<KConnectDeviceState>>,
}

#[no_mangle]
pub extern "C" fn kdeconnect_init() -> bool {
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

#[no_mangle]
/// # Safety
/// Safe if called with valid C string pointers
pub unsafe extern "C" fn kdeconnect_start(
    device_id: *const c_char,
    device_name: *const c_char,
    config_path: *const c_char,
    initialized_callback: extern "C" fn() -> (),
) -> bool {
    check_str!(device_name);
    check_str!(device_id);
    check_str!(config_path);

    if let Ok(rt) = build_runtime!() {
        let ret = rt.block_on(async move {
            if STATE.lock().await.is_some() {
                return Err::<(), Box<dyn Error + Sync + Send>>(Box::new(io::Error::other(
                    "Already started",
                )));
            }

            let config_provider = Arc::new(
                FsConfig::new(
                    config_path.into(),
                    "server_cert".into(),
                    "server_keypair".into(),
                )
                .await?,
            );
            let (kdeconnect, client, mut device_stream) =
                KdeConnect::new(device_id, device_name, DeviceType::Phone, config_provider).await?;

            STATE.lock().await.replace(KConnectState::new(client));

            info!("created kdeconnect client");

            tokio::spawn(async move { kdeconnect.start_server().await });

            std::thread::spawn(move || (initialized_callback)());

            info!("discovering");
            while let Some((mut dev, client)) = device_stream.next().await {
                info!(
                    "new device discovered: id {:?} name {:?} type {:?}",
                    dev.config.id, dev.config.name, dev.config.device_type
                );
                let state = Arc::new(Mutex::new(KConnectDeviceState::default()));
                let handler = Box::new(KConnectIosHandler(state.clone()));
                tokio::spawn(async move { dev.task(handler).await });
                // STATE will always be Some
                STATE
                    .lock()
                    .await
                    .as_mut()
                    .unwrap()
                    .devices
                    .push(KConnectDevice { client, state });
            }

            Ok::<(), Box<dyn Error + Sync + Send>>(())
        });
        info!("runtime ret {:?}", ret);

        ret.is_ok()
    } else {
        false
    }
}

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn kdeconnect_on_battery_event(
    level: c_double,
    charging: c_int,
    within_threshold: c_int,
) -> bool {
    let is_charging = charging == 1;
    let under_threshold = within_threshold == 1;
    info!(
        "recieved battery event: {:?}, {:?}, {:?}",
        level, is_charging, under_threshold
    );

    let battery_state = Battery {
        charge: level as i32,
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
