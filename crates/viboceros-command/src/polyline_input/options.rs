//! Polyline's prompt labels, option syntax and typed scalar admission.
use super::{Input, Mode, Next, OptionResult, USAGE, Value};
use crate::{CommandError, option_name_eq, parse_yes_no};
use viboceros_drafting::PointInput;
use viboceros_geometry::{LengthUnitSystem, Tolerance};

impl Input {
    pub fn prompt(&self) -> String {
        let close = if self.preferences.persistent_close {
            "Yes"
        } else {
            "No"
        };
        if self.first.is_none() {
            return format!("Start of polyline (PersistentClose={close})");
        }
        let stage = match self.next {
            Next::Direction => "Arc direction (Undo)",
            Next::ThroughPoint => "Point on arc (Undo)",
            Next::Center => "Arc center (Undo)",
            _ => "",
        };
        if !stage.is_empty() {
            return stage.into();
        }
        let mode = match self.preferences.mode {
            Mode::Line => "Line",
            Mode::Arc => "Arc",
        };
        let helpers = if self.preferences.helpers {
            "Yes"
        } else {
            "No"
        };
        let finish = if self.segments.is_empty() {
            ""
        } else {
            ". Press Enter when done"
        };
        let close_option = if self.segments.len() >= 2 {
            " Close"
        } else {
            ""
        };
        let extras = match self.preferences.mode {
            Mode::Arc => " Direction ThroughPoint Center",
            Mode::Line if !self.segments.is_empty() => " Length",
            _ => "",
        };
        let size = self
            .length
            .map_or_else(String::new, |size| format!(" <{size}>"));
        format!(
            "Next point of polyline{finish}{size} (PersistentClose={close}{close_option} Mode={mode} Helpers={helpers}{extras} Undo)"
        )
    }

    pub fn option(
        &mut self,
        word: &str,
        units: &LengthUnitSystem,
        tolerance: Tolerance,
    ) -> Result<Option<OptionResult>, CommandError> {
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(key, v)| (key, Some(v)));
        let toggle = |current: bool| {
            value
                .map_or(Some(!current), parse_yes_no)
                .ok_or(CommandError::Usage(USAGE))
        };
        if option_name_eq(key, "PersistentClose") {
            self.preferences.persistent_close = toggle(self.preferences.persistent_close)?;
        } else if value.is_none() && option_name_eq(key, "Enter") {
            return Ok(Some(OptionResult::Finish { close: false }));
        } else if value.is_none() && option_name_eq(key, "Undo") {
            self.undo();
            return Ok(Some(OptionResult::Undo));
        } else if self.first.is_none() {
            return Ok(None);
        } else if option_name_eq(key, "Helpers") {
            self.preferences.helpers = toggle(self.preferences.helpers)?;
        } else if option_name_eq(key, "Mode") {
            self.preferences.mode = match value {
                Some(word) if option_name_eq(word, "Arc") => Mode::Arc,
                Some(word) if option_name_eq(word, "Line") => Mode::Line,
                None if self.preferences.mode == Mode::Line => Mode::Arc,
                None => Mode::Line,
                _ => return Err(CommandError::Usage(USAGE)),
            };
            self.reset_next();
        } else if option_name_eq(key, "Close") && value.is_none() {
            return Ok(Some(OptionResult::Finish { close: true }));
        } else if self.preferences.mode == Mode::Line
            && !self.segments.is_empty()
            && option_name_eq(key, "Length")
        {
            if let Some(value) = value {
                let length = PointInput::parse_length_with_units(value, units)
                    .and_then(Result::ok)
                    .ok_or_else(|| CommandError::InvalidNumber(value.to_owned()))?;
                self.value(Value::Distance(length), tolerance)?;
            }
            self.next = Next::AlongLength;
        } else if self.preferences.mode == Mode::Arc && value.is_none() {
            self.next = if option_name_eq(key, "Direction") {
                Next::Direction
            } else if option_name_eq(key, "ThroughPoint") {
                Next::ThroughPoint
            } else if option_name_eq(key, "Center") {
                Next::Center
            } else {
                return Ok(None);
            };
        } else {
            return Ok(None);
        }
        Ok(Some(OptionResult::Updated))
    }

    pub fn origin_shortcut(&self) -> bool {
        !matches!(self.next, Next::CenterEnd(_))
    }

    pub fn parse_value(
        &self,
        word: &str,
        units: &LengthUnitSystem,
    ) -> Option<Result<Value, CommandError>> {
        self.first?;
        let (kind, units): (fn(f64) -> Value, &LengthUnitSystem) =
            match (self.preferences.mode, self.next) {
                (Mode::Line, _) => (Value::Distance, units),
                (Mode::Arc, Next::CenterEnd(_)) => (Value::Angle, &LengthUnitSystem::Unset),
                (Mode::Arc, Next::End | Next::TangentEnd(_) | Next::ThroughEnd(_)) => {
                    (Value::Radius, units)
                }
                _ => return None,
            };
        PointInput::parse_length_with_units(word, units).map(|parsed| {
            parsed
                .map(kind)
                .map_err(|_| CommandError::InvalidNumber(word.to_owned()))
        })
    }
}
