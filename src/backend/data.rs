use std::sync::OnceLock;

use crate::backend::field::Field;
use crate::error::*;
use chrono::{DateTime, Local, TimeZone};
use serde::Deserialize;
use tracing::info;

static DATA: OnceLock<Data> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct Data {
    pub last_try_time: Field<Option<DateTime<Local>>>,
    pub time: Field<DateTime<Local>>,
    pub alarm: Field<Option<DateTime<Local>>>,
    pub calendar: Field<Vec<CalendarEvent>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CalendarEvent {
    pub end: DateTime<Local>,
    pub name: String,
    pub start: DateTime<Local>,
}

#[derive(Debug, Deserialize)]
struct IncomingData {
    alarm: i64,
    calendar: Vec<IncomingCalendarEvent>,
    time: i64,
}

#[derive(Debug, Deserialize)]
struct IncomingCalendarEvent {
    end: i64,
    name: String,
    start: i64,
}

impl Data {
    pub fn update(json: &str) -> Result<()> {
        info!("Updating data from JSON");

        let incoming: IncomingData = serde_json::from_str(json)?;

        let mut calendar = Vec::with_capacity(incoming.calendar.len());
        for event in incoming.calendar {
            calendar.push(incoming_event_to_calendar(event)?);
        }

        let alarm = if incoming.alarm == 0 {
            None
        } else {
            Some(ts_to_local(incoming.alarm)?)
        };

        let time = ts_to_local(incoming.time)?;

        let data = global_data();
        data.alarm.update(alarm);
        data.calendar.update(calendar);
        data.time.update(time);

        Ok(())
    }
}

impl Default for Data {
    fn default() -> Self {
        Self {
            last_try_time: Field::new(None),
            time: Field::new(Local.timestamp_opt(0, 0).single().unwrap()),
            alarm: Field::new(None),
            calendar: Field::new(Vec::new()),
        }
    }
}

pub fn global_data() -> &'static Data {
    DATA.get_or_init(Data::default)
}

fn incoming_event_to_calendar(event: IncomingCalendarEvent) -> Result<CalendarEvent> {
    if event.end < event.start {
        return Err(Error::new(format!(
            "Invalid event range: {} (start: {}, end: {})",
            event.name, event.start, event.end
        )));
    }

    Ok(CalendarEvent {
        end: ts_to_local(event.end)?,
        name: event.name,
        start: ts_to_local(event.start)?,
    })
}

fn ts_to_local(ts: i64) -> Result<DateTime<Local>> {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .ok_or_else(|| Error::new(format!("Invalid timestamp: {}", ts)))
}
