use crate::backend::config::global_config;
use crate::backend::server::trigger_ask_update;
use crate::ui::clock::{Clock, ClockMessage};
use iced::{
    Animation, Element, Length, Subscription, Theme, Vector, animation,
    futures::{self, stream::Stream},
    time::{self, Instant},
    widget::{Space, container, float, mouse_area, row, stack, text},
    window,
};
use rand::seq::SliceRandom;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub enum ViewMessage {
    Clock(ClockMessage),
    DriftTick,
    ScreenPressed,
    SplitTimeout,
    #[expect(dead_code)]
    Animate(Instant),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenState {
    Full,
    Entering,
    Split,
    Exiting,
}

pub struct View {
    clock: Clock,
    drift: Drift,

    screen: ScreenState,
    transition: Option<Animation<bool>>,
    now: Instant,
}

impl View {
    pub fn new() -> Self {
        info!("Creating View");
        tokio::spawn(crate::backend::server::run());
        trigger_ask_update();

        let config = global_config().blocking_read();
        Self {
            clock: Clock::new(),
            drift: Drift::new(),

            screen: ScreenState::Full,
            transition: if config.animation {
                Some(
                    Animation::new(false)
                        .duration(config.animation_duration)
                        .easing(animation::Easing::EaseInOut),
                )
            } else {
                None
            },
            now: Instant::now(),
        }
    }

    fn split_timeout() -> impl Stream<Item = ViewMessage> {
        futures::stream::once(async {
            let split_timeout = global_config().read().await.split_timeout;
            tokio::time::sleep(split_timeout).await;
            ViewMessage::SplitTimeout
        })
    }

    pub fn update(&mut self, message: ViewMessage, now: Instant) {
        self.now = now;

        let animation = global_config().blocking_read().animation;

        match message {
            ViewMessage::Clock(message) => {
                self.clock.update(message);
            }
            ViewMessage::DriftTick => {
                self.drift.next();
            }
            ViewMessage::ScreenPressed => {
                if animation {
                    match self.screen {
                        ScreenState::Full => {
                            self.screen = ScreenState::Entering;
                            if let Some(transition) = self.transition.as_mut() {
                                transition.go_mut(true, now);
                            }
                        }
                        ScreenState::Split => {
                            trigger_ask_update();
                            self.screen = ScreenState::Exiting;
                            if let Some(transition) = self.transition.as_mut() {
                                transition.go_mut(false, now);
                            }
                        }
                        ScreenState::Entering | ScreenState::Exiting => {}
                    }
                } else {
                    self.screen = match self.screen {
                        ScreenState::Full => {
                            trigger_ask_update();
                            ScreenState::Split
                        }
                        ScreenState::Split => ScreenState::Full,
                        ScreenState::Entering | ScreenState::Exiting => self.screen,
                    };
                }
            }

            ViewMessage::SplitTimeout => {
                if animation {
                    if self.screen == ScreenState::Split {
                        self.screen = ScreenState::Exiting;
                        if let Some(transition) = self.transition.as_mut() {
                            transition.go_mut(false, now);
                        }
                    }
                } else {
                    self.screen = ScreenState::Full;
                }
            }
            ViewMessage::Animate(_) => {
                if animation
                    && self
                        .transition
                        .as_ref()
                        .is_some_and(|transition| !transition.is_animating(now))
                {
                    self.screen = match self.screen {
                        ScreenState::Entering => ScreenState::Split,
                        ScreenState::Exiting => ScreenState::Full,
                        state => state,
                    };
                }
            }
        }
    }

    pub fn subscription(&self) -> Subscription<ViewMessage> {
        let (animation, drift_interval) = {
            let config = global_config().blocking_read();
            (config.animation, config.drift_interval)
        };

        let clock = self.clock.subscription().map(ViewMessage::Clock);

        let drift = time::every(drift_interval).map(|_| ViewMessage::DriftTick);

        let split_timeout = if matches!(self.screen, ScreenState::Entering | ScreenState::Split) {
            Subscription::run(Self::split_timeout)
        } else {
            Subscription::none()
        };

        let animation = if animation {
            if self
                .transition
                .as_ref()
                .is_some_and(|transition| transition.is_animating(self.now))
            {
                window::frames().map(ViewMessage::Animate)
            } else {
                Subscription::none()
            }
        } else {
            Subscription::none()
        };

        Subscription::batch([clock, drift, split_timeout, animation])
    }

    pub fn view(&self) -> Element<'_, ViewMessage> {
        let (animation, animation_duration, panel_fade_delay) = {
            let config = global_config().blocking_read();
            (
                config.animation,
                config.animation_duration,
                config.animation_panel_fade_delay,
            )
        };

        let progress = if animation {
            self.transition
                .as_ref()
                .map(|transition| transition.interpolate(0.0, 1.0, self.now))
                .unwrap_or(0.0)
        } else {
            match self.screen {
                ScreenState::Full => 0.0,
                ScreenState::Entering | ScreenState::Split | ScreenState::Exiting => 1.0,
            }
        };

        let panel_delay = panel_fade_delay.as_secs_f32() / animation_duration.as_secs_f32();

        let panel_progress = if animation {
            ((progress - panel_delay) / (1.0 - panel_delay)).clamp(0.0, 1.0)
        } else {
            match self.screen {
                ScreenState::Full => 0.0,
                ScreenState::Entering | ScreenState::Split | ScreenState::Exiting => 1.0,
            }
        };

        let fullscreen_clock = container(self.clock.view().map(ViewMessage::Clock))
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill);

        let moving_clock = float(fullscreen_clock).translate(move |bounds, viewport| {
            let current_center_x = bounds.x + bounds.width / 2.0;

            let target_center_x = viewport.x + viewport.width / 6.0;

            let target_translation = target_center_x - current_center_x;

            Vector::new(target_translation * progress, 0.0)
        });

        let panel = container(
            text("PLACEHOLDER") // TODO
                .size(120)
                .style(move |theme: &Theme| {
                    let mut color = theme.palette().text;
                    color.a = panel_progress;
                    text::Style { color: Some(color) }
                }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill);

        let content: Element<'_, ViewMessage> = match self.screen {
            ScreenState::Full => mouse_area(moving_clock)
                .on_press(ViewMessage::ScreenPressed)
                .into(),
            ScreenState::Entering | ScreenState::Split | ScreenState::Exiting => {
                let clock_hit_area =
                    mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                        .on_press(ViewMessage::ScreenPressed);

                let split_layout = row![
                    container(clock_hit_area)
                        .width(Length::FillPortion(1))
                        .height(Length::Fill),
                    panel.width(Length::FillPortion(2)).height(Length::Fill),
                ]
                .width(Length::Fill)
                .height(Length::Fill);

                stack![split_layout, moving_clock,]
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        };

        float(content)
            .translate(move |_, _| self.drift.vector())
            .into()
    }
}

struct Drift {
    positions: Vec<(i32, i32)>,
    index: usize,
    x: i32,
    y: i32,
}

impl Drift {
    fn new() -> Self {
        let mut drift = Self {
            positions: Vec::new(),
            index: 0,
            x: 0,
            y: 0,
        };

        drift.shuffle();

        drift
    }

    fn shuffle(&mut self) {
        let drift_range = global_config().blocking_read().drift_range;

        self.positions.clear();

        for y in -drift_range..=drift_range {
            for x in -drift_range..=drift_range {
                self.positions.push((x, y));
            }
        }

        let mut rng = rand::rng();
        self.positions.shuffle(&mut rng);

        self.index = 0;
    }

    fn next(&mut self) {
        debug!("Drift next {}/{}", self.index, self.positions.len());
        if self.index >= self.positions.len() {
            self.shuffle();
        }

        let (x, y) = self.positions[self.index];

        self.index += 1;
        self.x = x;
        self.y = y;
    }

    fn vector(&self) -> Vector {
        Vector::new(self.x as f32, self.y as f32)
    }
}
