pub mod config;
pub mod data;
pub mod hardware;
pub mod runtime;
pub mod state;

use crate::backend::app_state::{hardware::HardwareState, runtime::RuntimeState};
use crate::error::*;
use crate::log::LogLayer;
use std::sync::OnceLock;
use tokio::sync::RwLock;

pub static APP_STATE: OnceLock<AppState> = OnceLock::new();

pub fn global_app_state() -> &'static AppState {
    APP_STATE.get().unwrap()
}

#[derive(Debug)]
pub struct AppState {
    pub config: RwLock<config::Config>,
    pub data: data::Data,
    pub hardware: RwLock<HardwareState>,
    pub runtime: RuntimeState,
    pub logs: LogLayer,
}

impl AppState {
    pub fn new(logs: LogLayer) -> Result<AppState> {
        let config = config::Config::load().add().unwrap_or_default();
        let data = data::Data::default();
        let hardware = HardwareState::blank();

        Ok(AppState {
            config: RwLock::new(config),
            data,
            hardware: RwLock::new(hardware),
            runtime: RuntimeState::new(),
            logs,
        })
    }
}
