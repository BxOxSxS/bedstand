use crate::backend::app_state::global_app_state;
use crate::backend::server::trigger_ask_update;
use crate::ui::theme::{line_height, text_size};
use chrono::{DateTime, Local};
use iced::widget::{container, mouse_area, row, space, stack, text};
use iced::{Element, Length, Subscription, time};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum TopBarMessage {
    Tick(DateTime<Local>),
    AlarmChanged(Option<DateTime<Local>>),
    AskUpdate,
    UpdateTimeChanged(DateTime<Local>),
    UpdateTryTimeChanged(Option<DateTime<Local>>),
    DataBatteryChanged(u8),
    BatteryChanged(Option<u8>),
}

#[derive(Clone)]
pub struct TopBar {
    alarm_string: String,
    update_str: String,
    update_indicator: String,
    data_battery_string: String,
    battery_string: String,
}

impl TopBar {
    pub fn new() -> Self {
        Self {
            alarm_string: String::new(),
            update_str: String::new(),
            update_indicator: String::new(),
            data_battery_string: String::new(),
            battery_string: String::new(),
        }
    }

    pub fn update(&mut self, message: TopBarMessage) {
        match message {
            TopBarMessage::Tick(time) => {
                let data_time = global_app_state().data.time.get();
                let try_time = global_app_state().data.last_try_time.get();

                self.update_indicator(&data_time, try_time, time);

                let new_update_str = Self::time_ago(&data_time);

                if new_update_str != self.update_str {
                    self.update_str = new_update_str;
                }

                let alarm_time = global_app_state().data.alarm.get();

                let new_alarm_string = match alarm_time {
                    Some(when) => format!("{} ({})", when.format("%H:%M"), Self::time_ago(&when)),
                    None => String::new(),
                };
                if new_alarm_string != self.alarm_string {
                    self.alarm_string = new_alarm_string;
                }
            }
            TopBarMessage::AlarmChanged(when) => {
                let new_alarm_string = match when {
                    Some(when) => format!("{} ({})", when.format("%H:%M"), Self::time_ago(&when)),
                    None => String::new(),
                };
                if new_alarm_string != self.alarm_string {
                    self.alarm_string = new_alarm_string;
                }
            }
            TopBarMessage::AskUpdate => trigger_ask_update(),
            TopBarMessage::UpdateTimeChanged(when) => {
                let new_update_str = Self::time_ago(&when).to_string();
                if new_update_str != self.update_str {
                    self.update_str = new_update_str;
                }
                let try_time = global_app_state().data.last_try_time.get();
                self.update_indicator(&when, try_time, Local::now());
            }
            TopBarMessage::UpdateTryTimeChanged(when) => {
                if let Some(when) = when {
                    let time = global_app_state().data.time.get();
                    self.update_indicator(&time, Some(when), Local::now());
                }
            }
            TopBarMessage::DataBatteryChanged(b) => {
                if b <= global_app_state()
                    .config
                    .blocking_read()
                    .remote_battery_show_threshold
                {
                    self.data_battery_string = format!("{}%", b);
                } else {
                    self.data_battery_string = String::new();
                }
            }
            TopBarMessage::BatteryChanged(b) => match b {
                Some(b) => {
                    if b <= global_app_state()
                        .config
                        .blocking_read()
                        .battery
                        .show_threshold
                    {
                        self.battery_string = format!(" {}%", b);
                    } else {
                        self.battery_string = String::new();
                    }
                }
                None => {
                    self.battery_string = String::new();
                }
            },
        }
    }

    pub fn view(&self) -> Element<'_, TopBarMessage> {
        row![
            text(&self.alarm_string)
                .size(text_size())
                .line_height(line_height()),
            space::horizontal(),
            text(format!(
                "{}{}",
                self.data_battery_string, self.battery_string
            ))
            .size(text_size())
            .line_height(line_height()),
            stack![
                text("…-99m") // virtual invisible text to reserve max space to prevent battery from changing position on different text
                    .size(text_size())
                    .line_height(line_height())
                    .color(iced::Color::TRANSPARENT),
                container(
                    mouse_area(
                        text(format!("{}{}", self.update_indicator, self.update_str))
                            .size(text_size())
                            .line_height(line_height())
                    )
                    .on_press(TopBarMessage::AskUpdate)
                )
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right),
            ],
        ]
        .width(Length::Fill)
        .into()
    }

    pub fn subscription(&self) -> Subscription<TopBarMessage> {
        Subscription::batch([
            time::every(Duration::from_millis(1000)).map(|_| TopBarMessage::Tick(Local::now())),
            global_app_state()
                .data
                .alarm
                .subscription(TopBarMessage::AlarmChanged),
            global_app_state()
                .data
                .time
                .subscription(TopBarMessage::UpdateTimeChanged),
            global_app_state()
                .data
                .last_try_time
                .subscription(TopBarMessage::UpdateTryTimeChanged),
            global_app_state()
                .data
                .battery
                .subscription(TopBarMessage::DataBatteryChanged),
            global_app_state()
                .hardware
                .blocking_read()
                .battery
                .subscription(TopBarMessage::BatteryChanged),
        ])
    }

    fn relative_time(from: &DateTime<Local>, to: &DateTime<Local>) -> String {
        let seconds = (*to - from).num_seconds();

        let sign = { if seconds < 0 { "+" } else { "-" } }.to_string();
        let seconds = seconds.abs();

        let value = match seconds {
            0..=59 => seconds,
            60..=3599 => seconds / 60,
            3600..=86_399 => seconds / 3600,
            _ => return format!("{}>1d", sign),
        };

        let suffix = match seconds {
            0..=59 => "s",
            60..=3599 => "m",
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
