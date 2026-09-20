use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use tracing::{debug, info};

use crate::error::*;

pub fn config_path() -> String {
    std::env::args()
        .nth(1)
        .unwrap_or_else(|| "clock-iced.toml".to_string())
}

impl Config {
    pub(crate) fn load() -> Result<Self> {
        let path = config_path();

        info!("Loading config from {}", path);

        let config_text = std::fs::read_to_string(path)?;
        let config = toml::from_str(&config_text)?;

        debug!("Loaded config:\n{:#?}", config);

        Ok(config)
    }

    pub(crate) fn save(&self) -> Result<()> {
        let path = config_path();

        info!("Saving config to {}", path);

        let config_toml = toml::to_string_pretty(self)?;
        std::fs::write(path, config_toml)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub window: WindowConfig,
    pub appearance: AppearanceConfig,
    pub http: HttpConfig,
    pub webhook: WebhookConfig,
    pub drift: DriftConfig,
    pub panel: PanelConfig,
    pub proximity: ProximityConfig,
    pub ambient: AmbientConfig,

    #[serde(with = "humantime_serde")]
    pub split_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub fullscreen: bool,
    pub width: f32,
    pub height: f32,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub clock_text_size: u32,
    pub text_size: u32,
    pub line_height: f32,

    #[serde_as(as = "DisplayFromStr")]
    pub color: iced::Color,

    #[serde_as(as = "DisplayFromStr")]
    pub background_color: iced::Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpConfig {
    pub server: String,
    pub fullchain_path: String,
    pub privkey_path: String,
    pub notify: bool,
    pub auth_tokens: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebhookConfig {
    pub url: String,

    #[serde(with = "humantime_serde")]
    pub retry_cooldown: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DriftConfig {
    pub range: i32,

    #[serde(with = "humantime_serde")]
    pub interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelConfig {
    pub device: String,

    #[serde(with = "humantime_serde")]
    pub poll_interval: Duration,

    pub screen_off_cmd: String,
    pub screen_on_cmd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProximityConfig {
    pub device: String,
    pub channel: String,
    pub threshold: i32,

    #[serde(with = "humantime_serde")]
    pub poll_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientConfig {
    pub device: String,
    pub channel: String,

    #[serde(with = "humantime_serde")]
    pub poll_interval: Duration,

    #[serde(with = "humantime_serde")]
    pub update_interval: Duration,

    pub map: Vec<(u32, u32)>,

    #[serde(with = "humantime_serde")]
    pub smoothing: Duration,

    pub proximity_ignore: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            appearance: AppearanceConfig::default(),
            http: HttpConfig::default(),
            webhook: WebhookConfig::default(),
            drift: DriftConfig::default(),
            panel: PanelConfig::default(),
            proximity: ProximityConfig::default(),
            ambient: AmbientConfig::default(),
            split_timeout: Duration::from_secs(60),
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            fullscreen: true,
            width: 1280.0,
            height: 720.0,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            clock_text_size: 350,
            text_size: 75,
            line_height: 1.0,
            color: iced::Color::from_rgba(0.1, 0.0, 0.0, 1.0),
            background_color: iced::Color::BLACK,
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            server: "0.0.0.0:8080".to_string(),
            fullchain_path: String::new(),
            privkey_path: String::new(),
            notify: false,
            auth_tokens: Vec::new(),
        }
    }
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            retry_cooldown: Duration::from_secs(60),
        }
    }
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            range: 2,
            interval: Duration::from_secs(60),
        }
    }
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            device: "panel".to_string(),
            poll_interval: Duration::from_secs(1),
            screen_off_cmd: String::new(),
            screen_on_cmd: String::new(),
        }
    }
}

impl Default for ProximityConfig {
    fn default() -> Self {
        Self {
            device: String::new(),
            channel: String::new(),
            threshold: 0,
            poll_interval: Duration::from_millis(100),
        }
    }
}

impl Default for AmbientConfig {
    fn default() -> Self {
        Self {
            device: String::new(),
            channel: String::new(),
            poll_interval: Duration::from_secs(1),
            update_interval: Duration::from_millis(100),
            map: Vec::new(),
            smoothing: Duration::from_secs(3),
            proximity_ignore: true,
        }
    }
}
