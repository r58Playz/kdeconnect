use std::sync::Arc;

use async_trait::async_trait;
use kdeconnect::{
    device::{DeviceClient, DeviceConfig, DeviceHandler},
    packets::{Battery, ConnectivityReport, DeviceType, Ping},
};
use log::info;
use safer_ffi::prelude::*;
use tokio::sync::Mutex;

use crate::{call_callback, call_callback_no_ret, STATE};

pub struct KConnectHandler {
    state: Arc<Mutex<KConnectDeviceState>>,
    config: DeviceConfig,
    id: char_p::Box,
}

impl KConnectHandler {
    pub fn new(state: Arc<Mutex<KConnectDeviceState>>, mut config: DeviceConfig) -> Self {
        // we don't need the cert
        config.certificate.take();
        Self {
            state,
            // this should never fail
            id: config.id.clone().try_into().unwrap(),
            config,
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
        // TODO: Add callback for connectivity report
    }

    async fn handle_pairing_request(&mut self) -> bool {
        info!("recieved pair from {:?}", self.config);
        let id = self.id.clone();
        let res = call_callback!(pairing_requested, id).unwrap_or(false);

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

    async fn handle_exit(&mut self) {
        // STATE will always be Some here
        STATE
            .lock()
            .await
            .as_mut()
            .unwrap()
            .devices
            .retain(|x| x.config.id != self.config.id);
    }
}

#[derive(Default)]
pub struct KConnectDeviceState {
    pub battery: Option<Battery>,
    pub clipboard: Option<String>,
    pub connectivity: Option<ConnectivityReport>,
}

pub struct KConnectDevice {
    pub client: Arc<DeviceClient>,
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
