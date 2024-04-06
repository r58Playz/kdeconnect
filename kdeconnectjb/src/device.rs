use std::{io, sync::Arc, time::Duration};

use async_trait::async_trait;
use kdeconnect::{
    device::{DeviceClient, DeviceConfig, DeviceHandler},
    packets::{Battery, DeviceType, Ping},
};
use log::{info, warn};
use safer_ffi::prelude::*;
use tokio::{
    io::{stdin, AsyncReadExt},
    sync::Mutex,
    time::timeout,
};

use crate::STATE;

pub struct KConnectHandler<F>
where
    F: FnMut(char_p::Box) + Send,
{
    state: Arc<Mutex<KConnectDeviceState>>,
    config: DeviceConfig,
    id: char_p::Box,
    changed_callback: F,
}

impl<F: FnMut(char_p::Box) + Send> KConnectHandler<F> {
    pub fn new(state: Arc<Mutex<KConnectDeviceState>>, config: DeviceConfig, callback: F) -> Self {
        Self {
            state,
            // this should never fail
            id: config.id.clone().try_into().unwrap(),
            config,
            changed_callback: callback,
        }
    }
}

#[async_trait]
impl<F: FnMut(char_p::Box) + Send> DeviceHandler for KConnectHandler<F> {
    async fn handle_ping(&mut self, packet: Ping) {
        warn!(
            "recieved ping: {:?} packet: {:#?}",
            self.config.name, packet
        );
    }

    async fn handle_battery(&mut self, packet: Battery) {
        let mut state = self.state.lock().await;
        state.battery_level.replace(packet.charge);
        state.battery_charging.replace(packet.is_charging);
        state
            .battery_under_threshold
            .replace(packet.under_threshold);
        info!(
            "recieved battery data: {:?} packet: {:#?}",
            self.config.name, packet
        );
        (self.changed_callback)(self.id.clone())
    }

    async fn handle_clipboard_content(&mut self, content: String) {
        info!(
            "recieved clipboard content: {:?} data: {:#?}",
            self.config.name, content
        );
        self.state.lock().await.clipboard.replace(content);
        (self.changed_callback)(self.id.clone())
    }

    async fn handle_pairing_request(&mut self) -> bool {
        info!("recieved pair from {:?}", self.config);
        let res = timeout(Duration::from_secs(5), stdin().read(&mut [0; 128]))
            .await
            .map_err(io::Error::other)
            .and_then(|x| x)
            .is_ok_and(|x| x > 0);
        warn!(
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
        STATE
            .lock()
            .await
            .as_ref()
            .unwrap()
            .current_clipboard
            .clone()
    }
}

#[derive(Default)]
pub struct KConnectDeviceState {
    pub battery_level: Option<i32>,
    pub battery_charging: Option<bool>,
    pub battery_under_threshold: Option<bool>,
    pub clipboard: Option<String>,
}

pub struct KConnectDevice {
    pub client: DeviceClient,
    pub config: DeviceConfig,
    pub state: Arc<Mutex<KConnectDeviceState>>,
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
}
