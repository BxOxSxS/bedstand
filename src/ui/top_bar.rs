use crate::backend::data::global_data;
use chrono::{DateTime, Local};
use iced::widget::{column, row, space, text};
use iced::{Element, Length, Subscription, time};
use std::time::Duration;
use crate::ui::theme::{line_height, text_size};

#[derive(Debug, Clone)]
pub enum TopBarMessage {
    Tick(DateTime<Local>),
    AlarmChanged(Option<DateTime<Local>>),
    UpdateTimeChanged(DateTime<Local>),
    UpdateTryTimeChanged(Option<DateTime<Local>>),
}

#[derive(Debug, Clone)]
pub struct TopBar {
    date_string: String,
    alarm_string: String,
    update_str: String,
    update_indicator: String,
}

impl TopBar {
    pub fn new() -> Self {
        Self {
            date_string: String::new(),
            alarm_string: String::new(),
            update_str: String::new(),
            update_indicator: String::new(),
        }
    }

    pub fn update(&mut self, message: TopBarMessage) {
        match message {
            TopBarMessage::Tick(time) => {
                let new_date_string = time.format("%d.%m.%Y").to_string();
                if new_date_string != self.date_string {
                    self.date_string = new_date_string;
                }

                let data_time = global_data().time.get();
                let try_time = global_data().last_try_time.get();

                self.update_indicator(&data_time, *try_time, time);

                let new_update_str = Self::time_ago(&data_time);

                if new_update_str != self.update_str {
                    self.update_str = new_update_str;
                }

                let alarm_time = global_data().alarm.get();
                let new_alarm_string = match *alarm_time {
                    Some(when) => format!("{}({})", when.format("%H:%M"), Self::time_ago(&when)),
                    None => String::new(),
                };
                if new_alarm_string != self.alarm_string {
                    self.alarm_string = new_alarm_string;
                }
            }
            TopBarMessage::AlarmChanged(when) => {
                let new_alarm_string = match when {
                    Some(when) => format!("{}({})", when.format("%H:%M"), Self::time_ago(&when)),
                    None => String::new(),
                };
                if new_alarm_string != self.alarm_string {
                    self.alarm_string = new_alarm_string;
                }
            }
            TopBarMessage::UpdateTimeChanged(when) => {
                let new_update_str = Self::time_ago(&when).to_string();
                if new_update_str != self.update_str {
                    self.update_str = new_update_str;
                }
                let try_time = global_data().last_try_time.get();
                self.update_indicator(&when, *try_time, Local::now());
            }
            TopBarMessage::UpdateTryTimeChanged(when) => {
                if let Some(when) = when {
                    let time = global_data().time.get();
                    self.update_indicator(&time, Some(when), Local::now());
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, TopBarMessage> {
        column![
            row![
                text(&self.date_string).size(text_size()).line_height(line_height()),
                space::horizontal(),
                text(format!("{}{}", self.update_indicator, self.update_str)).size(text_size()).line_height(line_height()),
            ]
            .width(Length::Fill),
            row![text(&self.alarm_string).size(text_size()).line_height(line_height()),]
        ]
        .width(Length::Fill)
        .into()
    }

    pub fn subscription(&self) -> Subscription<TopBarMessage> {
        Subscription::batch([
            time::every(Duration::from_millis(1000)).map(|_| TopBarMessage::Tick(Local::now())),
            global_data()
                .alarm
                .subscription(TopBarMessage::AlarmChanged),
            global_data()
                .time
                .subscription(TopBarMessage::UpdateTimeChanged),
            global_data()
                .last_try_time
                .subscription(TopBarMessage::UpdateTryTimeChanged),
        ])
    }

    fn relative_time(from: &DateTime<Local>, to: &DateTime<Local>) -> String {
        let seconds = (*to - from).num_seconds();

        let mut sign = { if seconds < 0 { "+" } else { "-" } }.to_string();
        let seconds = seconds.abs();

        let value = match seconds {
            0..=59 => {
                sign = format!("{}<", sign);
                1
            }
            60..=3599 => seconds / 60,
            3600..=86_399 => seconds / 3600,
            _ => return format!("{}>1d", sign),
        };

        let suffix = match seconds {
            0..=3599 => "m",
            3600..=86_399 => "h",
            _ => "d",
        };

        format!("{sign}{value}{suffix}")
    }

    fn time_ago(to: &DateTime<Local>) -> String {
        let now = Local::now();
        Self::relative_time(to, &now)
    }

    fn update_indicator(
        &mut self,
        time: &DateTime<Local>,
        try_time: Option<DateTime<Local>>,
        now: DateTime<Local>,
    ) {
        let new_indicator = if let Some(try_time) = try_time {
            if *time > try_time {
                String::new()
            } else {
                let elapsed = (now - try_time).num_seconds();

                if elapsed <= 30 {
                    "…".to_string()
                } else {
                    "!".to_string()
                }
            }
        } else {
            String::new()
        };

        if new_indicator != self.update_indicator {
            self.update_indicator = new_indicator;
        }
    }
}
