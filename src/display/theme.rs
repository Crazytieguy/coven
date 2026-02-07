use crossterm::style::{Attribute, Color, ContentStyle};

pub fn dim() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::DarkGrey),
        ..Default::default()
    }
}

pub fn dim_italic() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::DarkGrey),
        attributes: Attribute::Italic.into(),
        ..Default::default()
    }
}

pub fn tool_name() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::Yellow),
        ..Default::default()
    }
}

pub fn success() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::Green),
        ..Default::default()
    }
}

pub fn error() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::Red),
        ..Default::default()
    }
}

pub fn result_line() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::Green),
        attributes: Attribute::Bold.into(),
        ..Default::default()
    }
}

pub fn prompt_style() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::Cyan),
        attributes: Attribute::Bold.into(),
        ..Default::default()
    }
}
