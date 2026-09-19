use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::error::*;
use tracing::info;
use crate::backend::app_state::state::Reader;

const BRIGHTNESS_PATH: &str = "/sys/class/backlight/panel/brightness"; //todo make this configurable

pub fn reader() -> Result<Reader<Option<u32>>> {
    let path = PathBuf::from(BRIGHTNESS_PATH);

    if !path.is_file() {
        return Err(Error::new(format!(
            "Panel brightness path does not exist: {}",
            path.display()
        )));
    }

    let reader: Reader<Option<u32>> = Arc::new(move || {
        let path = path.clone();

        Box::pin(async move {
            read(&path).await
        })
    });

    Ok(reader)
}

async fn read(path: &Path) -> Result<Option<u32>> {
    let value_str = tokio::fs::read_to_string(path).await?;
    let value = value_str.trim().parse::<u32>()?;

    Ok(Some(value))
}

pub fn set(value: &Option<u32>) -> Result<()> {
    if let Some(value) = value {
        info!("Setting brightness to {value}");
        std::fs::write(BRIGHTNESS_PATH, value.to_string())?;
    }

    Ok(())
}
