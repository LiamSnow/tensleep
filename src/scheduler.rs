use std::sync::Arc;

use tokio::sync::RwLock;

use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;
use log::{debug, info};
use tokio::time;

use crate::{frank::FrankStream, settings::{TenSettings, WatchedTenSettings}};

struct VibrateTiming {
    pub clear: DateTime<Tz>,
    pub set: DateTime<Tz>,
    pub alarm: DateTime<Tz>,
}

struct SchedulerTiming {
    pub time_zone: Tz,
    pub vibrate_time: Option<VibrateTiming>,
    pub profile: Vec<(DateTime<Tz>, i32)>,
    ///seconds for last step in profile
    pub profile_end_length: u32,
}

const DAY: Duration = Duration::days(1);

pub fn spawn(dac: Arc<FrankStream>, settings: Arc<RwLock<WatchedTenSettings>>) {
    tokio::spawn(async move {
        loop {
            run(dac.clone(), settings.clone()).await;
        }
    });
}

// TODO FIXME if settings are updated and current_time > alarm_time
// it still sets bed temp
async fn run(dac: Arc<FrankStream>, settings_ref: Arc<RwLock<WatchedTenSettings>>) {
    info!("Scheduler: starting");
    info!("Scheduler: calculating timing...");

    let settings = settings_ref.read().await.clone();

    let mut timing = calc_timing(&settings.settings);

    loop {
        let new_settings = settings_ref.read().await;
        if *new_settings != settings {
            info!("Scheduler: restarting with new settings (change {})!", new_settings.get_change_number());
            break;
        }

        let now = Utc::now().with_timezone(&timing.time_zone);

        if let Some(vt) = &mut timing.vibrate_time {
            if now >= vt.clear {
                info!("Scheduler: clearing vibration alarm");
                let _ = dac.alarm_clear().await;
                vt.clear += DAY;
            }

            if now >= vt.set {
                let ts = vt.alarm.timestamp().try_into().unwrap();
                let vs = settings.settings.alarm.vibration.clone().unwrap();
                let ps = vs.make_event(ts);
                info!("Scheduler: setting vibration alarm for {ts}");
                let _ = dac.set_alarm(&ps).await;
                vt.alarm += DAY;
                vt.set += DAY;
            }
        }

        let pl = timing.profile.len();
        for i in 0..pl {
            if now < timing.profile[i].0 {
                continue;
            }

            info!(
                "Scheduler: at profile {i} -> temp to {}",
                timing.profile[i].1
            );
            let duration = if i == (pl - 1) {
                timing.profile_end_length
            } else {
                36000
            };
            let _ = dac.set_temp(timing.profile[i].1, duration).await;
            timing.profile[i].0 += DAY;
        }

        time::sleep(time::Duration::from_secs(10)).await;
    }
}

fn calc_timing(settings: &TenSettings) -> SchedulerTiming {
    let now = Utc::now().with_timezone(&settings.time_zone);
    let sleep_datetime = now.with_time(settings.sleep_time).unwrap();
    let mut alarm_datetime = now.with_time(settings.alarm.time).unwrap();
    if alarm_datetime < sleep_datetime {
        alarm_datetime += DAY;
    }

    debug!("now = {now}, alarm_datetime = {alarm_datetime}, sleep_datetime = {sleep_datetime}");

    let vibrate_time = settings.alarm.vibration.clone().map(|v| {
        let alarm = alarm_datetime - Duration::seconds(v.offset.into());
        let set = alarm - Duration::hours(2);
        let clear = alarm - Duration::hours(4);

        debug!(
            "vibrate_time = (clear: {}, set: {}, alarm: {})",
            clear, set, alarm
        );

        VibrateTiming { clear, set, alarm }
    });

    let mut total_time = alarm_datetime - sleep_datetime;
    debug!("total_time = {total_time}");
    if settings.alarm.heat.is_some() {
        let off = Duration::seconds(settings.alarm.heat.clone().unwrap().offset.into());
        total_time -= off;
        debug!(
            "heat alarm is included -> offsetting total_time by {} to {}",
            off, total_time
        );
    }
    let step = total_time / 3;
    debug!("step = {step}");
    let mut profile = vec![
        (sleep_datetime, settings.temp_profile[0]),
        (sleep_datetime + step, settings.temp_profile[1]),
        (sleep_datetime + step + step, settings.temp_profile[2]),
    ];

    if let Some(h) = &settings.alarm.heat {
        profile.push((alarm_datetime - Duration::seconds(h.offset.into()), h.temp));
    }

    let profile_end_length = (alarm_datetime - profile.last().unwrap().0).num_seconds() as u32;
    debug!(
        "profile_end_length = {} - {} = {}",
        alarm_datetime,
        profile.last().unwrap().0,
        profile_end_length
    );

    SchedulerTiming {
        vibrate_time,
        time_zone: settings.time_zone,
        profile,
        profile_end_length,
    }
}
