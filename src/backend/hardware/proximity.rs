use std::path::Path;
use std::sync::Arc;
use crate::backend::app_state::global_app_state;
use crate::backend::app_state::state::Reader;
use crate::backend::hardware::find_device;
use crate::error::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProximityState {
    Far,
    Near,
}


pub fn reader() -> Result<Reader<Option<ProximityState>>> {
    let (device, channel, threshold) = {
        let config = global_app_state().config.blocking_read();

        (
            config.proximity_device.clone(),
            config.proximity_channel.clone(),
            config.proximity_threshold,
        )
    };

    let device_path = find_device(&device)?;
    let raw_path = device_path.join(channel);

    let reader: Reader<Option<ProximityState>> = Arc::new(move || {
        let raw_path = raw_path.clone();

        Box::pin(async move {
            read(&raw_path, threshold).await
        })
    });

    Ok(reader)
}

async fn read(raw_path: &Path, threshold: i32) -> Result<Option<ProximityState>> {
    let raw = tokio::fs::read_to_string(raw_path).await?;
    let raw = raw.trim();

    let raw = raw
        .parse::<i32>()
        .map_err(|error| Error::new(format!("Invalid proximity raw value '{raw}': {error}")))?;

    Ok(Some(if raw >= threshold {
        ProximityState::Near
    } else {
        ProximityState::Far
    }))
}
