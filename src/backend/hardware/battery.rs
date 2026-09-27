use crate::backend::app_state::global_app_state;
use crate::backend::app_state::state::Reader;
use crate::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn reader() -> crate::error::Result<Reader<Option<u8>>> {
    let path = global_app_state()
        .config
        .blocking_read()
        .battery
        .channel
        .clone();
    let path = PathBuf::from(path);

    if !path.is_file() {
        return Err(Error::new(format!(
            "Battery path does not exist: {}",
            path.display()
        )));
    }

    let reader: Reader<Option<u8>> = Arc::new(move || {
        let path = path.clone();

        Box::pin(async move { read(&path).await })
    });

    Ok(reader)
}

async fn read(path: &Path) -> crate::error::Result<Option<u8>> {
    let value_str = tokio::fs::read_to_string(path).await?;
    let value = value_str.trim().parse::<u8>()?;

    Ok(Some(value))
}
