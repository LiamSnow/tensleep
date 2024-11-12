use chrono::{DateTime, Duration, TimeDelta, Utc};
use chrono_tz::Tz;
use log::{debug, info};
use tokio::time;

use crate::{
    dac::manager::{self, DacStream},
    settings::Settings,
    BedSide,
};

pub struct VibrateTiming {
    pub clear: DateTime<Tz>,
    pub set: DateTime<Tz>,
    pub alarm: DateTime<Tz>,
}

pub struct SchedulerTiming {
    pub time_zone: Tz,
    pub vibrate_time: Option<VibrateTiming>,
    pub profile: Vec<(DateTime<Tz>, i32)>,
    ///seconds for last step in profile
    pub profile_end_length: u32
}

const DAY: TimeDelta = chrono::Duration::days(1);

fn calc_times(settings: &Settings) -> SchedulerTiming {
    let now = Utc::now().with_timezone(&settings.time_zone);
    let sleep_datetime = now.with_time(settings.sleep_time).unwrap();
    let mut alarm_datetime = now.with_time(settings.alarm.time).unwrap();
    if alarm_datetime < sleep_datetime {
      alarm_datetime += DAY;
    }

    debug!("Scheduler Timing: now = {now}, alarm_datetime = {alarm_datetime}, sleep_datetime = {sleep_datetime}");

    let vibrate_time = settings.alarm.vibration.clone().map(|v| {
        let alarm = alarm_datetime - TimeDelta::seconds(v.offset.into());
        let set = alarm - Duration::hours(2);
        let clear = alarm - Duration::hours(4);

        debug!(
            "Scheduler Timing: vibrate_time = (clear: {}, set: {}, alarm: {})",
            clear, set, alarm
        );

        VibrateTiming { clear, set, alarm }
    });

    let mut total_time = alarm_datetime - sleep_datetime;
    debug!("Scheduler Timing: total_time = {total_time}");
    if settings.alarm.heat.is_some() {
        let off = TimeDelta::seconds(settings.alarm.heat.clone().unwrap().offset.into());
        total_time -= off;
        debug!(
            "Scheduler Timing: heat alarm is included -> offsetting total_time by {} to {}",
            off, total_time
        );
    }
    let step = total_time / 3;
    debug!("Scheduler Timing: step = {step}");
    let mut profile = vec![
        (sleep_datetime, settings.temp_profile[0]),
        (sleep_datetime + step, settings.temp_profile[1]),
        (sleep_datetime + step + step, settings.temp_profile[2]),
    ];

    if let Some(h) = &settings.alarm.heat {
        profile.push((alarm_datetime - TimeDelta::seconds(h.offset.into()), h.temp));
    }

    let profile_end_length = (alarm_datetime - profile.last().unwrap().0).num_seconds() as u32;
    debug!("Scheduler Timing: profile_end_length = {} - {} = {}", alarm_datetime, profile.last().unwrap().0, profile_end_length);

    SchedulerTiming {
        vibrate_time,
        time_zone: settings.time_zone,
        profile,
        profile_end_length
    }
}

pub async fn run(settings: Settings, stream: DacStream) {
    info!("Scheduler: starting");
    info!("Scheduler: calculating timing...");
    let mut times = calc_times(&settings);
    loop {
        while !manager::hello(stream.clone()).contains("ok") {
            debug!("Scheduler: lost frank connection");
            time::sleep(time::Duration::from_secs(10)).await;
        }

        // let vars = manager::get_variables(stream.clone());
        // debug!("Scheduler: loop / frank vars {vars}");

        let now = Utc::now().with_timezone(&times.time_zone);

        if let Some(vt) = &mut times.vibrate_time {
            if now >= vt.clear {
                info!("Scheduler: clearing vibration alarm");
                manager::alarm_clear(stream.clone());
                vt.clear += DAY;
            }

            if now >= vt.set {
                let ts = vt.alarm.timestamp().try_into().unwrap();
                let vs = settings.alarm.vibration.clone().unwrap();
                let ps = vs.to_dac_alarm_settings(ts);
                info!("Scheduler: setting vibration alarm for {ts}. Mapped {vs:#?} -> {ps:#?}");
                manager::set_alarm(BedSide::Both, &ps, stream.clone());
                vt.alarm += DAY;
                vt.set += DAY;
            }
        }

        let pl = times.profile.len();
        for i in 0..pl {
            if now < times.profile[i].0 {
                continue;
            }

            info!("Scheduler: at profile {i} -> temp to {}", times.profile[i].1);
            manager::set_temperature(BedSide::Both, times.profile[i].1, stream.clone());
            let duration = if i == (pl-1) { times.profile_end_length  } else { 36000 };
            manager::set_temperature_duration(BedSide::Both, duration, stream.clone()); //10 hours
            times.profile[i].0 += DAY;
        }

        time::sleep(time::Duration::from_secs(60)).await;
    }
}
