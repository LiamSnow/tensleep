use chrono::NaiveTime;
use chrono_tz::Tz;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fs;
use anyhow::Context;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct VibrationEvent {
    pub pl: u8,
    pub du: u16,
    pub pi: String,
    pub tt: u64,
}

impl VibrationEvent {
    pub fn to_cbor(&self) -> String {
        let mut buffer = Vec::<u8>::new();
        ciborium::into_writer(&self, &mut buffer).unwrap();
        hex::encode(buffer)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VibrationPattern {
    ///heavy
    Double,
    ///gentle
    Rise
}

impl VibrationPattern {
    pub fn to_string(&self) -> String {
        match self {
            VibrationPattern::Double => "double",
            VibrationPattern::Rise => "rise",
        }.to_string()
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq, Eq)]
pub struct HeatSettings {
    pub temp: i32,
    ///seconds before alarm time
    pub offset: u16,
}

fn deserialize_time<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NaiveTime, D::Error> {
    let time_str = String::deserialize(deserializer)?;
    NaiveTime::parse_from_str(&time_str, "%I:%M %p")
        .or_else(|_| NaiveTime::parse_from_str(&time_str, "%H:%M"))
        .map_err(serde::de::Error::custom)
}

fn serialize_time<S: Serializer>(time: &NaiveTime, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&time.format("%I:%M %p").to_string())
}

//TODO move
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_settings() {
        let settings = Settings::from_str(
            r#"
        {
            "temp_profile": [-10, 10, 20],
            "time_zone": "America/Los_Angeles",
            "sleep_time": "10:00 PM",
            "alarm": {
                "time": "10:30 AM",
                "vibration": {
                    "pattern": "rise",
                    "intensity": 80,
                    "duration": 600,
                    "offset": 300
                },
                "heat": {
                    "temp": 100,
                    "offset": 1800
                }
            }
        }
        "#,
        )
        .unwrap();

        assert_eq!(settings.temp_profile, [-10, 10, 20]);
        assert_eq!(settings.time_zone, Tz::America__Los_Angeles);
        assert_eq!(
            settings.sleep_time,
            NaiveTime::from_hms_opt(22, 0, 0).unwrap()
        );
        assert_eq!(
            settings.alarm.time,
            NaiveTime::from_hms_opt(10, 30, 0).unwrap()
        );

        let vibration = settings.alarm.vibration.unwrap();
        assert!(matches!(vibration.pattern, VibrationPattern::Rise));
        assert_eq!(vibration.intensity, 80);
        assert_eq!(vibration.duration, 600);
        assert_eq!(vibration.offset, 300);

        let heat = settings.alarm.heat.unwrap();
        assert_eq!(heat.temp, 100);
        assert_eq!(heat.offset, 1800);
    }
}
