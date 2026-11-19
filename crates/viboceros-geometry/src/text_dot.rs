//! A point-anchored annotation with a view-independent text height.
use crate::{GeometryError, Point3};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextDotStyle {
    pub font_face: String,
    pub height_points: i32,
    pub always_on_top: bool,
    pub transparent: bool,
    pub bold: bool,
    pub italic: bool,
}

impl Default for TextDotStyle {
    fn default() -> Self {
        Self {
            font_face: "Arial".into(),
            height_points: 14,
            always_on_top: true,
            transparent: false,
            bold: false,
            italic: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextDot {
    point: Point3,
    text: String,
    secondary: String,
    style: TextDotStyle,
}

impl TextDot {
    pub fn try_new(point: Point3, text: impl Into<String>) -> Result<Self, GeometryError> {
        Self::try_styled(point, text.into(), String::new(), TextDotStyle::default())
    }

    pub fn try_styled(
        point: Point3,
        text: String,
        secondary: String,
        mut style: TextDotStyle,
    ) -> Result<Self, GeometryError> {
        if style.height_points < 3
            || [&text, &secondary, &style.font_face]
                .iter()
                .any(|value| value.contains('\0'))
        {
            return Err(GeometryError::InvalidTextDot);
        }
        style.font_face = style.font_face.trim_matches(is_dot_whitespace).into();
        if style.font_face.is_empty() {
            style.font_face = "Arial".into();
        }
        Ok(Self {
            point,
            text: normalize_text(&text),
            secondary: normalize_text(&secondary),
            style,
        })
    }

    pub fn point(&self) -> Point3 {
        self.point
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn secondary_text(&self) -> &str {
        &self.secondary
    }

    pub fn style(&self) -> &TextDotStyle {
        &self.style
    }

    pub fn with_point(&self, point: Point3) -> Self {
        Self {
            point,
            ..self.clone()
        }
    }
}

fn is_dot_whitespace(ch: char) -> bool {
    ch <= '\u{20}'
}

fn normalize_text(text: &str) -> String {
    text.trim_matches(is_dot_whitespace)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}
