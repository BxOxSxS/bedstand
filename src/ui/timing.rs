use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveDate, TimeZone};
use iced::{
    Subscription,
    futures::stream::{self, BoxStream},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn aligned_every(interval: Duration) -> Subscription<DateTime<Local>> {
    assert!(
        !interval.is_zero(),
        "Aligned timer interval must not be zero"
    );
    Subscription::run_with(interval, aligned_every_stream)
}

fn aligned_every_stream(interval: &Duration) -> BoxStream<'static, DateTime<Local>> {
    let interval = *interval;

    Box::pin(stream::unfold(interval, |interval| async move {
        loop {
            let delay = until_next_boundary(interval);
            if delay > Duration::from_secs(24 * 60 * 60) {
                tokio::time::sleep(Duration::from_secs(24 * 60 * 60)).await;
                continue;
            }

            tokio::time::sleep(delay).await;
            return Some((Local::now(), interval));
        }
    }))
}

fn until_next_boundary(interval: Duration) -> Duration {
    let interval_nanos = interval.as_nanos();
    let elapsed_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let wait_nanos = interval_nanos - elapsed_nanos % interval_nanos;

    Duration::new(
        (wait_nanos / 1_000_000_000) as u64,
        (wait_nanos % 1_000_000_000) as u32,
    )
}

pub fn local_day_changes() -> Subscription<DateTime<Local>> {
    Subscription::run(local_day_stream)
}

fn local_day_stream() -> BoxStream<'static, DateTime<Local>> {
    Box::pin(stream::unfold(true, |initial| async move {
        if !initial {
            let (next_day, midnight) = next_local_day_boundary();

            loop {
                let now = Local::now();
                if now.date_naive() >= next_day {
                    return Some((now, false));
                }

                let delay = (midnight - now)
                    .to_std()
                    .unwrap_or(Duration::from_millis(1));
                tokio::time::sleep(delay).await;
            }
        }

        //Emit the first tick immediately
        Some((Local::now(), false))
    }))
}

fn next_local_day_boundary() -> (NaiveDate, DateTime<Local>) {
    let mut date = Local::now()
        .date_naive()
        .succ_opt()
        .expect("Failed to calculate next local date");

    loop {
        let mut time = date.and_hms_opt(0, 0, 0).unwrap();

        //Midnight might be skipped by a timezone change, use the first valid minute instead
        for _ in 0..24 * 60 {
            if let Some(boundary) = Local.from_local_datetime(&time).earliest() {
                return (date, boundary);
            }

            time += ChronoDuration::minutes(1);
        }

        //The whole local date was skipped by a timezone change
        date = date
            .succ_opt()
            .expect("Failed to calculate next local date");
    }
}
