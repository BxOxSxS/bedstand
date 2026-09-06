use std::time::Duration;

use chrono::{DateTime, Local};
use iced::{
    Element, Font, Length, Subscription, alignment, time,
    widget::{column, text},
};

use crate::backend::data::global_data;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
    AlarmChanged(Option<DateTime<Local>>),
}

pub struct Clock {
    time_string: String,
    date_string: String,
    alarm: String,
}

impl Clock {
    pub(crate) fn new() -> Self {
        let alarm = if let Some(alarm) = *global_data().alarm.get() {
            alarm.format("%H:%M").to_string()
        } else {
            String::new()
        };

        Self {
            time_string: String::new(),
            date_string: String::new(),
            alarm,
        }
    }

    pub(crate) fn update(&mut self, message: ClockMessage) {
        match message {
            ClockMessage::Tick(now) => {
                let new_time_string = now.format("%H\n%M").to_string();
                let new_date_string = now.format("%d.%m.%Y").to_string();

                if new_time_string != self.time_string || new_date_string != self.date_string {
                    self.time_string = new_time_string;
                    self.date_string = new_date_string;
                }
            }
            ClockMessage::AlarmChanged(alarm) => {
                self.alarm = if let Some(alarm) = alarm {
                    alarm.format("%H:%M").to_string()
                } else {
                    String::new()
                };
            }
        }
    }

    pub(crate) fn view(&self) -> Element<'_, ClockMessage> {
        let content = column![
            text(&self.time_string)
                .size(300)
                .font(Font::with_name("Roboto Mono Outline"))
                .line_height(1.0)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center),
            text(&self.date_string)
                .size(50)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center),
            text(&self.alarm)
                .size(50)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center),
        ];

        content
            .align_x(alignment::Horizontal::Center)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub(crate) fn subscription(&self) -> Subscription<ClockMessage> {
        Subscription::batch([
            time::every(Duration::from_millis(250)).map(|_| ClockMessage::Tick(Local::now())),
            global_data().alarm.subscription(ClockMessage::AlarmChanged),
        ])
    }
}
