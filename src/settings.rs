use std::ops::Sub;

use chrono::{DateTime, Utc};

use crate::VibrationPattern;

pub struct Profile {
    ///offset from "neutral" temperature. degrees C * 10 (IE 10C -> 100)
    pub temp_profile: Vec<i32>,
    pub time_zone: chrono::FixedOffset,
    ///minutes from 12AM
    pub sleep_time: Time,
    pub alarm: AlarmSettings,
}

impl Profile {
    pub fn get_temp_at_time(&self, time: DateTime<self.time_zone>) {
        let num_steps = self.temp_profile.len();
    }
}

pub struct AlarmSettings {
    pub time: Time,
    pub vibration: Option<VibrationSettings>,
    pub heat: Option<HeatSettings>,
}

impl AlarmSettings {
    pub fn none() -> Self {
        AlarmSettings {
            time: Time::new(0, 0),
            vibration: None,
            heat: None,
        }
    }

    pub fn vibration_enabled(&self) -> bool {
        self.heat.is_some()
    }

    pub fn heat_enabled(&self) -> bool {
        self.vibration.is_some()
    }

    pub fn get_heat_time(&self) -> Option<Time> {
        match &self.heat {
            Some(v) => Some(self.time - v.offset), //TODO FIXME midnight?
            None => None,
        }
    }

    pub fn should_heat(&self, time: DateTime<self.time_zone>) -> bool {
        if self.heat.is_none() {
            return false;
        }


    }

    pub fn get_vibrate_time(&self) -> Option<Time> {
        match &self.vibration {
            Some(v) => Some(self.time - v.offset), //TODO FIXME midnight?
            None => None,
        }
    }
}

pub struct VibrationSettings {
    pub pattern: VibrationPattern,
    ///0-100
    pub intensity: u8,
    ///seconds
    pub duration: u16,
    ///Time before alarm time
    pub offset: Time,
}

#[derive(Clone)]
pub struct HeatSettings {
    pub temp: i32,
    ///Time before alarm time
    pub offset: Time,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Time {
    ///minutes from 12AM
    minutes: u32,
}

impl Time {
    pub fn new(hours: u32, minutes: u32) -> Self {
        Time {
            minutes: hours * 60 + minutes,
        }
    }
}

impl Sub for Time {
    type Output = Self;
    fn sub(self, other: Self) -> Self::Output {
        Self {
            minutes: self.minutes - other.minutes,
        }
    }
}
