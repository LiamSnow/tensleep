
use cbor::{Encoder, ToCbor};
use chrono::{DateTime, Utc};
use log::{info, warn};
use rustc_serialize::{json::Json, Encodable};
use std::{
    io::{BufReader, BufWriter, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};
use serde::{Deserialize, Serialize};

pub enum VibrationPattern {
    ///heavy
    Double,
    ///gentle
    Rise
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlarmSettings {
    //TODO FIXME
    // pl: Vibration intensity percentage
    pub pl: u8,
    // du: Duration in seconds?
    pub du: u16,
    // pi: Vibration pattern ("double" (heavy) or "rise" (gentle))?
    pub pi: String,
    // tt: Timestamp in unix epoch for alarm
    pub tt: u64,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum BedSide {
    Left, Right, Both
}


#[derive(Debug, Serialize, Deserialize)]
pub struct StreamItem {
    pub part: String,
    pub proto: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchItem {
    pub seq: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CapSenseSide {
    pub status: String,
    pub cen: u16,
    #[serde(rename = "in")]
    pub in_: u16,
    pub out: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CapSense {
    #[serde(with = "ts_seconds")]
    pub ts: DateTime<Utc>,
    pub left: CapSenseSide,
    pub right: CapSenseSide,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PiezoDual {
    #[serde(with = "ts_seconds")]
    pub ts: DateTime<Utc>,
    pub adc: u8,
    pub freq: u16,
    pub gain: u16,
    #[serde(with = "serde_bytes")]
    pub left1: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub left2: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub right1: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub right2: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BedTempSide {
    pub cen: u16,
    #[serde(rename = "in")]
    pub in_: u16,
    pub out: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BedTemp {
    #[serde(with = "ts_seconds")]
    pub ts: DateTime<Utc>,
    pub mcu: u16,
    pub amb: u16,
    pub hu: u16,
    pub left: BedTempSide,
    pub right: BedTempSide,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchItemLog {
    #[serde(with = "ts_seconds")]
    pub ts: DateTime<Utc>,
    pub msg: String,
    pub level: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FrzTemp {
    #[serde(with = "ts_seconds")]
    pub ts: DateTime<Utc>,
    pub amb: u16,
    pub hs: u16,
    pub left: u16,
    pub right: u16,
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BatchItemData {
    #[serde(rename = "capSense")]
    CapSense(CapSense),
    #[serde(rename = "piezo-dual")]
    PiezoDual(PiezoDual),
    #[serde(rename = "bedTemp")]
    BedTemp(BedTemp),
    #[serde(rename = "log")]
    BatchItemLog(BatchItemLog),
    #[serde(rename = "frzTemp")]
    FrzTemp(FrzTemp),
}

