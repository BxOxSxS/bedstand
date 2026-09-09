use std::time::Duration;

use chrono::{DateTime, Local};
use iced::{Element, Font, Length, Subscription, alignment, time, widget::text};
use iced::widget::stack;
use crate::ui::theme::{clock_text_size, line_height, text_size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
    ToggleSeconds(bool),
}

pub struct Clock {
    time_string: String,
    show_seconds: bool,
}

impl Clock {
    pub(crate) fn new() -> Self {
        Self {
            time_string: String::new(),
            show_seconds: false,
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
            ClockMessage::ToggleSeconds(seconds) => {
                if seconds != self.show_seconds {
                    self.show_seconds = seconds;
                }
            }
        }
    }

    pub(crate) fn view(&self) -> Element<'_, ClockMessage> {
        let main = text(&self.time_string)
            .size(clock_text_size())
            .font(Font::with_name("Roboto Mono Outline"))
            .line_height(line_height())
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .height(Length::Fill)
            .width(Length::Fill);

        if self.show_seconds {
            let seconds_str = Local::now().format("%S").to_string();

            stack![
                text(seconds_str)
                .size(text_size())
                .line_height(line_height())
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .width(Length::Fill)
                .height(Length::Fill),
                main,
            ]
                .into()
        } else {
            main.into()
        }
    }

    pub(crate) fn subscription(&self) -> Subscription<ClockMessage> {
        time::every(Duration::from_millis(250)).map(|_| ClockMessage::Tick(Local::now()))
    }
}
