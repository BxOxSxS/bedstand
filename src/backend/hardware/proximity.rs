use crate::backend::config::global_config;
use crate::backend::hardware::find_device;
use crate::error::*;
use iced::{
    Subscription,
    futures::{SinkExt, Stream, channel::mpsc},
    stream,
};
use std::path::Path;
use tracing::{debug, info};

#[derive(Debug, Clone, Copy)]
pub enum ProximityEvent {
    Near,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProximityState {
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
    let raw_path = {
        let config = global_config().read().await;
        find_device(config.proximity_device.clone())
            .await
            .add()?
            .join(config.proximity_channel.clone())
    };

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

async fn read_raw(raw_path: &Path) -> Result<i32> {
    let value = tokio::fs::read_to_string(raw_path).await?;
    let value = value.trim();
    value
        .parse::<i32>()
        .map_err(|error| Error::new(format!("Invalid proximity raw value '{value}': {error}")))
}

pub async fn read() -> Result<ProximityState> {
    let (raw_path, threshold) = {
        let config = global_config().read().await;
        (
            find_device(config.proximity_device.clone())
                .await
                .add()?
                .join(config.proximity_channel.clone()),
            config.proximity_threshold,
        )
    };

    let raw = read_raw(&raw_path).await?;

    let state = if raw >= threshold {
        ProximityState::Near
    } else {
        ProximityState::Far
    };

    Ok(state)
}
