use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum VibrationPattern {
    ///heavy
    Double,
    ///gentle
    Rise
}

impl VibrationPattern {
    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default().trim_matches('"').to_string()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlarmSettings {
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
