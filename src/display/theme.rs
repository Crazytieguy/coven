use crossterm::style::{Attribute, Attributes, Color, ContentStyle};

pub fn dim() -> ContentStyle {
    ContentStyle {
        attributes: Attribute::Dim.into(),
        ..Default::default()
    }
}

pub fn dim_italic() -> ContentStyle {
    ContentStyle {
        attributes: Attributes::from(Attribute::Dim) | Attribute::Italic,
        ..Default::default()
    }
}

pub fn tool_name() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::Yellow),
        ..Default::default()
    }
}

pub fn tool_name_dim() -> ContentStyle {
    ContentStyle {
        foreground_color: Some(Color::Yellow),
        attributes: Attribute::Dim.into(),
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
