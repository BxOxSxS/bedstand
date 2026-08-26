use crate::backend::config::global_config;
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
    let (text_color, background_color) = {
        let config = global_config().blocking_read();
        (config.color, config.background_color)
    };

    Style {
        text_color,
        background_color,
    }
}
