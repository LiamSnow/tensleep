use cbor::{Encoder, ToCbor};
use log::{info, trace, warn};
use rustc_serialize::{json::Json, Encodable};
use std::{
    io::{BufReader, BufWriter, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use crate::frank::types::*;

pub type FrankStream = Arc<RwLock<Option<UnixStream>>>;

pub fn init() -> FrankStream {
    let stream = Arc::new(RwLock::<Option<UnixStream>>::new(None));

    let streamcopy = stream.clone();
    thread::spawn(move || {
        let listener = match UnixListener::bind("/deviceinfo/dac.sock") {
            Ok(listener) => listener,
            Err(error) => {
                panic!("Frank: failed to listen {:?}", error)
            }
        };
        for newstream in listener.incoming() {
            match newstream {
                Ok(newstream) => {
                    info!("Frank: new UNIX socket connection");
                    let _ = streamcopy.write().unwrap().insert(newstream);
                }
                Err(_) => continue,
            }
        }
    });

    thread::spawn(|| {
        let listener = TcpListener::bind("0.0.0.0:1337").unwrap();
        for stream in listener.incoming() {
            let stream = match stream {
                Err(_) => continue,
                Ok(stream) => stream,
            };
            thread::spawn(move || {
                handle_data_stream(stream);
            });
        }
    });

    stream
}

// Just returns "ok" to show that communication with the firmware is working.
pub fn hello(streamobj: &FrankStream) -> String {
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
pub fn get_variables(streamobj: &FrankStream) -> String {
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
pub fn set_alarm(side: BedSide, settings: &AlarmSettings, streamobj: &FrankStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }

    if side == BedSide::Both {
        set_alarm(BedSide::Left, settings, streamobj);
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

pub fn alarm_clear(streamobj: &FrankStream) -> String {
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
pub fn set_settings(data: &str, streamobj: &FrankStream) -> String {
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
pub fn set_temperature_duration(side: BedSide, data: u32, streamobj: &FrankStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }

    if side == BedSide::Both {
        set_temperature_duration(BedSide::Left, data, streamobj);
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
pub fn set_temperature(side: BedSide, data: i32, streamobj: &FrankStream) -> String {
    if streamobj.read().unwrap().is_none() {
        return "not connected".to_string();
    }

    if side == BedSide::Both {
        set_temperature(BedSide::Left, data, streamobj);
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
pub fn prime(streamobj: FrankStream) -> String {
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

fn handle_session(item: StreamItem, writer: &mut dyn Write) {
    info!(
        "Frank: session started for device {}",
        item.dev.expect("expected device ID")
    );
    let _ = ciborium::into_writer::<StreamItem, &mut dyn Write>(
        &StreamItem {
            part: "session".into(),
            proto: "raw".into(),
            id: None,
            version: None,
            dev: None,
            stream: None,
        },
        writer,
    );
    let _ = writer.flush();
}

fn handle_batch(item: StreamItem, writer: &mut dyn Write) {
    let id = match item.id {
        Some(id) => id,
        None => {
            warn!("Frank: no id was present for batch");
            return;
        }
    };
    info!("Frank: received batch {}", id);
    let _ = ciborium::into_writer::<StreamItem, &mut dyn Write>(
        &StreamItem {
            id: Some(id),
            proto: "raw".into(),
            part: "batch".into(),
            dev: None,
            stream: None,
            version: None,
        },
        writer,
    );
    let _ = writer.flush();

    let datastream = match item.stream {
        Some(stream) => stream,
        None => {
            warn!("Frank: no stream in batch");
            return;
        }
    };
    let mut reader = BufReader::new(datastream.as_slice());
    loop {
        let item: BatchItem = match ciborium::from_reader(&mut reader) {
            Ok(item) => item,
            Err(ciborium::de::Error::Io(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => {
                warn!("Frank: failed to read batch item: {:?}", error);
                break;
            }
        };
        let seq = item.seq;
        let item: BatchItemData = match ciborium::from_reader(item.data.as_slice()) {
            Ok(item) => item,
            Err(_) => {
                match ciborium::from_reader::<ciborium::Value, &[u8]>(item.data.as_slice()) {
                    Ok(item) => {
                        warn!("Frank: failed to read batch item data, generic value: {:?}", item);
                        continue;
                    }
                    Err(error) => {
                        warn!(
                            "Frank: failed to read batch item data. Data was {:?}. Error was {:?}",
                            hex::encode(item.data),
                            error
                        );
                        continue;
                    }
                };
            }
        };
        trace!("Frank: batch item {} datum: {:?}", seq, item);
    }
}

fn handle_data_stream(stream: TcpStream) {
    info!("Frank: incoming TCP connection");
    let _ = stream.set_read_timeout(Some(Duration::new(60, 0)));

    let mut writer = BufWriter::new(&stream);
    let mut reader = BufReader::new(&stream);

    loop {
        let item: StreamItem = ciborium::from_reader(&mut reader).unwrap();
        match item.part.as_str() {
            "session" => handle_session(item, &mut writer),
            "batch" => handle_batch(item, &mut writer),
            _ => {
                warn!("Frank: unrecognized part {:?}", item.part);
                continue;
            }
        }
    }
}
