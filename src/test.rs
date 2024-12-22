
#[cfg(test)]
mod tests {
    use chrono::NaiveTime;
    use chrono_tz::Tz;

    use crate::settings::{TenSettings, VibrationPattern};

    #[test]
    fn test_deserialize_settings() {
        let settings = TenSettings::from_str(
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
