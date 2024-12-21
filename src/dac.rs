use std::{io::ErrorKind, sync::Arc, time::Duration};

use log::info;
use tokio::{
    fs, io::{AsyncReadExt, AsyncWriteExt}, net::{UnixListener, UnixStream}, sync::Mutex, time::timeout
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

        if !me.ping().await {
            bail!("DAC stream connected, but ping failed")
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

    async fn write_read(&self, command: &[u8]) -> anyhow::Result<String> {
        let mut stream_opt = self.stream_lock.lock().await;
        let stream = stream_opt.as_mut().ok_or(anyhow!("Dac stream is None!"))?;
        stream.writable().await?;
        stream.write(command).await?;

        stream.readable().await?;
        let mut buffer = Vec::new();
        let mut temp_buffer = [0u8; 1024];

        loop {
            //TODO find acutal end of stream
            match timeout(Duration::from_millis(50), stream.read(&mut temp_buffer)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => buffer.extend_from_slice(&temp_buffer[..n]),
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => {
                    info!("254 timeout, partial response: {} bytes", buffer.len());
                    break;
                }
            }
        }

        Ok(String::from_utf8_lossy(&buffer).into_owned())
    }

    //TODO response
    async fn command(&self, command: u8) -> anyhow::Result<String> {
        self.write_read(format!("{}\n\n", command).as_bytes()).await
    }

    async fn command_with_data(&self, command: u8, data: String) -> anyhow::Result<String> {
        self.write_read(format!("{}\n{}\n\n", command, data).as_bytes())
            .await
    }

    /// sends "hello" command and returns if it responds "ok"
    pub async fn ping(&self) -> bool {
        let res = self.command(0).await;
        match res {
            Ok(o) => o.contains("ok"),
            Err(_) => false,
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

    pub async fn set_alarm_both(&self, settings: &VibrationEvent) -> anyhow::Result<String> {
        self.set_alarm(BedSide::Left, settings).await?;
        self.set_alarm(BedSide::Right, settings).await
    }

    pub async fn set_alarm(&self, side: BedSide, settings: &VibrationEvent) -> anyhow::Result<String> {
        let command = if side == BedSide::Left { 5 } else { 6 };
        self.command_with_data(command, settings.to_cbor()).await
    }

    //TODO turn light off

    pub async fn set_temp_both(&self, temp: i32, duration: u32) -> anyhow::Result<String> {
        self.set_temp(BedSide::Left, temp, duration).await?;
        self.set_temp(BedSide::Right, temp, duration).await
    }

    pub async fn set_temp(&self, side: BedSide, temp: i32, duration: u32) -> anyhow::Result<String> {
        let temp_cmd = if side == BedSide::Left { 11 } else { 12 };
        self.command_with_data(temp_cmd, temp.to_string()).await?;

        let dur_cmd = if side == BedSide::Left { 9 } else { 10 };
        self.command_with_data(dur_cmd, duration.to_string()).await
    }
}
