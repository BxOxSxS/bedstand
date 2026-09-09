use std::time::Duration;

use chrono::{DateTime, Local};
use iced::{Element, Font, Length, Subscription, alignment, time, widget::text};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
}

pub struct Clock {
    time_string: String,
}

impl Clock {
    pub(crate) fn new() -> Self {
        Self {
            time_string: String::new(),
        }
    }

    pub(crate) fn update(&mut self, message: ClockMessage) {
        match message {
            ClockMessage::Tick(now) => {
                let new_time_string = now.format("%H\n%M").to_string();

                if new_time_string != self.time_string {
                    self.time_string = new_time_string;
                }
            }
        }
    }

    pub(crate) fn view(&self) -> Element<'_, ClockMessage> {
        text(&self.time_string)
            .size(350)
            .font(Font::with_name("Roboto Mono Outline"))
            .line_height(1.0)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    pub(crate) fn subscription(&self) -> Subscription<ClockMessage> {
        time::every(Duration::from_millis(500)).map(|_| ClockMessage::Tick(Local::now()))
    }
}
