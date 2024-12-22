use std::{collections::HashMap, io::ErrorKind, str::FromStr, sync::Arc, time::Duration};

use log::{debug, info};
use serde::Serialize;
use tokio::{
    fs, io::{AsyncWriteExt, AsyncBufReadExt, BufReader}, net::{UnixListener, UnixStream}, sync::Mutex, time::timeout
};
use anyhow::{anyhow, bail, Context};

use crate::settings::VibrationEvent;

pub struct FrankStream {
    stream_lock: Mutex<Option<UnixStream>>,
}

const SOCKET_PATH: &str = "/deviceinfo/dac.sock";

/// Communicates with frankenfirmware by pretending to be the dac process
impl FrankStream {
    pub async fn spawn() -> anyhow::Result<Arc<Self>> {
        Self::remove_socket().await?;
        let listener = UnixListener::bind(SOCKET_PATH).context("Binding to Unix Socket")?;
        let stream = Mutex::new(None);
        let me = Arc::new(FrankStream { stream_lock: stream });
        me.accept_conn(&listener).await; //wait for first connection

        //continously accept newest connection
        let clone = me.clone();
        tokio::spawn(async move {
            loop {
                clone.accept_conn(&listener).await;
            }
        });

        if let Err(e) = me.ping().await {
            bail!("Frank stream connected, but ping failed: {}", e)
        }

        Ok(me)
    }

    async fn accept_conn(&self, listener: &UnixListener) {
        match listener.accept().await {
            Ok((stream, _)) => {
                let mut guard = self.stream_lock.lock().await;
                *guard = Some(stream);
                info!("Frank accepted new connection");
            }
            Err(e) => {
                info!("Frank failed accepting connection: {}", e);
            }
        }
    }

