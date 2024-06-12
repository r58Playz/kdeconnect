use safer_ffi::{ffi_export, prelude::*};
use std::sync::Arc;

use crate::device::KConnectMprisPlayerAction;

pub struct KConnectCallbacks {
    pub initialized: Option<Arc<dyn Fn() + Sync + Send>>,
    pub discovered: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub gone: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,

    pub ping_recieved: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,

    pub pairing_requested: Option<Arc<dyn Fn(char_p::Box, char_p::Box) -> bool + Sync + Send>>,
    pub find_requested: Option<Arc<dyn Fn() + Sync + Send>>,
    pub volume_change_requested: Option<Arc<dyn Fn(i32) + Sync + Send>>,
    pub player_change_requested: Option<Arc<dyn Fn(KConnectMprisPlayerAction, i64) + Sync + Send>>,

    pub pair_status_changed: Option<Arc<dyn Fn(char_p::Box, bool) + Sync + Send>>,
    pub battery_changed: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub clipboard_changed: Option<Arc<dyn Fn(char_p::Box, char_p::Box) + Sync + Send>>,
    pub connectivity_changed: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub volume_changed: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,
    pub player_changed: Option<Arc<dyn Fn(char_p::Box) + Sync + Send>>,

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
            connectivity_changed: None,
            volume_changed: None,
            player_changed: None,

            pairing_requested: None,
            find_requested: None,
            volume_change_requested: None,
            player_change_requested: None,

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
                    $crate::CALLBACKS
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

callback!(
    kdeconnect_register_player_change_callback,
    extern "C" fn(char_p::Box) -> (),
    player_changed,
    x
);

callback!(
    kdeconnect_register_player_action_callback,
    extern "C" fn(KConnectMprisPlayerAction, i64) -> (),
    player_change_requested,
    x,
    y
);
