use crate::backend::app_state::global_app_state;
use crate::backend::app_state::state::State;
use crate::backend::hardware;
use crate::error::*;
use tracing::info;

#[derive(Debug, Clone)]
pub struct HardwareState {
    pub ambient: State<Option<u32>>,
    pub proximity: State<Option<hardware::proximity::ProximityState>>,
    pub panel: State<Option<u32>>,
    pub battery: State<Option<u8>>,
}

impl HardwareState {
    pub fn blank() -> Self {
        Self {
            ambient: State::new(None).without_setter(),
            proximity: State::new(None).without_setter(),
            panel: State::new(None).without_setter(),
            battery: State::new(None).without_setter(),
        }
    }

    pub fn init(&mut self) -> Result<()> {
        let (ambient_pool, proximity_pool, panel_pool, battery_pool) = {
            let config = global_app_state().config.blocking_read();
            (
                config.ambient.poll_interval,
                config.proximity.poll_interval,
                config.panel.poll_interval,
                config.battery.poll_interval,
            )
        };

        if let Ok(reader) = hardware::ambient::reader() {
            self.ambient = self.ambient.clone().with_reader(reader);
            self.ambient.spawn_poller(ambient_pool, true)?;
        }

        if let Ok(reader) = hardware::proximity::reader() {
            self.proximity = self.proximity.clone().with_reader(reader);
            self.proximity.spawn_poller(proximity_pool, true)?;
        }

        if let Ok(reader) = hardware::panel::reader() {
            let mut panel = self.panel.clone();
            panel = panel.with_reader(reader);

            if let Ok(setter) = hardware::panel::set() {
                panel = panel.with_setter(setter);
                self.panel = panel;
                self.panel.spawn_poller(panel_pool, false)?;
            }
        }

        if let Ok(reader) = hardware::battery::reader() {
            self.battery = self.battery.clone().with_reader(reader);
            self.battery.spawn_poller(battery_pool, false)?;
        }

        info!("Hardware initialized");
        Ok(())
    }
}
