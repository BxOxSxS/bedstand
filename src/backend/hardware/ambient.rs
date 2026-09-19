use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::backend::app_state::global_app_state;
use crate::backend::app_state::state::{Reader, State};
use crate::backend::hardware::{find_device, proximity};
use crate::error::*;

const MIN_MAP_POINTS: usize = 2;

pub fn reader() -> Result<Reader<Option<u32>>> {
    let (device, channel) = {
        let config = global_app_state().config.blocking_read();

        (
            config.ambient_device.clone(),
            config.ambient_channel.clone(),
        )
    };

    let device_path = find_device(&device).add()?;
    let channel_path = device_path.join(channel);

    if !channel_path.is_file() {
        return Err(Error::new(format!(
            "Ambient channel path does not exist: {}",
            channel_path.display()
        )));
    }

    let reader: Reader<Option<u32>> = Arc::new(move || {
        let channel_path = channel_path.clone();

        Box::pin(async move {
            read(&channel_path).await
        })
    });

    Ok(reader)
}

pub fn spawn_reactor(
    ambient: State<Option<u32>>,
    proximity: State<Option<proximity::ProximityState>>,
) {
    tokio::spawn(async move {
        let (ambient_map, smoothing, proximity_ignore, update_interval) = {
            let config = global_app_state().config.read().await;

            (
                config.ambient_map.clone(),
                config.ambient_smoothing,
                config.ambient_proximity_ignore,
                config.ambient_update_interval,
            )
        };

        if validate_map(&ambient_map).add().is_err() {
            return;
        }

        let mut ambient_rx = ambient.subscribe();
        let mut proximity_rx = proximity.subscribe();

        let mut ema = Ema::new(smoothing);
        let mut update_interval = tokio::time::interval(update_interval);
        let mut last_update = tokio::time::Instant::now();

        let mut ambient_value = loop {
            if let Some(value) = *ambient_rx.borrow() {
                break value;
            }

            if ambient.changed(&mut ambient_rx).await.add().is_err() {
                return;
            }
        };
        let mut proximity_state = *proximity_rx.borrow();

        loop {
            tokio::select! {
                result = ambient.changed(&mut ambient_rx) => {
                    if result.add().is_err() {
                        return;
                    }

                    ambient_value = if let Some(value) = *ambient_rx.borrow() {
                        value
                    } else {
                        continue;
                    };
                }

                result = proximity.changed(&mut proximity_rx) => {
                    if result.add().is_err() {
                        return;
                    }

                    proximity_state = *proximity_rx.borrow();
                }

                _ = update_interval.tick() => {
                    if proximity_ignore
                        && proximity_state == Some(proximity::ProximityState::Near) {
                        continue;
                    }

                    let target = map_value(&ambient_map, ambient_value);

                    let now = tokio::time::Instant::now();
                    let dt = now.duration_since(last_update);
                    last_update = now;

                    let brightness = ema
                        .update(target, dt)
                        .round() as u32;

                    let _ = global_app_state()
                        .hardware
                        .read()
                        .await
                        .panel
                        .set(Some(brightness));
                }
            }
        }
    });
}

async fn read(channel_path: &Path) -> Result<Option<u32>> {
    let value = tokio::fs::read_to_string(channel_path)
        .await?
        .trim()
        .parse::<u32>()?;

    Ok(Some(value))
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