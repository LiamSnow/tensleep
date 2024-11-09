// extern crate rustc_serialize;

mod frank;
mod settings;

use chrono_tz::Tz;
use clokwerk::{ScheduleHandle, Scheduler, TimeUnits};
use clokwerk::Interval::*;
use std::thread;
use std::time::Duration;

use frank::manager;
use frank::types::*;
use settings::Settings;

#[rocket::main]
async fn main() {
    env_logger::init();
    // let s = manager::init();

    let settings = Settings::from_file("settings.json").unwrap();
    let handle = schedule(settings).await;
}

async fn schedule(settings: Settings) -> ScheduleHandle {
    let mut scheduler = Scheduler::with_tz(settings.time_zone);

    /*
    Schedules:
    profile[0]
    profile[1]
    profile[2]
    heat_alarm?
    alarm?
    */

    let sleep_time = settings.alarm.time - settings.sleep_time;

    scheduler.every(1.day()).at_time();
        // .at("3:20 pm").run(|| println!("Daily task"));

    scheduler.watch_thread(Duration::from_secs(1))
}

async fn set_temp() {

}
