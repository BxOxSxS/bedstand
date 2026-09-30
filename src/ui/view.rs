use crate::backend::{self, app_state::global_app_state, hardware::proximity::ProximityState};
use crate::error::*;
use crate::ui::calendar::{Calendar, CalendarMessage};
use crate::ui::clock::{Clock, ClockMessage};
use crate::ui::tap_scroll::tap_scroll;
use crate::ui::theme::{line_height, style, text_size};
use crate::ui::top_bar::{TopBar, TopBarMessage};
use iced::{
    Element, Length, Subscription, Vector, event,
    futures::stream::{self, BoxStream},
    time,
    widget::{
        Column, Container, MouseArea, Space, column, container, float, mouse_area, row, scrollable,
        stack, text,
    },
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
    Proximity(ProximityState),
    OffPressed,
    ScreenOn,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewState {
    Clock,
    Split,
    Calendar,

    Off,
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

        global_app_state().hardware.blocking_write().init().unwrap();

        tokio::spawn(backend::server::run());

        let ambient_state = global_app_state().hardware.blocking_read().ambient.clone();
        let proximity_state = global_app_state()
            .hardware
            .blocking_read()
            .proximity
            .clone();
        backend::hardware::ambient::spawn_reactor(ambient_state, proximity_state);

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
                        self.state = ViewState::Split;
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
                if self.state != ViewState::Clock && self.state != ViewState::Off {
                    self.clock.update(ClockMessage::ToggleSeconds(false));
                    self.state = ViewState::Clock;
                    info!("Split timeout reached");
                }
            }
            ViewMessage::CalendarPressed => {
                match self.state {
                    ViewState::Calendar => {
                        self.state = ViewState::Split;
                    }
                    _ => {
                        self.state = ViewState::Calendar;
                    }
                }
                info!("ViewState changed to {:?}", self.state);
            }
            ViewMessage::Proximity(p) => {
                if p != ProximityState::Near {
                    return;
                }

                match self.state {
                    ViewState::Clock => {
                        self.top_bar.update(TopBarMessage::AskUpdate);
                        self.clock.update(ClockMessage::ToggleSeconds(true));
                        self.state = ViewState::Split;
                    }
                    ViewState::Split => {
                        self.state = ViewState::Calendar;
                    }
                    ViewState::Calendar => {
                        self.clock.update(ClockMessage::ToggleSeconds(false));
                        self.state = ViewState::Clock;
                    }
                    ViewState::Off => {}
                }
                info!("ViewState changed to {:?}", self.state);
                self.update(ViewMessage::AnyClick);
            }
            ViewMessage::OffPressed => {
                let screen_off_cmd = global_app_state()
                    .config
                    .blocking_read()
                    .panel
                    .screen_off_cmd
                    .clone();
                if !screen_off_cmd.is_empty() {
                    let _ = std::process::Command::new("sh")
                        .args(["-c", &screen_off_cmd])
                        .spawn()
                        .map_err(|e| {
                            Error::new(format!("Failed to execute screen off command: {e}"))
                        });
                }

                self.state = ViewState::Off;
                info!("ViewState changed to {:?}", self.state);
            }
            ViewMessage::ScreenOn => {
                let screen_on_cmd = global_app_state()
                    .config
                    .blocking_read()
                    .panel
                    .screen_on_cmd
                    .clone();
                if !screen_on_cmd.is_empty() {
                    let _ = std::process::Command::new("sh")
                        .args(["-c", &screen_on_cmd])
                        .spawn()
                        .map_err(|e| {
                            Error::new(format!("Failed to execute screen on command: {e}"))
                        });
                }

                self.update(ViewMessage::ClockPressed);
            }
        }
    }

    pub fn subscription(&self) -> Subscription<ViewMessage> {
        let drift_interval = global_app_state().config.blocking_read().drift.interval;

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

        let proximity = global_app_state()
            .hardware
            .blocking_read()
            .proximity
            .subscription(|p| {
                if let Some(p) = p {
                    ViewMessage::Proximity(p)
                } else {
                    ViewMessage::Proximity(ProximityState::Far)
                }
            });

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

    fn clock(&self) -> Container<'_, ViewMessage> {
        container(
            mouse_area(self.clock.view().map(ViewMessage::Clock))
                .on_press(ViewMessage::ClockPressed),
        )
        .width(Length::Fill)
        .height(Length::Fill)
    }

    fn panel(&self) -> Column<'_, ViewMessage> {
        column![
            self.top_bar.view().map(ViewMessage::TopBar),
            tap_scroll(self.calendar.view().map(ViewMessage::Calendar),)
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
        ]
        .width(Length::Fill)
        .height(Length::Fill)
    }

    fn off_button(&self) -> MouseArea<'_, ViewMessage> {
        mouse_area(text("X").size(text_size()).line_height(line_height()))
            .on_press(ViewMessage::OffPressed)
    }

    pub fn view(&self) -> Element<'_, ViewMessage> {
        let content: Element<'_, ViewMessage> = match self.state {
            ViewState::Clock => self.clock().into(),

            ViewState::Split => stack![
                row![
                    self.clock().width(Length::FillPortion(1)),
                    self.panel().width(Length::FillPortion(2)),
                ]
                .width(Length::Fill)
                .height(Length::Fill),
                self.off_button(),
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),

            ViewState::Calendar => self.panel().into(),

            ViewState::Off => mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                .on_press(ViewMessage::ScreenOn)
                .into(),
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
                let split_timeout = global_app_state().config.read().await.split_timeout;
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
        let drift_range = global_app_state().config.blocking_read().drift.range;

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
