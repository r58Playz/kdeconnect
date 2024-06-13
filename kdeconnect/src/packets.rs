use std::{collections::HashMap, fmt::Display, str::FromStr};

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

struct DeserializeIDVisitor;

impl<'de> serde::de::Visitor<'de> for DeserializeIDVisitor {
    type Value = u128;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an u128 or a string")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v as u128)
    }

    fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        FromStr::from_str(v).map_err(serde::de::Error::custom)
    }
}

fn deserialize_id<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(DeserializeIDVisitor)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Packet {
    // kdeconnect-kde set this to a string but it's supposed to be an int... :(
    // kdeconnect-android follows the protocol!! so we crash!!
    // so we coerce to a u64
    #[serde(deserialize_with = "deserialize_id")]
    pub id: u128,
    #[serde(rename = "type")]
    pub packet_type: String,
    pub body: Value,
    #[serde(rename = "payloadSize")]
    pub payload_size: Option<i64>,
    #[serde(rename = "payloadTransferInfo")]
    pub payload_transfer_info: Option<PacketPayloadTransferInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PacketPayloadTransferInfo {
    pub port: u16,
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

impl Display for ConnectivityReportNetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ConnectivityReportNetworkType as C;
        match self {
            C::Gsm => write!(f, "GSM"),
            C::Cdma => write!(f, "CDMA"),
            C::Iden => write!(f, "iDEN"),
            C::Umts => write!(f, "UMTS"),
            C::Cdma2000 => write!(f, "CDMA2000"),
            C::Edge => write!(f, "EDGE"),
            C::Gprs => write!(f, "GPRS"),
            C::Hspa => write!(f, "HSPA"),
            C::Lte => write!(f, "LTE"),
            C::FiveG => write!(f, "5G"),
            C::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct ConnectivityReportRequest {}
derive_type!(
    ConnectivityReportRequest,
    "kdeconnect.connectivity_report.request"
);

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
#[serde(untagged)]
pub enum Presenter {
    Move { dx: f32, dy: f32 },
    Stop { stop: bool },
}
derive_type!(Presenter, "kdeconnect.presenter");

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum SystemVolume {
    List {
        #[serde(rename = "sinkList")]
        sink_list: Vec<SystemVolumeStream>,
    },
    Update {
        name: String,
        enabled: Option<bool>,
        muted: Option<bool>,
        volume: Option<i32>,
    },
}
derive_type!(SystemVolume, "kdeconnect.systemvolume");

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SystemVolumeStream {
    pub name: String,
    pub description: String,
    pub enabled: Option<bool>,
    pub muted: bool,
    pub max_volume: Option<i32>,
    pub volume: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SystemVolumeRequest {
    // this may happen again
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_sinks: Option<bool>,
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub muted: Option<bool>,
    pub volume: Option<i32>,
}
derive_type!(SystemVolumeRequest, "kdeconnect.systemvolume.request");

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ShareRequest {
    File(ShareRequestFile),
    Text { text: String },
    Url { url: String },
}
derive_type!(ShareRequest, "kdeconnect.share.request");

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ShareRequestUpdate {
    pub number_of_files: Option<i32>,
    pub total_payload_size: Option<i64>,
}
derive_type!(ShareRequestUpdate, "kdeconnect.share.request.update");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShareRequestFile {
    pub filename: String,
    #[serde(rename = "creationTime")]
    pub creation_time: Option<u128>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<u128>,
    pub open: Option<bool>,
    #[serde(rename = "numberOfFiles")]
    pub number_of_files: Option<i32>,
    #[serde(rename = "totalPayloadSize")]
    pub total_payload_size: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Mpris {
    List {
        #[serde(rename = "playerList")]
        player_list: Vec<String>,
        #[serde(rename = "supportAlbumArtPayload")]
        supports_album_art_payload: bool,
    },
    TransferringArt {
        player: String,
        #[serde(rename = "albumArtUrl")]
        album_art_url: String,
        #[serde(rename = "transferringAlbumArt")]
        transferring_album_art: bool,
    },
    Info(MprisPlayer),
}
derive_type!(Mpris, "kdeconnect.mpris");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MprisPlayer {
    pub player: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    #[serde(rename = "isPlaying")]
    pub is_playing: Option<bool>,
    #[serde(rename = "canPause")]
    pub can_pause: Option<bool>,
    #[serde(rename = "canPlay")]
    pub can_play: Option<bool>,
    #[serde(rename = "canGoNext")]
    pub can_go_next: Option<bool>,
    #[serde(rename = "canGoPrevious")]
    pub can_go_previous: Option<bool>,
    #[serde(rename = "canSeek")]
    pub can_seek: Option<bool>,
    #[serde(rename = "loopStatus")]
    pub loop_status: Option<MprisLoopStatus>,
    pub shuffle: Option<bool>,
    pub pos: Option<i32>,
    pub length: Option<i32>,
    pub volume: Option<i32>,
    #[serde(rename = "albumArtUrl")]
    pub album_art_url: Option<String>,
    // undocumented kdeconnect-kde field
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum MprisLoopStatus {
    None,
    Track,
    Playlist,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum MprisRequest {
    List {
        #[serde(rename = "requestPlayerList")]
        request_player_list: bool,
    },
    PlayerRequest {
        player: String,
        #[serde(rename = "requestNowPlaying")]
        request_now_playing: Option<bool>,
        #[serde(rename = "requestVolume")]
        request_volume: Option<bool>,
        // set to a file:// string to get kdeconnect-kde to send (local) album art
        #[serde(rename = "albumArtUrl", skip_serializing_if = "Option::is_none")]
        request_album_art: Option<String>,
    },
    Action(MprisRequestAction),
}
derive_type!(MprisRequest, "kdeconnect.mpris.request");

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MprisRequestAction {
    pub player: String,
    // ????
    #[serde(rename = "Seek", skip_serializing_if = "Option::is_none")]
    pub seek: Option<i64>,
    #[serde(rename = "setVolume", skip_serializing_if = "Option::is_none")]
    pub set_volume: Option<i64>,
    #[serde(rename = "setLoopStatus", skip_serializing_if = "Option::is_none")]
    pub set_loop_status: Option<MprisLoopStatus>,
    // ??????
    #[serde(rename = "SetPosition", skip_serializing_if = "Option::is_none")]
    pub set_position: Option<i64>,
    #[serde(rename = "setShuffle", skip_serializing_if = "Option::is_none")]
    pub set_shuffle: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<MprisAction>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum MprisAction {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MousepadRequest {
    pub key: Option<String>,
    #[serde(rename = "specialKey")]
    pub special_key: Option<MousepadSpecialKey>,
    pub alt: Option<bool>,
    pub ctrl: Option<bool>,
    pub shift: Option<bool>,

    pub dx: Option<f32>,
    pub dy: Option<f32>,
    pub scroll: Option<bool>,
    pub singleclick: Option<bool>,
    pub doubleclick: Option<bool>,
    pub middleclick: Option<bool>,
    pub rightclick: Option<bool>,
    pub singlehold: Option<bool>,
    pub singlerelease: Option<bool>,

    #[serde(rename = "sendAck")]
    pub send_ack: Option<bool>,
}
derive_type!(MousepadRequest, "kdeconnect.mousepad.request");

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
#[repr(u8)]
pub enum MousepadSpecialKey {
    Backspace = 1,
    Tab = 2,
    DpadLeft = 4,
    DpadUp = 5,
    DpadRight = 6,
    DpadDown = 7,
    PageUp = 8,
    PageDown = 9,
    Home = 10,
    End = 11,
    Enter = 12,
    Delete = 13,
    Escape = 14,
    SysRq = 15,
    ScrollLock = 16,
    F1 = 21,
    F2 = 22,
    F3 = 23,
    F4 = 24,
    F5 = 25,
    F6 = 26,
    F7 = 27,
    F8 = 28,
    F9 = 29,
    F10 = 30,
    F11 = 31,
    F12 = 32,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct MousepadKeyboardState {
    pub state: bool,
}
derive_type!(MousepadKeyboardState, "kdeconnect.mousepad.keyboardstate");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MousepadEcho {
    pub key: Option<String>,
    #[serde(rename = "specialKey")]
    pub special_key: Option<MousepadSpecialKey>,
    pub alt: Option<bool>,
    pub ctrl: Option<bool>,
    pub shift: Option<bool>,

    pub dx: Option<f32>,
    pub dy: Option<f32>,
    pub scroll: Option<bool>,
    pub singleclick: Option<bool>,
    pub doubleclick: Option<bool>,
    pub middleclick: Option<bool>,
    pub rightclick: Option<bool>,
    pub singlehold: Option<bool>,
    pub singlerelease: Option<bool>,

    #[serde(rename = "isAck")]
    pub is_ack: bool,
}
derive_type!(MousepadEcho, "kdeconnect.mousepad.echo");

// to_value should never fail, as Serialize will always be successful and packets should never
// contain non-string keys anyway
#[macro_export]
macro_rules! make_packet {
    ($packet:ident) => {
        Packet {
            id: $crate::util::get_time_ms(),
            packet_type: $packet.get_type_self().to_string(),
            body: serde_json::value::to_value($packet).expect("packet was invalid"),
            payload_size: None,
            payload_transfer_info: None,
        }
    };
}

#[macro_export]
macro_rules! make_packet_payload {
    ($packet:ident, $payload_size:expr, $payload_port:expr) => {
        Packet {
            id: $crate::util::get_time_ms(),
            packet_type: $packet.get_type_self().to_string(),
            body: serde_json::value::to_value($packet).expect("packet was invalid"),
            payload_size: Some($payload_size),
            payload_transfer_info: Some(PacketPayloadTransferInfo {
                port: $payload_port,
            }),
        }
    };
}

#[macro_export]
macro_rules! make_packet_str {
    ($packet:ident) => {
        serde_json::to_string(&make_packet!($packet)).map(|x| x + "\n")
    };
}

#[macro_export]
macro_rules! make_packet_str_payload {
    ($packet:ident, $payload_size:expr, $payload_port:expr) => {
        serde_json::to_string(&make_packet_payload!($packet, $payload_size, $payload_port))
            .map(|x| x + "\n")
    };
}
