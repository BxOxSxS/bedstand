use crate::backend::config::global_config;
use crate::backend::proximity;
use crate::error::*;
use crate::ui::calendar::{Calendar, CalendarMessage};
use crate::ui::clock::{Clock, ClockMessage};
use crate::ui::tap_scroll::tap_scroll;
use crate::ui::theme::style;
use crate::ui::top_bar::{TopBar, TopBarMessage};
use iced::{
    Element, Length, Subscription, Vector, event,
    futures::stream::{self, BoxStream},
    time,
    widget::{Space, column, container, float, mouse_area, row, scrollable, stack},
};
use rand::seq::SliceRandom;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub enum ViewMessage {
    Clock(ClockMessage),
    TopBar(TopBarMessage),
    Calendar(CalendarMessage),
    DriftTick,
    ClockPressed,
    AnyClick,
    SplitTimeout,
    CalendarPressed,
    Proximity,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewState {
    Clock,
    Split(bool), //bool indicates direction of preview state, true = clock, false = calendar
    Calendar,
}

pub struct View {
    clock: Clock,
    top_bar: TopBar,
    calendar: Calendar,

    drift: Drift,

    split_reset_tx: SplitResetSender,

    state: ViewState,
}

impl View {
    pub fn new() -> Self {
        info!("Creating View");
        tokio::spawn(crate::backend::server::run());

        let mut top_bar = TopBar::new();
        top_bar.update(TopBarMessage::AskUpdate);

        let (split_reset_tx, _) = tokio::sync::broadcast::channel(1);

        Self {
            clock: Clock::new(),
            top_bar,
            calendar: Calendar::new(),

            drift: Drift::new(),

            split_reset_tx: SplitResetSender(split_reset_tx),

            state: ViewState::Clock,
        }
    }

    pub fn update(&mut self, message: ViewMessage) {
        match message {
            ViewMessage::Clock(message) => {
                self.clock.update(message);
            }
            ViewMessage::TopBar(message) => {
                self.top_bar.update(message);
            }
            ViewMessage::Calendar(message) => {
                self.calendar.update(message);
            }
            ViewMessage::DriftTick => {
                self.drift.next();
            }
            ViewMessage::ClockPressed => {
                match self.state {
                    ViewState::Clock => {
                        self.top_bar.update(TopBarMessage::AskUpdate);
                        self.clock.update(ClockMessage::ToggleSeconds(true));
                        self.state = ViewState::Split(true);
                    }
                    _ => {
                        self.clock.update(ClockMessage::ToggleSeconds(false));
                        self.state = ViewState::Clock
                    }
                }
                info!("ViewState changed to {:?}", self.state);
            }
            ViewMessage::AnyClick => {
                let _ = self
                    .split_reset_tx
                    .0
                    .send(())
                    .map_err(|e| Error::new(format!("Failed to send split reset: {e}")));
            }
            ViewMessage::SplitTimeout => {
                if self.state != ViewState::Clock {
                    self.clock.update(ClockMessage::ToggleSeconds(false));
                    self.state = ViewState::Clock;
                    info!("Split timeout reached");
                }
            }
            ViewMessage::CalendarPressed => {
                match self.state {
                    ViewState::Calendar => {
                        self.state = ViewState::Split(false);
                    }
                    _ => {
                        self.state = ViewState::Calendar;
                    }
                }
                info!("ViewState changed to {:?}", self.state);
            }
            ViewMessage::Proximity => {
                match self.state {
                    ViewState::Clock => {
                        self.top_bar.update(TopBarMessage::AskUpdate);
                        self.clock.update(ClockMessage::ToggleSeconds(true));
                        self.state = ViewState::Split(true);
                    }
                    ViewState::Split(true) => {
                        self.state = ViewState::Calendar;
                    }
                    ViewState::Split(false) => {
                        self.clock.update(ClockMessage::ToggleSeconds(false));
                        self.state = ViewState::Clock;
                    }
                    ViewState::Calendar => {
                        self.state = ViewState::Split(false);
                    }
                }
                info!("ViewState changed to {:?}", self.state);
                self.update(ViewMessage::AnyClick);
            }
        }
    }

    pub fn subscription(&self) -> Subscription<ViewMessage> {
        let drift_interval = global_config().blocking_read().drift_interval;

        let clock = self.clock.subscription().map(ViewMessage::Clock);
        let top_bar = self.top_bar.subscription().map(ViewMessage::TopBar);
        let calendar = self.calendar.subscription().map(ViewMessage::Calendar);

        let drift = time::every(drift_interval).map(|_| ViewMessage::DriftTick);

        let mouse = event::listen_with(|event, _, _| match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                Some(ViewMessage::AnyClick)
            }
            iced::Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                Some(ViewMessage::AnyClick)
            }
            _ => None,
        });

        let proximity = proximity::subscription().map(|_| ViewMessage::Proximity);

        let split_timeout =
            Subscription::run_with(self.split_reset_tx.clone(), Self::split_timeout);

        Subscription::batch([
            clock,
            drift,
            split_timeout,
            top_bar,
            mouse,
            calendar,
            proximity,
        ])
    }

    pub fn view(&self) -> Element<'_, ViewMessage> {
        let fullscreen_clock = container(self.clock.view().map(ViewMessage::Clock))
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill);

        let panel = container(column![
            self.top_bar.view().map(ViewMessage::TopBar),
            tap_scroll(self.calendar.view().map(ViewMessage::Calendar))
                .style(|theme, status| {
                    let mut s = scrollable::default(theme, status);
                    s.vertical_rail.scroller.background = style().text_color.into();
                    s.horizontal_rail.scroller.background = style().text_color.into();
                    s
                })
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new().width(2).scroller_width(2),
                ))
                .width(Length::Fill)
                .height(Length::Fill)
                .on_press(ViewMessage::CalendarPressed),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill);

        let moving_clock = float(fullscreen_clock).translate(move |bounds, viewport| {
            let current_center_x = bounds.x + bounds.width / 2.0;
            let target_center_x = viewport.x + viewport.width / 6.0;
            let target_translation = target_center_x - current_center_x;

            let progress = if let ViewState::Split(_) = self.state {
                1.0
            } else {
                0.0
            };
            Vector::new(target_translation * progress, 0.0)
        });

        let content: Element<'_, ViewMessage> = if let ViewState::Split(_) = self.state {
            let clock_hit_area = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                .on_press(ViewMessage::ClockPressed);

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
        } else if self.state == ViewState::Clock {
            mouse_area(moving_clock)
                .on_press(ViewMessage::ClockPressed)
                .into()
        } else {
            panel.into()
        };

        float(content)
            .translate(move |_, _| self.drift.vector())
            .into()
    }

    fn split_timeout(reset_tx: &SplitResetSender) -> BoxStream<'static, ViewMessage> {
        let reset_rx = reset_tx.0.subscribe();
        Box::pin(stream::unfold(reset_rx, |mut reset_rx| async move {
            //wait for first click
            if reset_rx.recv().await.is_err() {
                let _ = Error::new("Split timeout reset channel closed unexpectedly");
                return None;
            }
            loop {
                let split_timeout = global_config().read().await.split_timeout;
                tokio::select! {
                    _ = tokio::time::sleep(split_timeout) => {
                        return Some((
                            ViewMessage::SplitTimeout,
                            reset_rx,
                        ));
                    }
                    result = reset_rx.recv() => {
                        match result {
                            Ok(()) => {
                                debug!("Split timeout reset");
                                continue;
                            }

                            Err(
                                tokio::sync::broadcast::error::RecvError::Lagged(_)
                            ) => {
                                debug!("Split timeout reset");
                                continue;
                            }

                            Err(
                                tokio::sync::broadcast::error::RecvError::Closed
                            ) => {
                                let _ = Error::new("Split timeout reset channel closed unexpectedly");
                                return None;
                            }
                        }
                    }
                }
            }
        }))
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

#[derive(Clone)]
struct SplitResetSender(tokio::sync::broadcast::Sender<()>);

impl std::hash::Hash for SplitResetSender {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        "split-reset-timer".hash(state);
    }
}
