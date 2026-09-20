use crate::backend::app_state::global_app_state;
use crate::backend::app_state::state::{Reader, Setter};
use crate::error::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

pub fn reader() -> Result<Reader<Option<u32>>> {
    let device = global_app_state()
        .config
        .blocking_read()
        .panel
        .device
        .clone();
    let path = PathBuf::from(format!("/sys/class/backlight/{}/brightness", device));

    if !path.is_file() {
        return Err(Error::new(format!(
            "Panel brightness path does not exist: {}",
            path.display()
        )));
    }

    let reader: Reader<Option<u32>> = Arc::new(move || {
        let path = path.clone();

        Box::pin(async move { read(&path).await })
    });

    Ok(reader)
}

async fn read(path: &Path) -> Result<Option<u32>> {
    let value_str = tokio::fs::read_to_string(path).await?;
    let value = value_str.trim().parse::<u32>()?;

    Ok(Some(value))
}

pub fn set() -> Result<Setter<Option<u32>>> {
    let device = {
        let config = global_app_state().config.blocking_read();
        config.panel.device.clone()
    };

    let path = PathBuf::from(format!("/sys/class/backlight/{}/brightness", device));

    let setter: Setter<Option<u32>> = Arc::new(move |value| {
        let path = path.clone();

        Box::pin(async move {
            if let Some(value) = value {
                info!("Setting brightness to {value}");

                tokio::fs::write(&path, value.to_string()).await?;
            }

            Ok(())
        })
    });

    Ok(setter)
}
