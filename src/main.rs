mod backend;
mod error;
mod ui;

use crate::ui::view::View;
use iced::{Error, Font, Size, application, window};
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub const LOG_FILE: &str = "clock-iced.log";

fn main() -> Result<(), Error> {
    let filter = EnvFilter::new("warn,clock_iced=debug");
    let log_file = std::fs::File::create(LOG_FILE).unwrap();

    let console_layer = fmt::layer()
        .compact()
        .with_thread_names(false)
        .with_thread_ids(false)
        .with_line_number(false)
        .with_file(false)
        .with_target(false)
        .with_filter(filter.clone());
    let file_layer = fmt::layer()
        .compact()
        .with_writer(log_file)
        .with_ansi(false)
        .with_thread_names(false)
        .with_thread_ids(false)
        .with_line_number(false)
        .with_file(false)
        .with_target(false)
        .with_filter(filter);

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    let (size, fullscreen);
    // make sure to drop lock
    {
        let config = backend::config::Config::load().unwrap().blocking_read();
        size = Size::new(config.window_width, config.window_height);
        fullscreen = config.fullscreen;
    }

    application::timed(View::new, View::update, View::subscription, View::view)
        .theme(ui::theme::theme())
        .style(|_, _| ui::theme::style())
        .font(include_bytes!("../assets/fonts/RobotoMono-Outline.ttf"))
        .font(include_bytes!("../assets/fonts/RobotoMono-Thin.ttf"))
        .default_font(Font::with_name("Roboto Mono Thin"))
        .window(window::Settings {
            size,
            resizable: false,
            decorations: false,
            fullscreen,
            ..window::Settings::default()
        })
        .run()
}
