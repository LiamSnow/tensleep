use anyhow::{anyhow, bail, Context};
use chrono::NaiveTime;
use chrono_tz::Tz;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Value};
use std::{fs, str::FromStr};

const TIME_FORMAT: &str = "%I:%M %p";

pub type TempProfile = [i32; 3];

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Settings {
    ///offset from "neutral" temperature, °C*10 (IE -40 -> -4°C)
    pub temp_profile: TempProfile,
    pub time_zone: Tz,
    #[serde(
        deserialize_with = "deserialize_time",
        serialize_with = "serialize_time"
    )]
    pub sleep_time: NaiveTime,
    pub alarm: AlarmSettings,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct AlarmSettings {
    #[serde(
        deserialize_with = "deserialize_time",
        serialize_with = "serialize_time"
    )]
    pub time: NaiveTime,
    pub vibration: Option<VibrationSettings>,
    pub heat: Option<HeatSettings>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct VibrationSettings {
    pub pattern: VibrationPattern,
    ///0-100
    pub intensity: u8,
    ///seconds
    pub duration: u16,
    ///seconds before alarm time
    pub offset: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VibrationEvent {
    pub pl: u8,
    pub du: u16,
    pub pi: String,
    pub tt: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VibrationPattern {
    ///heavy
    Double,
    ///gentle
    Rise,
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Eq)]
pub struct HeatSettings {
    pub temp: i32,
    ///seconds before alarm time
    pub offset: u16,
}


impl Settings {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let file_contents = fs::read_to_string(path).context("Reading settings file")?;
        Self::from_str(&file_contents).context("Parsing settings file")
    }

    pub fn from_str(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn serialize(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let json = self.serialize()?;
        Ok(fs::write(path, json)?)
    }
}

fn parse_temp_profile(s: &str) -> anyhow::Result<TempProfile> {
    let re = Regex::new(r"[\[\]{}()]").unwrap();
    let s = re.replace_all(s, "");
    let s_clear = s.replace(" ", "");

    let elements: Vec<&str> = if s.contains(',') {
        s_clear.split(',')
    }
    else if s.contains(';') {
        s_clear.split(';')
    }
    else {
        s.split(' ')
    }.collect();

    if elements.len() != 3 {
        bail!("Wrong amount of elements in temp_profile")
    }

    Ok([elements[0].parse()?, elements[1].parse()?, elements[2].parse()?])
}

impl VibrationSettings {
    pub fn make_event(&self, timestamp: u64) -> VibrationEvent {
        VibrationEvent {
            pl: self.intensity,
            du: self.duration,
            pi: self.pattern.to_string(),
            tt: timestamp,
        }
    }
}

impl VibrationEvent {
    pub fn to_cbor(&self) -> String {
        let mut buffer = Vec::<u8>::new();
        ciborium::into_writer(&self, &mut buffer).unwrap();
        hex::encode(buffer)
    }
}

impl VibrationPattern {
    pub fn to_string(&self) -> String {
        match self {
            VibrationPattern::Double => "double",
            VibrationPattern::Rise => "rise",
        }
        .to_string()
    }
}

impl FromStr for VibrationPattern {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "double" => Ok(Self::Double),
            "rise" => Ok(Self::Rise),
            _ => Err(anyhow!("Invalid vibration pattern")),
        }
    }
}

fn deserialize_time<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NaiveTime, D::Error> {
    let time_str = String::deserialize(deserializer)?;
    NaiveTime::parse_from_str(&time_str, TIME_FORMAT)
        .or_else(|_| NaiveTime::parse_from_str(&time_str, "%H:%M"))
        .map_err(serde::de::Error::custom)
}

fn parse_time(time_str: &str) -> anyhow::Result<NaiveTime> {
    Ok(NaiveTime::parse_from_str(&time_str, TIME_FORMAT)
        .or_else(|_| NaiveTime::parse_from_str(&time_str, "%H:%M"))?)
}

fn serialize_time<S: Serializer>(time: &NaiveTime, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&time.format(TIME_FORMAT).to_string())
}

pub trait ByPath {
    fn get_at_path(&self, path: Vec<&str>) -> anyhow::Result<Option<Value>>;
    fn set_at_path(&mut self, path: Vec<&str>, value: String) -> anyhow::Result<()>;
}

