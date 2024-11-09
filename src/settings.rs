use chrono::NaiveTime;
use chrono_tz::Tz;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fs, str::FromStr};

use crate::VibrationPattern;

pub type TempProfile = [i32; 3];

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    ///offset from "neutral" temperature. degrees C * 10 (IE 10C -> 100)
    pub temp_profile: TempProfile,
    pub time_zone: Tz,
    #[serde(deserialize_with = "deserialize_time", serialize_with = "serialize_time")]
    pub sleep_time: NaiveTime,
    pub alarm: AlarmSettings,
}

impl Settings {
    pub fn from_file(path: &str) -> Option<Self> {
        let file_contents = fs::read_to_string(path).ok()?;
        Self::from_str(&file_contents).ok()
    }

    pub fn from_str(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn serialize(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AlarmSettings {
    #[serde(deserialize_with = "deserialize_time", serialize_with = "serialize_time")]
    pub time: NaiveTime,
    pub vibration: Option<VibrationSettings>,
    pub heat: Option<HeatSettings>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VibrationSettings {
    pub pattern: VibrationPattern,
    ///0-100
    pub intensity: u8,
    ///seconds
    pub duration: u16,
    ///minutes before alarm time
    pub offset: u16,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct HeatSettings {
    pub temp: i32,
    ///minutes before alarm time
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
        let settings = Settings::from_str(r#"
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
                    "offset": 5
                },
                "heat": {
                    "temp": 100,
                    "offset": 30
                }
            }
        }
        "#).unwrap();

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
        assert_eq!(vibration.offset, 5);

        let heat = settings.alarm.heat.unwrap();
        assert_eq!(heat.temp, 100);
        assert_eq!(heat.offset, 30);
    }
}
