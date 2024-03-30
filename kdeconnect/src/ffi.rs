use std::{
    ffi::{c_char, CStr},
    sync::{Arc, OnceLock},
};

use tokio::runtime::{Builder, Runtime};

use crate::{config::FsConfig, packets::DeviceType, KdeConnect};

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

#[no_mangle]
pub extern "C" fn start_kdeconnect(
    device_id: *const c_char,
    device_name: *const c_char,
    config_path: *const c_char,
) -> bool {
    check_str!(device_name);
    check_str!(device_id);
    check_str!(config_path);
    if let Ok(rt) = build_runtime!() {
        let ret = rt.block_on(async move {
            let kdeconnect = KdeConnect::new(
                device_id,
                device_name,
                DeviceType::Phone,
                Arc::new(FsConfig::new(
                    config_path.into(),
                    "server_cert".into(),
                    "server_keypair".into(),
                )),
            )
            .await?;
            kdeconnect.start_server().await
        });
        println!("ret {:?}", ret);
        ret.is_ok()
    } else {
        false
    }
}
