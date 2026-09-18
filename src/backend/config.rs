use crate::error::*;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokio::sync::RwLock;
use tracing::{debug, info};

static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();

pub fn global_config() -> &'static RwLock<Config> {
    CONFIG.get().expect("Config not initialized") // expect is safe here because CONFIG is initialized at startup
}

pub fn config_path() -> String {
    std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.json".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub fullscreen: bool,
    pub window_width: f32,
    pub window_height: f32,

    pub clock_text_size: u32,
    pub text_size: u32,
    pub line_height: f32,

    pub http_server: String,
    pub pem_fullchain_path: String,
    pub pem_privkey_path: String,
    pub pem_notify: bool,
    pub auth_tokens: Vec<String>,

    pub webhook_url: String,                 //runtime
    pub retry_cooldown: std::time::Duration, //runtime

    pub color: iced::Color,
    pub background_color: iced::Color,

    pub drift_range: i32,
    pub drift_interval: std::time::Duration,
    pub split_timeout: std::time::Duration, //runtime

    pub proximity_device: String,
    pub proximity_channel: String,
    pub proximity_threshold: i32,                     //runtime
    pub proximity_poll_interval: std::time::Duration, //runtime

    pub screen_off_cmd: String, //runtime
    pub screen_on_cmd: String,  //runtime,

    pub ambient_device: String,
    pub ambient_channel: String,
    pub ambient_poll_interval: std::time::Duration,
    pub ambient_update_interval: std::time::Duration,
    pub ambient_map: Vec<(u32, u32)>,
    pub ambient_smoothing: std::time::Duration,
    pub ambient_proximity_ignore: bool,
}

impl Config {
    pub fn load() -> Result<&'static RwLock<Config>> {
        info!("Loading config from {}", config_path());

        let config_file = std::fs::File::open(config_path())?;
        let config = serde_json::from_reader(config_file)?;

        debug!("Loaded config:\n{:#?}", config);

        let config = CONFIG.get_or_init(|| RwLock::new(config));
        Ok(config)
    }
    pub fn save(&self) -> Result<()> {
        info!("Saving config to {}", config_path());

        let config_json = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path(), config_json)?;
        Ok(())
    }
}
