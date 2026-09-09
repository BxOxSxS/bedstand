use crate::backend::config::global_config;
use crate::backend::server::trigger_ask_update;
use crate::ui::clock::{Clock, ClockMessage};
use crate::ui::top_bar::{TopBar, TopBarMessage};
use iced::{
    Element, Length, Subscription, Theme, Vector,
    futures::{self, stream::Stream},
    time::{self},
    widget::{Space, column, container, float, mouse_area, row, stack, text},
};
use rand::seq::SliceRandom;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub enum ViewMessage {
    Clock(ClockMessage),
    TopBar(TopBarMessage),
    DriftTick,
    ScreenPressed,
    SplitTimeout,
}

pub struct View {
    clock: Clock,
    top_bar: TopBar,

    drift: Drift,

    is_split: bool,
}

impl View {
    pub fn new() -> Self {
        info!("Creating View");
        tokio::spawn(crate::backend::server::run());
        trigger_ask_update();

        Self {
            clock: Clock::new(),
            top_bar: TopBar::new(),

            drift: Drift::new(),

            is_split: false,
        }
    }

    fn split_timeout() -> impl Stream<Item = ViewMessage> {
        futures::stream::once(async {
            let split_timeout = global_config().read().await.split_timeout;
            tokio::time::sleep(split_timeout).await;
            ViewMessage::SplitTimeout
        })
    }

    pub fn update(&mut self, message: ViewMessage) {
        match message {
            ViewMessage::Clock(message) => {
                self.clock.update(message);
            }
            ViewMessage::TopBar(message) => {
                self.top_bar.update(message);
            }
            ViewMessage::DriftTick => {
                self.drift.next();
            }
            ViewMessage::ScreenPressed => {
                if self.is_split {
                    self.clock.update(ClockMessage::ToggleSeconds(false));
                    self.is_split = false;
                } else {
                    trigger_ask_update();
                    self.clock.update(ClockMessage::ToggleSeconds(true));
                    self.is_split = true;
                }
            }
            ViewMessage::SplitTimeout => {
                if !self.is_split {
                    self.clock.update(ClockMessage::ToggleSeconds(true));
                    self.is_split = true;
                }
            }
        }
    }

    pub fn subscription(&self) -> Subscription<ViewMessage> {
        let drift_interval = global_config().blocking_read().drift_interval;

        let clock = self.clock.subscription().map(ViewMessage::Clock);
        let top_bar = self.top_bar.subscription().map(ViewMessage::TopBar);

        let drift = time::every(drift_interval).map(|_| ViewMessage::DriftTick);

        let split_timeout = if self.is_split {
            Subscription::run(Self::split_timeout)
        } else {
            Subscription::none()
        };

        Subscription::batch([clock, drift, split_timeout, top_bar])
    }

    pub fn view(&self) -> Element<'_, ViewMessage> {
        let fullscreen_clock = container(self.clock.view().map(ViewMessage::Clock))
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill);

        let panel = container(column![
            self.top_bar.view().map(ViewMessage::TopBar),
            text("PLACEHOLDER") // TODO
                .size(120)
                .style(move |theme: &Theme| {
                    let color = theme.palette().text;
                    text::Style { color: Some(color) }
                })
                .width(Length::Fill)
                .height(Length::Fill)
                .center(),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill);

        let moving_clock = float(fullscreen_clock).translate(move |bounds, viewport| {
            let current_center_x = bounds.x + bounds.width / 2.0;
            let target_center_x = viewport.x + viewport.width / 6.0;
            let target_translation = target_center_x - current_center_x;

            let progress = if self.is_split {
                1.0
            } else {
                0.0
            };
            Vector::new(target_translation * progress, 0.0)
        });

        let content: Element<'_, ViewMessage> = if self.is_split {
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

            stack![split_layout, moving_clock]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            mouse_area(moving_clock)
                .on_press(ViewMessage::ScreenPressed)
                .into()
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