    async fn remove_socket() -> anyhow::Result<()> {
        let a = fs::remove_file(SOCKET_PATH).await;
        match a {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn write_read(&self, bytes: &[u8]) -> anyhow::Result<String> {
        let mut stream_opt = self.stream_lock.lock().await;
        let stream = stream_opt.as_mut().ok_or(anyhow!("Frank stream is None!"))?;
        stream.writable().await?;
        stream.write(bytes).await?;

        stream.readable().await?;
        let mut reader = BufReader::new(stream);
        let read_result = timeout(Duration::from_secs(1), async {
            //read until a double newline
            let mut result = String::new();
            let mut prev_ended = false;
            loop {
                let mut line = String::new();
                let bytes_read = reader.read_line(&mut line).await?;

                if bytes_read == 0 {
                    bail!("Frank got unexpected end of stream");
                }
                result.push_str(&line);

                if line == "\n" && prev_ended {
                    break;
                }
                prev_ended = line.ends_with('\n');
            }
            Ok(result)
        }).await;

        match read_result {
            Ok(result) => result,
            Err(_) => bail!("Timeout occurred while reading from Frank"),
        }
    }

    async fn command(&self, command: u8) -> anyhow::Result<String> {
        self.write_read(format!("{}\n\n", command).as_bytes()).await
    }

    async fn command_with_data(&self, command: u8, data: &str) -> anyhow::Result<String> {
        self.write_read(format!("{}\n{}\n\n", command, data).as_bytes())
            .await
    }

    /// sends "hello" command and returns if it responds "ok"
    pub async fn ping(&self) -> anyhow::Result<()> {
        let res = self.command(0).await?;
        match res.contains("ok") {
            true => Ok(()),
            false => bail!("Bad ping response"),
        }
    }

    pub async fn prime(&self) -> anyhow::Result<String> {
        self.command(13).await
    }

    /// Clear vibration alarm
    pub async fn alarm_clear(&self) -> anyhow::Result<String> {
        self.command(16).await
    }

    /// Set vibration alarm at one timestamp on both sides
    /// Proper usage should create VibrationSettings and call .make_event() every night
    pub async fn set_alarm(&self, settings: &VibrationEvent) -> anyhow::Result<String> {
        let cbor = settings.to_cbor();
        debug!("setting alarm to {}", cbor);
        self.command_with_data(5, &cbor).await?;
        self.command_with_data(6, &cbor).await
    }

    //TODO turn light off

    /// Set the bed temperature for N seconds on both sides
    pub async fn set_temp(&self, temp: i32, duration: u32) -> anyhow::Result<String> {
        self.command_with_data(9, &duration.to_string()).await?;
        self.command_with_data(10, &duration.to_string()).await?;
        self.command_with_data(11, &temp.to_string()).await?;
        self.command_with_data(12, &temp.to_string()).await
    }

    pub async fn get_state(&self) -> anyhow::Result<FrankVariables> {
        FrankVariables::parse(self.command(14).await?)
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct FrankVariables {
    target_heat_level_left: i32,
    target_heat_level_right: i32,
    heat_level_left: i32,
    heat_level_right: i32,
    heat_time_left: u32,
    heat_time_right: u32,
    sensor_label: String,
    water_level: bool,
    priming: bool,
    settings: String,
}

impl FrankVariables {
    fn parse(s: String) -> anyhow::Result<Self> {
        let variables: HashMap<&str, &str> = s
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .collect();

        Ok(FrankVariables {
            target_heat_level_left: Self::parse_var::<i32>(&variables, "tgHeatLevelL")?,
            target_heat_level_right: Self::parse_var::<i32>(&variables, "tgHeatLevelR")?,
            heat_level_left: Self::parse_var::<i32>(&variables, "heatLevelL")?,
            heat_level_right: Self::parse_var::<i32>(&variables, "heatLevelR")?,
            heat_time_left: Self::parse_var::<u32>(&variables, "heatTimeL")?,
            heat_time_right: Self::parse_var::<u32>(&variables, "heatTimeR")?,
            sensor_label: Self::get_var_string(&variables, "sensorLabel")?,
            water_level: Self::parse_var::<bool>(&variables, "waterLevel")?,
            priming: Self::parse_var::<bool>(&variables, "priming")?,
            settings: Self::get_var_string(&variables, "settings")?,
        })
    }

    fn get_var_string(variables: &HashMap<&str, &str>, variable_name: &str) -> anyhow::Result<String> {
        let mut s = variables.get(variable_name).ok_or(anyhow!("Frank Variables missing {}", variable_name))?.to_string();
        s.pop();
        if s.len() > 0 {
            s.remove(0);
        }
        Ok(s)
    }

    fn parse_var<T: FromStr>(variables: &HashMap<&str, &str>, variable_name: &str) -> anyhow::Result<T> {
        let s = variables.get(variable_name).ok_or(anyhow!("Frank Variables missing {}", variable_name))?;
        Ok(s.parse().or(Err(anyhow!("Failed to parse Frank Variable {}", variable_name)))?)
    }

    pub fn serialize(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::FrankVariables;

    #[test]
    fn test_frank_variables() {
        let inp = r#"tgHeatLevelR = 100
tgHeatLevelL = 100
heatTimeL = 0
heatLevelL = -100
heatTimeR = 0
heatLevelR = -100
sensorLabel = "20600-0001-F00-0001089C"
waterLevel = true
priming = false
settings = "BF61760162676C190190626772190190626C621864FF""#;
        let expected = FrankVariables {
            target_heat_level_left: 100,
            target_heat_level_right: 100,
            heat_level_left: -100,
            heat_level_right: -100,
            heat_time_left: 0,
            heat_time_right: 0,
            sensor_label: "20600-0001-F00-0001089C".to_string(),
            water_level: true,
            priming: false,
            settings: "BF61760162676C190190626772190190626C621864FF".to_string(),
        };
        let actual = FrankVariables::parse(inp.to_string()).unwrap();
        println!("{actual:#?}");
        assert_eq!(actual, expected);
    }
}
