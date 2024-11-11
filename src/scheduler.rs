use chrono::{DateTime, Duration, TimeDelta, Utc};
use chrono_tz::Tz;
use tokio::time;

use crate::{
    frank::manager::{self, FrankStream},
    settings::Settings,
    BedSide,
};

pub struct VibrateTiming {
    pub clear: DateTime<Tz>,
    pub set: DateTime<Tz>,
    pub alarm: DateTime<Tz>,
}

pub struct SchedulerConfig {
    pub time_zone: Tz,
    pub vibrate_time: Option<VibrateTiming>,
    pub profile: Vec<(DateTime<Tz>, i32)>,
}

const DAY: TimeDelta = chrono::Duration::days(1);

fn calc_times(settings: &Settings) -> SchedulerConfig {
    let now = Utc::now().with_timezone(&settings.time_zone);
    let tomorrow = now + Duration::days(1);
    let alarm_datetime = tomorrow.with_time(settings.alarm.time).unwrap();
    let sleep_datetime = now.with_time(settings.sleep_time).unwrap();

    let vibrate_time = settings.alarm.vibration.clone().map(|v| {
        let alarm = alarm_datetime - TimeDelta::seconds(v.offset.into());

        VibrateTiming {
            clear: alarm - Duration::hours(4),
            set: alarm - Duration::hours(2),
            alarm,
        }
    });

    let mut total_time = alarm_datetime - sleep_datetime;
    if settings.alarm.heat.is_some() {
        total_time -= TimeDelta::seconds(settings.alarm.heat.clone().unwrap().offset.into());
    }
    let step = total_time / 3;
    let mut profile = vec![
        (sleep_datetime, settings.temp_profile[0]),
        (sleep_datetime + step, settings.temp_profile[1]),
        (sleep_datetime + step + step, settings.temp_profile[2]),
    ];

    if let Some(h) = &settings.alarm.heat {
        profile.push((alarm_datetime - TimeDelta::seconds(h.offset.into()), h.temp));
    }

    SchedulerConfig {
        vibrate_time,
        time_zone: settings.time_zone,
        profile,
    }
}

pub async fn run(settings: Settings, stream: &FrankStream) {
    let mut times = calc_times(&settings);
    loop {
        let now = Utc::now().with_timezone(&times.time_zone);

        if let Some(vt) = &mut times.vibrate_time {
            if now >= vt.clear {
                manager::alarm_clear(stream);
                vt.clear += DAY;
            }

            if now >= vt.set {
                let ts = vt.alarm.timestamp().try_into().unwrap();
                let vs = settings.alarm.vibration.clone().unwrap();
                manager::set_alarm(
                    BedSide::Both,
                    &vs.to_frank_alarm_settings(ts),
                    stream,
                );
                vt.alarm += DAY;
                vt.set += DAY;
            }
        }

        for i in 0..times.profile.len() {
            manager::set_temperature(BedSide::Both, times.profile[i].1, stream);
            manager::set_temperature_duration(BedSide::Both, 36000, stream); //10 hours
            times.profile[i].0 += DAY;
        }

        time::sleep(time::Duration::from_secs(60)).await;
    }
}

