use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::backend::config::global_config;
use crate::error::*;
use iced::{
    Subscription,
    futures::{SinkExt, Stream, channel::mpsc},
    stream,
};
use tracing::{debug, info};

#[derive(Debug, Clone, Copy)]
pub enum ProximityEvent {
    Near,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProximityState {
    Far,
    Near,
}

pub fn subscription() -> Subscription<ProximityEvent> {
    Subscription::run(event_stream)
}

fn event_stream() -> impl Stream<Item = ProximityEvent> {
    stream::channel(1, async |mut output| {
        let _ = run(&mut output).await.add();
    })
}

async fn run(output: &mut mpsc::Sender<ProximityEvent>) -> Result<()> {
    let sysfs_path = find_device().await.add()?;

    let raw_path = sysfs_path.join("in_proximity_raw");

    info!("Found proximity sensor: {}", raw_path.display());

    let initial_raw = read_raw(&raw_path).await.add()?;

    let mut state = if initial_raw >= global_config().read().await.proximity_threshold {
        ProximityState::Near
    } else {
        ProximityState::Far
    };

    debug!("Initial proximity state: {:?}, raw={initial_raw}", state);

    loop {
        tokio::time::sleep(global_config().read().await.proximity_poll_interval).await;

        let raw = read_raw(&raw_path).await.add()?;

        let new_state = if raw >= global_config().read().await.proximity_threshold {
            ProximityState::Near
        } else {
            ProximityState::Far
        };

        if new_state == state {
            continue;
        }

        info!("Proximity: {:?} -> {:?}, raw={raw}", state, new_state);

        if let (ProximityState::Far, ProximityState::Near) = (state, new_state) {
            output
                .send(ProximityEvent::Near)
                .await
                .map_err(|_| Error::new("Proximity subscription receiver dropped"))?;
        }

        state = new_state;
    }
}

async fn find_device() -> Result<PathBuf> {
    const IIO_PATH: &str = "/sys/bus/iio/devices";
    let device_name = global_config().read().await.proximity_device.clone();

    for entry in fs::read_dir(IIO_PATH)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name_path = path.join("name");

        let name = match fs::read_to_string(&name_path) {
            Ok(name) => name.trim().to_owned(),
            Err(_) => continue,
        };

        if name != device_name {
            continue;
        }

        return Ok(path);
    }

    Err(Error::new(format!("IIO device '{device_name}' not found")))
}

async fn read_raw(raw_path: &Path) -> Result<i32> {
    let value = tokio::fs::read_to_string(raw_path).await?;
    let value = value.trim();
    value
        .parse::<i32>()
        .map_err(|error| Error::new(format!("Invalid proximity raw value '{value}': {error}")))
}
