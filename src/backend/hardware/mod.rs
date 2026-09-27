use crate::error::*;
use std::fs;
use std::path::PathBuf;

pub mod ambient;
pub(crate) mod battery;
pub mod panel;
pub mod proximity;

fn find_device(device_name: &str) -> Result<PathBuf> {
    const IIO_PATH: &str = "/sys/bus/iio/devices";

    for entry in fs::read_dir(IIO_PATH)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = match fs::read_to_string(path.join("name")) {
            Ok(name) => name.trim().to_owned(),
            Err(_) => continue,
        };

        if name == device_name {
            return Ok(path);
        }
    }

    Err(Error::new(format!("IIO device '{device_name}' not found")))
}