impl ByPath for Settings {
    fn get_at_path(&self, path: Vec<&str>) -> anyhow::Result<Option<Value>> {
        if path.len() < 1 { bail!("Path is too short"); }
        Ok(match path[0] {
            "temp_profile" => Some(json!(self.temp_profile)),
            "time_zone" => Some(json!(self.time_zone.to_string())),
            "sleep_time" => Some(json!(self.sleep_time.format(TIME_FORMAT).to_string())),
            "alarm" => self.alarm.get_at_path(path[1..].to_vec())?,
            _ => bail!("Invalid path for settings"),
        })
    }

    fn set_at_path(&mut self, path: Vec<&str>, value: String) -> anyhow::Result<()> {
        if path.len() < 1 { bail!("Path is too short"); }
        match path[0] {
            "temp_profile" => self.temp_profile = parse_temp_profile(&value)?,
            "time_zone" => self.time_zone = Tz::from_str_insensitive(&value)?,
            "sleep_time" => self.sleep_time = parse_time(&value)?,
            "alarm" => self.alarm.set_at_path(path[1..].to_vec(), value)?,
            _ => bail!("Invalid path for settings"),
        }

        Ok(())
    }
}

impl ByPath for AlarmSettings {
    fn get_at_path(&self, path: Vec<&str>) -> anyhow::Result<Option<Value>> {
        if path.len() < 1 { bail!("Path is too short"); }
        Ok(match path[0] {
            "time" => Some(json!(self.time.format(TIME_FORMAT).to_string())),
            "vibration" => self.vibration.as_ref().and_then(|vib| vib.get_at_path(path[1..].to_vec()).ok().flatten()),
            "heat" => self.heat.as_ref().and_then(|heat| heat.get_at_path(path[1..].to_vec()).ok().flatten()),
            _ => bail!("Invalid path for alarm setting"),
        })
    }

    fn set_at_path(&mut self, path: Vec<&str>, value: String) -> anyhow::Result<()> {
        if path.len() < 1 { bail!("Path is too short"); }
        match path[0] {
            "time" => self.time = parse_time(&value)?,
            "vibration" => match &mut self.vibration {
                Some(vib) => vib.set_at_path(path[1..].to_vec(), value)?,
                None => bail!("Cannot partially modify vibration settings, as it does not exist"),
            },
            "heat" => match &mut self.heat {
                Some(heat) => heat.set_at_path(path[1..].to_vec(), value)?,
                None => bail!("Cannot partially modify heat settings, as it does not exist"),
            },
            _ => bail!("Invalid path for alarm setting"),
        }
        Ok(())
    }
}

impl ByPath for VibrationSettings {
    fn get_at_path(&self, path: Vec<&str>) -> anyhow::Result<Option<Value>> {
        if path.len() < 1 { bail!("Path is too short"); }
        Ok(Some(match path[0] {
            "pattern" => json!(self.pattern),
            "intensity" => json!(self.intensity),
            "duration" => json!(self.duration),
            "offset" => json!(self.offset),
            _ => bail!("Invalid path for vibration setting"),
        }))
    }

    fn set_at_path(&mut self, path: Vec<&str>, value: String) -> anyhow::Result<()> {
        if path.len() < 1 { bail!("Path is too short"); }
        match path[0] {
            "pattern" => self.pattern = value.parse()?,
            "intensity" => self.intensity = value.parse()?,
            "duration" => self.duration = value.parse()?,
            "offset" => self.offset = value.parse()?,
            _ => bail!("Invalid path for vibration setting"),
        }
        Ok(())
    }
}

impl ByPath for HeatSettings {
    fn get_at_path(&self, path: Vec<&str>) -> anyhow::Result<Option<Value>> {
        if path.len() < 1 { bail!("Path is too short"); }
        Ok(match path[0] {
            "temp" => Some(json!(self.temp)),
            "offset" => Some(json!(self.offset)),
            _ => bail!("Invalid path for heat setting"),
        })
    }

    fn set_at_path(&mut self, path: Vec<&str>, value: String) -> anyhow::Result<()> {
        if path.len() < 1 { bail!("Path is too short"); }
        match path[0] {
            "temp" => self.temp = value.parse()?,
            "offset" => self.offset = value.parse()?,
            _ => bail!("Invalid path for heat setting"),
        }
        Ok(())
    }
}

