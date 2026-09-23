use crate::backend::app_state::global_app_state;
use iced::{
    Theme,
    theme::{Palette, Style},
};

pub fn theme() -> Theme {
    Theme::custom("BlackRed", palette())
}

fn palette() -> Palette {
    let style = style();
    let background = style.background_color;
    let color = style.text_color;
    Palette {
        background,
        text: color,
        primary: color,
        success: color,
        warning: color,
        danger: color,
    }
}

pub fn style() -> Style {
    let (mut text_color, background_color) = {
        let config = global_app_state().config.blocking_read();
        (config.appearance.color, config.appearance.background_color)
    };

    let alpha = global_app_state().runtime.ui_alpha.get();
    text_color.a = alpha;

    Style {
        text_color,
        background_color,
    }
}

pub fn text_size() -> u32 {
    global_app_state()
        .config
        .blocking_read()
        .appearance
        .text_size
}

pub fn clock_text_size() -> u32 {
    global_app_state()
        .config
        .blocking_read()
        .appearance
        .clock_text_size
}

pub fn line_height() -> f32 {
    global_app_state()
        .config
        .blocking_read()
        .appearance
        .line_height
}
