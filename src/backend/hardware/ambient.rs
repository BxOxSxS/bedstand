use std::path::PathBuf;
use std::time::Duration;

use crate::backend::config::global_config;
use crate::backend::hardware::panel::set_brightness_if_changed;
use crate::backend::hardware::{find_device, proximity};
use crate::error::*;

const MIN_MAP_POINTS: usize = 2;

pub fn spawn_ambient_controller() {
    tokio::spawn(async {
        let (
            ambient_device,
            ambient_channel,
            poll_interval,
            ambient_map,
            smoothing,
            proximity_ignore,
            update_interval,
        ) = {
            let config = global_config().read().await;

            (
                config.ambient_device.clone(),
                config.ambient_channel.clone(),
                config.ambient_poll_interval,
                config.ambient_map.clone(),
                config.ambient_smoothing,
                config.ambient_proximity_ignore,
                config.ambient_update_interval,
            )
        };

        let device_path = match find_device(ambient_device).await.add() {
            Ok(path) => path,
            Err(_) => {
                return;
            }
        };

        let channel_path = device_path.join(ambient_channel);

        if !channel_path.is_file() {
            let _ = Error::new(format!(
                "Ambient channel path does not exist: {}",
                channel_path.display()
            ));
            return;
        }

        if validate_map(&ambient_map).add().is_err() {
            return;
        }

        run_ambient_controller(
            channel_path,
            ambient_map,
            poll_interval,
            update_interval,
            smoothing,
            proximity_ignore,
        )
        .await;
    });
}

async fn run_ambient_controller(
    channel_path: PathBuf,
    ambient_map: Vec<(u32, u32)>,
    poll_interval: Duration,
    update_interval: Duration,
    smoothing: Duration,
    proximity_ignore: bool,
) {
    let mut sensor_interval = tokio::time::interval(poll_interval);
    let mut update_interval = tokio::time::interval(update_interval);

    let mut ema = Ema::new(smoothing);
    let mut last_update = tokio::time::Instant::now();

    let mut target: Option<f64> = None;

    loop {
        tokio::select! {
            biased;

            _ = sensor_interval.tick() => {
                let ambient = match read_ambient(&channel_path).add() {
                    Ok(value) => value,
                    Err(_) => continue,
                };

                target = Some(map_value(&ambient_map, ambient));
            }
            _ = update_interval.tick() => {
                let Some(target) = target else {
                    continue;
                };

                let now = tokio::time::Instant::now();
                let dt = now.duration_since(last_update);
                last_update = now;

                let brightness = ema.update(target, dt);
                let brightness = brightness.round() as u32;

                if proximity_ignore && proximity::read().await == Ok(proximity::ProximityState::Near)
                {
                    continue;
                }

                let _ = set_brightness_if_changed(brightness).add();
            }
        }
    }
}

fn read_ambient(channel_path: &PathBuf) -> Result<u32> {
    let value = std::fs::read_to_string(channel_path)?
        .trim()
        .parse::<u32>()?;

    Ok(value)
}

fn validate_map(points: &[(u32, u32)]) -> Result<()> {
    if points.len() < MIN_MAP_POINTS {
        return Err(Error::new(format!(
            "ambient_map must contain at least {MIN_MAP_POINTS} points"
        )));
    }

    for pair in points.windows(2) {
        let (x0, _) = pair[0];
        let (x1, _) = pair[1];

        if x0 >= x1 {
            return Err(Error::new(format!(
                "ambient_map X values must be strictly increasing: {x0} >= {x1}"
            )));
        }
    }

    Ok(())
}

fn map_value(points: &[(u32, u32)], x: u32) -> f64 {
    if x <= points[0].0 {
        return points[0].1 as f64;
    }

    let last = points.len() - 1;

    if x >= points[last].0 {
        return points[last].1 as f64;
    }

    for pair in points.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];

        if x <= x1 {
            let x0 = x0 as f64;
            let x1 = x1 as f64;
            let y0 = y0 as f64;
            let y1 = y1 as f64;
            let x = x as f64;

            let t = (x - x0) / (x1 - x0);

            return y0 + t * (y1 - y0);
        }
    }

    unreachable!()
}

struct Ema {
    value: Option<f64>,
    tau: Duration,
}

impl Ema {
    fn new(tau: Duration) -> Self {
        Self { value: None, tau }
    }

    fn update(&mut self, input: f64, dt: Duration) -> f64 {
        let Some(current) = self.value else {
            self.value = Some(input);
            return input;
        };

        if self.tau.is_zero() {
            self.value = Some(input);
            return input;
        }

        let dt = dt.as_secs_f64();
        let tau = self.tau.as_secs_f64();

        let alpha = 1.0 - (-dt / tau).exp();
        let value = current + alpha * (input - current);

        self.value = Some(value);

        value
    }
}
