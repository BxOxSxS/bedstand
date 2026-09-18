use crate::error::Error;
use std::fs;
use std::path::PathBuf;

pub mod ambient;
pub mod panel;
pub mod proximity;

async fn find_device(device_name: String) -> crate::error::Result<PathBuf> {
    const IIO_PATH: &str = "/sys/bus/iio/devices";

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
