use std::{io::ErrorKind, sync::Arc, time::Duration};

use log::{debug, info};
use tokio::{
    fs, io::{AsyncWriteExt, AsyncBufReadExt, BufReader}, net::{UnixListener, UnixStream}, sync::Mutex, time::timeout
};
use anyhow::{anyhow, bail, Context};

use crate::settings::VibrationEvent;

pub struct DacStream {
    stream_lock: Mutex<Option<UnixStream>>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum BedSide {
    Left,
    Right,
}

const SOCKET_PATH: &str = "/deviceinfo/dac.sock";

impl DacStream {
    pub async fn spawn() -> anyhow::Result<Arc<Self>> {
        Self::remove_socket().await?;

        let listener = UnixListener::bind(SOCKET_PATH).context("Binding to Unix Socket")?;
        let stream = Mutex::new(None);
        let me = Arc::new(DacStream { stream_lock: stream });
        me.accept_stream(&listener).await; //wait for first connection

        let clone = me.clone();
        tokio::spawn(async move {
            loop {
                clone.accept_stream(&listener).await;
            }
        });

        if let Err(e) = me.ping().await {
            bail!("DAC stream connected, but ping failed: {}", e)
        }

        Ok(me)
    }

    async fn accept_stream(&self, listener: &UnixListener) {
        match listener.accept().await {
            Ok((stream, _)) => {
                let mut guard = self.stream_lock.lock().await;
                *guard = Some(stream);
                info!("DAC accepted new connection");
            }
            Err(e) => {
                info!("DAC failed accepting connection: {}", e);
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
        let stream = stream_opt.as_mut().ok_or(anyhow!("DAC stream is None!"))?;
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
                    bail!("DAC got unexpected end of stream");
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
            Err(_) => bail!("Timeout occurred while reading from DAC"),
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

    pub async fn get_variables(&self) -> anyhow::Result<String> {
        self.command(14).await
    }

    pub async fn prime(&self) -> anyhow::Result<String> {
        self.command(13).await
    }

    pub async fn alarm_clear(&self) -> anyhow::Result<String> {
        self.command(16).await
    }

    pub async fn set_alarm(&self, settings: &VibrationEvent) -> anyhow::Result<String> {
        let cbor = settings.to_cbor();
        debug!("setting alarm to {}", cbor);
        self.command_with_data(5, &cbor).await?;
        self.command_with_data(6, &cbor).await
    }

    pub async fn set_alarm_for_side(&self, side: BedSide, settings: &VibrationEvent) -> anyhow::Result<String> {
        let command = if side == BedSide::Left { 5 } else { 6 };
        self.command_with_data(command, &settings.to_cbor()).await
    }

    //TODO turn light off

    pub async fn set_temp(&self, temp: i32, duration: u32) -> anyhow::Result<String> {
        self.command_with_data(9, &duration.to_string()).await?;
        self.command_with_data(10, &duration.to_string()).await?;
        self.command_with_data(11, &temp.to_string()).await?;
        self.command_with_data(12, &temp.to_string()).await
    }

    pub async fn set_temp_for_side(&self, side: BedSide, temp: i32, duration: u32) -> anyhow::Result<String> {
        let dur_cmd = if side == BedSide::Left { 9 } else { 10 };
        self.command_with_data(dur_cmd, &duration.to_string()).await?;
        let temp_cmd = if side == BedSide::Left { 11 } else { 12 };
        self.command_with_data(temp_cmd, &temp.to_string()).await
    }
}
