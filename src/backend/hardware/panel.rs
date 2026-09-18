use crate::error::*;
use tracing::info;

const BRIGHTNESS_PATH: &str = "/sys/class/backlight/panel/brightness";

pub fn get_brightness() -> Result<u32> {
    let value_str = std::fs::read_to_string(BRIGHTNESS_PATH)?;
    let value = value_str.trim().parse::<u32>()?;
    Ok(value)
}

pub fn set_brightness(value: u32) -> Result<()> {
    info!("Setting brightness to {value}");
    std::fs::write(BRIGHTNESS_PATH, value.to_string())?;

    Ok(())
}

pub fn set_brightness_if_changed(value: u32) -> Result<()> {
    let current_value = get_brightness()?;
    if current_value != value {
        set_brightness(value)?;
    }
    Ok(())
}
