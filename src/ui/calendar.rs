use crate::backend::data::{CalendarEvent, global_data};
use crate::ui::theme::{line_height, style, text_size};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike};
use iced::widget::{column, rule, text};
use iced::{Element, Length, Subscription};

#[derive(Clone, Debug)]
pub enum CalendarMessage {
    Update(Vec<CalendarEvent>),
    Tick(DateTime<Local>),
}

pub struct Calendar {
    today_header: String,
    today: Vec<CalendarEvent>,
    tomorrow_header: String,
    tomorrow: Vec<CalendarEvent>,
}

impl Calendar {
    pub fn new() -> Self {
        Self {
            today: Vec::new(),
            today_header: String::new(),
            tomorrow_header: String::new(),
            tomorrow: Vec::new(),
        }
    }

    pub fn update(&mut self, message: CalendarMessage) {
        match message {
            CalendarMessage::Update(events) => {
                self.sort_events(events);
            }
            CalendarMessage::Tick(now) => {
                let today_header = format!(
                    "{} {:02}.{:02}",
                    Self::weekday_name(now.weekday()),
                    now.day(),
                    now.month(),
                );

                if today_header != self.today_header {
                    self.today_header = today_header;
                }

                let tomorrow = now.date_naive() + Duration::days(1);
                let tomorrow_header = format!(
                    "{} {:02}.{:02}",
                    Self::weekday_name(tomorrow.weekday()),
                    tomorrow.day(),
                    tomorrow.month(),
                );

                if tomorrow_header != self.tomorrow_header {
                    self.tomorrow_header = tomorrow_header;
                }
            }
        }
    }

    pub fn subscription(&self) -> Subscription<CalendarMessage> {
        Subscription::batch([
            global_data().calendar.subscription(CalendarMessage::Update),
            iced::time::every(std::time::Duration::from_millis(1000))
                .map(|_| CalendarMessage::Tick(Local::now())),
        ])
    }

    pub fn view(&self) -> Element<'_, CalendarMessage> {
        let today_columns = if self.today.is_empty() {
            column![
                text("<brak wydarzeń>")
                    .size(text_size())
                    .line_height(line_height())
            ]
        } else {
            column(self.today.iter().map(|event| self.view_event(event)))
        };

        let today = column![
            text(&self.today_header)
                .size(text_size())
                .line_height(line_height()),
            rule::horizontal(1).style(|_| rule::Style {
                color: style().text_color,
                radius: 0.0.into(),
                fill_mode: rule::FillMode::Full,
                snap: false
            }),
            today_columns,
        ]
        .width(Length::Fill);

        let tomorrow_columns = if self.tomorrow.is_empty() {
            column![
                text("<brak wydarzeń>")
                    .size(text_size())
                    .line_height(line_height())
            ]
        } else {
            column(self.tomorrow.iter().map(|event| self.view_event(event)))
        };

        let tomorrow = column![
            text(&self.tomorrow_header)
                .size(text_size())
                .line_height(line_height()),
            rule::horizontal(1).style(|_| rule::Style {
                color: style().text_color,
                radius: 0.0.into(),
                fill_mode: rule::FillMode::Full,
                snap: false
            }),
            tomorrow_columns
        ]
        .width(Length::Fill);

        column![today, tomorrow]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_event<'a>(&self, event: &'a CalendarEvent) -> Element<'a, CalendarMessage> {
        let start = event.start.format("%H:%M").to_string();
        let end = event.end.format("%H:%M").to_string();

        let time = if start == end {
            if Self::is_all_day(event) {
                String::new()
            } else {
                start
            }
        } else {
            format!("{start}–{end}")
        };

        text(format!("{time} {}", event.name))
            .size(text_size())
            .line_height(line_height())
            .into()
    }

    fn is_all_day(event: &CalendarEvent) -> bool {
        let start = event.start;
        let end = event.end;
        // since time in data for all day events are in UTC, we need to get hour zero in UTC to local
        let hour_zero = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .with_timezone(&Local)
            .hour();

        start.hour() == hour_zero
            && start.minute() == 0
            && end.hour() == hour_zero
            && end.minute() == 0
    }

    fn weekday_name(weekday: chrono::Weekday) -> &'static str {
        match weekday {
            chrono::Weekday::Mon => "Pn",
            chrono::Weekday::Tue => "Wt",
            chrono::Weekday::Wed => "Śr",
            chrono::Weekday::Thu => "Cz",
            chrono::Weekday::Fri => "Pt",
            chrono::Weekday::Sat => "So",
            chrono::Weekday::Sun => "Nd",
        }
    }

    fn sort_events(&mut self, events: Vec<CalendarEvent>) {
        let now = Local::now();

        let today = now.date_naive();
        let tomorrow = today + Duration::days(1);
        let day_after_tomorrow = tomorrow + Duration::days(1);

        let tomorrow_start = Local
            .from_local_datetime(&tomorrow.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap();

        let day_after_tomorrow_start = Local
            .from_local_datetime(&day_after_tomorrow.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap();

        let mut today_events = Vec::new();
        let mut tomorrow_events = Vec::new();

        for event in events {
            // event already ended, ignore it.
            if event.end <= now {
                continue;
            }

            // Event must be ongoing
            if event.start < tomorrow_start {
                today_events.push(event);
                continue;
            }

            // Tomorrow events
            if event.start < day_after_tomorrow_start {
                tomorrow_events.push(event);
            }
        }

        //first sort by start time, then by end time
        today_events.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));
        tomorrow_events.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));

        self.today = today_events;
        self.tomorrow = tomorrow_events;
    }
}
