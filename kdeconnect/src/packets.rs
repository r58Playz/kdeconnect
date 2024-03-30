use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: usize = 7;

macro_rules! derive_type {
    ($struct:ty, $type:literal) => {
        impl PacketType for $struct {
            fn get_type_self(&self) -> &'static str {
                $type
            }
        }
        impl $struct {
            pub const TYPE: &'static str = $type;
        }
    };
}

pub(crate) trait PacketType {
    fn get_type_self(&self) -> &'static str;
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Desktop,
    Laptop,
    Phone,
    Tablet,
    Tv,
}

impl Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DeviceType as D;
        match self {
            D::Desktop => write!(f, "desktop"),
            D::Laptop => write!(f, "laptop"),
            D::Phone => write!(f, "phone"),
            D::Tablet => write!(f, "tablet"),
            D::Tv => write!(f, "tv"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    // kdeconnect-kde set this to a string but it's supposed to be an int... :(
    pub id: String,
    #[serde(rename = "type")]
    pub packet_type: String,
    pub body: Value,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub device_id: String,
    pub device_name: String,
    pub device_type: DeviceType,
    pub incoming_capabilities: Vec<String>,
    pub outgoing_capabilities: Vec<String>,
    pub protocol_version: usize,
    pub tcp_port: Option<u16>,
}
derive_type!(Identity, "kdeconnect.identity");

#[derive(Serialize, Deserialize, Debug)]
pub struct Pair {
    pub pair: bool,
}
derive_type!(Pair, "kdeconnect.pair");

#[derive(Serialize, Deserialize, Debug)]
pub struct Ping {
    pub message: Option<String>,
}
derive_type!(Ping, "kdeconnect.ping");

// to_value should never fail, as Serialize will always be successful and packets should never
// contain non-string keys anyway
#[macro_export]
macro_rules! make_packet {
    ($packet:ident) => {
        Packet {
            id: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("time went backwards")
                .as_millis().to_string(),
            packet_type: $packet.get_type_self().to_string(),
            body: serde_json::value::to_value($packet).expect("packet was invalid"),
        }
    };
}
