use std::{collections::HashMap, fmt::Display};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Packet {
    // kdeconnect-kde set this to a string but it's supposed to be an int... :(
    pub id: String,
    #[serde(rename = "type")]
    pub packet_type: String,
    pub body: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct Pair {
    pub pair: bool,
}
derive_type!(Pair, "kdeconnect.pair");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ping {
    pub message: Option<String>,
}
derive_type!(Ping, "kdeconnect.ping");

fn serialize_threshold<S>(x: &bool, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_i32(if *x { 1 } else { 0 })
}

fn deserialize_threshold<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = i32::deserialize(deserializer)?;

    match buf {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Signed(buf.into()),
            &"0 or 1",
        )),
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct Battery {
    #[serde(rename = "currentCharge")]
    pub charge: i32,
    #[serde(rename = "isCharging")]
    pub is_charging: bool,
    #[serde(
        rename = "thresholdEvent",
        serialize_with = "serialize_threshold",
        deserialize_with = "deserialize_threshold"
    )]
    pub under_threshold: bool,
}
derive_type!(Battery, "kdeconnect.battery");

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct BatteryRequest {
    pub request: bool,
}
derive_type!(BatteryRequest, "kdeconnect.battery.request");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Clipboard {
    pub content: String,
}
derive_type!(Clipboard, "kdeconnect.clipboard");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClipboardConnect {
    pub content: String,
    pub timestamp: u128,
}
derive_type!(ClipboardConnect, "kdeconnect.clipboard.connect");

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct FindPhone {}
derive_type!(FindPhone, "kdeconnect.findmyphone.request");

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectivityReport {
    pub signal_strengths: HashMap<String, ConnectivityReportSignal>,
}
derive_type!(ConnectivityReport, "kdeconnect.connectivity_report");

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectivityReportSignal {
    pub network_type: ConnectivityReportNetworkType,
    pub signal_strength: i32,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum ConnectivityReportNetworkType {
    #[serde(rename = "GSM")]
    Gsm,
    #[serde(rename = "CDMA")]
    Cdma,
    #[serde(rename = "iDEN")]
    Iden,
    #[serde(rename = "UMTS")]
    Umts,
    #[serde(rename = "CDMA2000")]
    Cdma2000,
    #[serde(rename = "EDGE")]
    Edge,
    #[serde(rename = "GPRS")]
    Gprs,
    #[serde(rename = "HSPA")]
    Hspa,
    #[serde(rename = "LTE")]
    Lte,
    #[serde(rename = "5G")]
    FiveG,
    #[serde(rename = "Unknown")]
    Unknown,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct ConnectivityReportRequest {}
derive_type!(
    ConnectivityReportRequest,
    "kdeconnect.connectivity_report.request"
);

// to_value should never fail, as Serialize will always be successful and packets should never
// contain non-string keys anyway
#[macro_export]
macro_rules! make_packet {
    ($packet:ident) => {
        Packet {
            id: $crate::util::get_time_ms().to_string(),
            packet_type: $packet.get_type_self().to_string(),
            body: serde_json::value::to_value($packet).expect("packet was invalid"),
        }
    };
}

#[macro_export]
macro_rules! make_packet_str {
    ($packet:ident) => {
        serde_json::to_string(&make_packet!($packet)).map(|x| x + "\n")
    };
}
