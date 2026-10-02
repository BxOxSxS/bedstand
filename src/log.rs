use std::{
    fs::File,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use crate::error::*;
use axum::body::Bytes;
use tokio::sync::broadcast;
use tracing_subscriber::fmt::writer::MakeWriter;

pub const LOG_FILE: &str = "bedstand.log";
const BROADCAST_CAPACITY: usize = 2048;

#[derive(Debug, Clone)]
pub struct LogLayer {
    file: Arc<Mutex<File>>,
    broadcast: broadcast::Sender<Bytes>,
}

impl LogLayer {
    pub fn new() -> Result<Self> {
        let file = File::create(LOG_FILE)?;
        let (broadcast, _) = broadcast::channel(BROADCAST_CAPACITY);

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            broadcast,
        })
    }

    pub fn subscribe_snapshot(&self) -> Result<(broadcast::Receiver<Bytes>, u64)> {
        let file = self.file.lock()?;

        let receiver = self.broadcast.subscribe();
        let size = file.metadata()?.len();

        Ok((receiver, size))
    }
}

impl<'a> MakeWriter<'a> for LogLayer {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Write for LogLayer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let bytes = Bytes::copy_from_slice(buf);
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("log file mutex poisoned"))?;

        if let Err(error) = file.write_all(buf) {
            eprintln!("Failed to write log file: {error}");
        }

        if let Err(error) = io::stdout().write_all(buf) {
            eprintln!("Failed to write logs to stdout: {error}");
        }

        let _ = self.broadcast.send(bytes);

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
