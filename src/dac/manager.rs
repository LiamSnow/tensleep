use cbor::{Encoder, ToCbor};
use log::info;
use rustc_serialize::{json::Json, Encodable};
use std::{
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use crate::dac::types::*;

pub type DacStream = Arc<RwLock<Option<UnixStream>>>;

pub fn init(stream: DacStream) {
    thread::spawn(move || {
        let listener = match UnixListener::bind("/deviceinfo/dac.sock") {
            Ok(listener) => listener,
            Err(error) => {
                info!("DAC: failed to listen {:?}", error);
                panic!();
            }
        };
        for newstream in listener.incoming() {
            match newstream {
                Ok(newstream) => {
                    info!("DAC: new UNIX socket connection");
                    let _ = stream.write().unwrap().insert(newstream);
                }
                Err(_) => continue,
            }
        }
    });
}

// Just returns "ok" to show that communication with the firmware is working.
pub fn hello(streamobj: DacStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }
    let mut streamoption = streamobj.write().unwrap();
    let stream = streamoption.as_mut().unwrap();
    let _ = stream.write(b"0\n\n");
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50000000)));
    let mut result = String::new();
    let _ = stream.read_to_string(&mut result);
    result
}

// Gets current state. Example result:
// tgHeatLevelR = 0
// tgHeatLevelL = 0
// heatTimeL = 0
// heatLevelL = -100
// heatTimeR = 0
// heatLevelR = -100
// sensorLabel = null
// waterLevel = true
// priming = false
// settings = "BF61760162676C190190626772190190626C6200FF"
pub fn get_variables(streamobj: DacStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }
    let mut streamoption = streamobj.write().unwrap();
    let stream = streamoption.as_mut().unwrap();
    let _ = stream.write(b"14\n\n");
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50000000)));
    let mut result = String::new();
    let _ = stream.read_to_string(&mut result);
    result
}

// Example CBOR: a462706c18326264751902586274741a65af6af862706966646f75626c65
// pl: Vibration intensity percentage
// pi: Vibration pattern ("double" (heavy) or "rise" (gentle))?
// du: Duration in seconds?
// tt: Timestamp in unix epoch for alarm
// Presumably thermal alarm is controlled with the temperature commands
pub fn set_alarm(side: BedSide, settings: &AlarmSettings, streamobj: DacStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }

    if side == BedSide::Both {
        set_alarm(BedSide::Left, settings, streamobj.clone());
        return set_alarm(BedSide::Right, settings, streamobj);
    }

    let command = match side {
        BedSide::Left => 5,
        BedSide::Right => 6,
        BedSide::Both => panic!(),
    };

    let mut bincbor = Vec::<u8>::new();
    ciborium::into_writer(settings, &mut bincbor).unwrap();
    let serializeddata = hex::encode(bincbor);

    let mut streamoption = streamobj.write().unwrap();
    let stream = streamoption.as_mut().unwrap();
    let _ = stream.write(format!("{}\n{}\n\n", command, serializeddata).as_bytes());
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50000000)));
    let mut result = String::new();
    let _ = stream.read_to_string(&mut result);
    result
}

pub fn alarm_clear(streamobj: DacStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }
    let mut streamoption = streamobj.write().unwrap();
    let stream = streamoption.as_mut().unwrap();
    let _ = stream.write(b"16\n\n");
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50000000)));
    let mut result = String::new();
    let _ = stream.read_to_string(&mut result);
    result
}

// Example CBOR: a1626c6200, a1626c621837. Controls light intensity.
pub fn set_settings(data: &str, streamobj: &DacStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }

    let jsondata = Json::from_str(data).unwrap();
    let cbordata = jsondata.to_cbor();
    let mut cborencoder = Encoder::from_memory();
    cbordata.encode(&mut cborencoder).unwrap();
    let serializeddata = hex::encode(cborencoder.as_bytes());

    let mut streamoption = streamobj.write().unwrap();
    let stream = streamoption.as_mut().unwrap();
    let _ = stream.write(format!("8\n{}\n\n", serializeddata).as_bytes());
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50000000)));
    let mut result = String::new();
    let _ = stream.read_to_string(&mut result);
    result
}

// Takes an integer number of seconds, presumably until the heat ends, e.g. 7200.
pub fn set_temperature_duration(side: BedSide, data: u32, streamobj: DacStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }

    if side == BedSide::Both {
        set_temperature_duration(BedSide::Left, data, streamobj.clone());
        return set_temperature_duration(BedSide::Right, data, streamobj);
    }

    let command = match side {
        BedSide::Left => 9,
        BedSide::Right => 10,
        BedSide::Both => panic!(),
    };

    let mut streamoption = streamobj.write().unwrap();
    let stream = streamoption.as_mut().unwrap();
    let _ = stream.write(format!("{}\n{}\n\n", command, data).as_bytes());
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50000000)));
    let mut result = String::new();
    let _ = stream.read_to_string(&mut result);
    result
}

// Takes a signed integer number. May represent tenths of degrees of heating/cooling. e.g. -40 = -4°C.
pub fn set_temperature(side: BedSide, data: i32, streamobj: DacStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }

    if side == BedSide::Both {
        set_temperature(BedSide::Left, data, streamobj.clone());
        return set_temperature(BedSide::Right, data, streamobj);
    }

    let command = match side {
        BedSide::Left => 11,
        BedSide::Right => 12,
        BedSide::Both => panic!(),
    };

    let mut streamoption = streamobj.write().unwrap();
    let stream = streamoption.as_mut().unwrap();
    let _ = stream.write(format!("{}\n{}\n\n", command, data).as_bytes());
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50000000)));
    let mut result = String::new();
    let _ = stream.read_to_string(&mut result);
    result
}

// Takes a boolean string. Unclear what true/false mean exactly, maybe on/off?
pub fn prime(streamobj: DacStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }

    let mut streamoption = streamobj.write().unwrap();
    let stream = streamoption.as_mut().unwrap();
    let _ = stream.write(b"13\n\n");
    let _ = stream.set_read_timeout(Some(Duration::new(0, 50000000)));
    let mut result = String::new();
    let _ = stream.read_to_string(&mut result);
    result
}
