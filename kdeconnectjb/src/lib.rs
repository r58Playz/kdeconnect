#![feature(once_cell_try)]
use std::{
    error::Error,
    ffi::{c_char, CStr},
    sync::{Arc, OnceLock},
};

use kdeconnect::{
    config::FsConfig,
    device::{Device, DeviceHandler},
    packets::DeviceType,
    KdeConnect,
};
use tokio::runtime::{Builder, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

macro_rules! build_runtime {
    () => {
        RUNTIME.get_or_try_init(|| Builder::new_multi_thread().enable_all().build())
    };
}

macro_rules! check_str {
    ($var:ident) => {
        let $var = unsafe {
            if $var.is_null() {
                return false;
            }
            CStr::from_ptr($var).to_string_lossy().to_string()
        };
    };
}

struct KConnectIosHandler {}

impl DeviceHandler for KConnectIosHandler {
    fn handle_pairing_request(&mut self, device: &Device) -> bool {
        println!("oblivious pair accept: {:?}", device.config);
        true
    }
}

#[no_mangle]
/// # Safety
/// Safe if called with vaild C string pointers
pub unsafe extern "C" fn start_kdeconnect(
    device_id: *const c_char,
    device_name: *const c_char,
    config_path: *const c_char,
) -> bool {
    check_str!(device_name);
    check_str!(device_id);
    check_str!(config_path);

    if let Ok(rt) = build_runtime!() {
        let ret = rt.block_on(async move {
            let config_provider = Arc::new(FsConfig::new(
                config_path.into(),
                "server_cert".into(),
                "server_keypair".into(),
            ));
            let (kdeconnect, mut client) =
                KdeConnect::new(device_id, device_name, DeviceType::Phone, config_provider).await?;

            tokio::spawn(async move { kdeconnect.start_server().await });

            println!("discovering");
            while let Some(mut dev) = client.discover_devices().await {
                println!(
                    "new device discovered: id {:?} name {:?} type {:?}",
                    dev.config.id, dev.config.name, dev.config.device_type
                );
                tokio::spawn(async move { dev.task(Box::new(KConnectIosHandler {})).await });
            }

            Ok::<(), Box<dyn Error + Sync + Send>>(())
        });
        println!("ret {:?}", ret);

        ret.is_ok()
    } else {
        false
    }
}
