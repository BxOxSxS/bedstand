mod backend;
mod error;
mod log;
mod ui;

use crate::ui::view::View;
use iced::{Error, Font, Size, application, font::Weight, window};
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<(), Error> {
    let app_state = backend::app_state::AppState::new().unwrap();
    let log_layer = app_state.logs.clone();

    backend::app_state::APP_STATE.set(app_state).unwrap();

    let filter = EnvFilter::new("warn,bedstand=debug");
    let fmt_layer = fmt::layer()
        .compact()
        .with_writer(log_layer)
        .with_ansi(false)
        .with_thread_names(false)
        .with_thread_ids(false)
        .with_line_number(false)
        .with_file(false)
        .with_target(false)
        .with_filter(filter);
    tracing_subscriber::registry().with(fmt_layer).init();

    let (size, fullscreen);
    // make sure to drop lock
    {
        let config = backend::app_state::APP_STATE
            .get()
            .unwrap()
            .config
            .blocking_read();
        size = Size::new(config.window.width, config.window.height);
        fullscreen = config.window.fullscreen;
    }

    let mut font = Font::with_name("Roboto Condensed");
    font.weight = Weight::Thin;

    application(View::new, View::update, View::view)
        .subscription(View::subscription)
        .theme(ui::theme::theme())
        .style(|_, _| ui::theme::style())
        .font(include_bytes!("../assets/fonts/RobotoMono-Outline.ttf"))
        .font(include_bytes!("../assets/fonts/RobotoCondensed-Thin.ttf"))
        .default_font(font)
        .window(window::Settings {
            size,
            resizable: false,
            decorations: false,
            fullscreen,
            ..window::Settings::default()
        })
        .run()
}
