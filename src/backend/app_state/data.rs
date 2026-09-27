use crate::backend::app_state::state::State;
use crate::error::*;
use chrono::{DateTime, Local, TimeZone};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Clone)]
pub struct Data {
    pub last_try_time: State<Option<DateTime<Local>>>,
    pub time: State<DateTime<Local>>,
    pub alarm: State<Option<DateTime<Local>>>,
    pub calendar: State<Vec<CalendarEvent>>,
    pub battery: State<u8>,
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
    battery: u8,
}

#[derive(Debug, Deserialize)]
struct IncomingCalendarEvent {
    end: i64,
    name: String,
    start: i64,
}

impl Data {
    pub fn update(&self, json: &str) -> Result<()> {
        info!("Updating data from JSON");

        let incoming: IncomingData = serde_json::from_str(json)?;

        let mut calendar = Vec::with_capacity(incoming.calendar.len());
        for event in incoming.calendar {
            calendar.push(incoming_event_to_calendar(event).add()?);
        }

        let alarm = if incoming.alarm == 0 {
            None
        } else {
            Some(ts_to_local(incoming.alarm).add()?)
        };

        let time = ts_to_local(incoming.time).add()?;

        self.alarm.set_detached(alarm);
        self.calendar.set_detached(calendar);
        self.time.set_detached(time);
        self.battery.set_detached(incoming.battery);
        Ok(())
    }
}

impl Default for Data {
    fn default() -> Self {
        Self {
            last_try_time: State::new(None),
            time: State::new(Local.timestamp_opt(0, 0).single().unwrap()),
            alarm: State::new(None),
            calendar: State::new(Vec::new()),
            battery: State::new(0),
        }
    }
}

fn incoming_event_to_calendar(event: IncomingCalendarEvent) -> Result<CalendarEvent> {
    if event.end < event.start {
        return Err(Error::new(format!(
            "Invalid event range: {} (start: {}, end: {})",
            event.name, event.start, event.end
        )));
    }

    Ok(CalendarEvent {
        end: ts_to_local(event.end).add()?,
        name: event.name,
        start: ts_to_local(event.start).add()?,
    })
}

fn ts_to_local(ts: i64) -> Result<DateTime<Local>> {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .ok_or_else(|| Error::new(format!("Invalid timestamp: {}", ts)))
}
