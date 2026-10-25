//! Shared script and interactive Loft options. Updates are validated atomically.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub style: LoftStyle,
    pub closed: bool,
    pub split_at_tangents: bool,
    pub natural_seams: bool,
    pub ends: LoftEndPoints,
    pub rebuild_point_count: usize,
    pub refit_tolerance: Option<f64>,
}

impl Options {
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        Self::default().updated(arguments)
    }

    /// Replaces options supplied in this input; duplicate keys in one input fail.
    pub fn updated(self, arguments: &[&str]) -> Result<Self, CommandError> {
        let mut result = self;
        let mut seen = 0u16;
        let mut simplify = None;
        for argument in arguments {
            let (name, value) = argument.split_once('=').unwrap_or((argument, ""));
            let key = if option_name_eq(name, "Type") {
                result.style = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                    "normal" => LoftStyle::Normal,
                    "loose" => LoftStyle::Loose,
                    "tight" => LoftStyle::Tight,
                    "straight" => LoftStyle::Straight,
                    "uniform" => LoftStyle::Uniform,
                    _ => return Err(CommandError::Usage(USAGE)),
                };
                1
            } else if option_name_eq(name, "Closed") {
                result.closed = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                2
            } else if option_name_eq(name, "SplitAtTangents") {
                result.split_at_tangents = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
                4
            } else if option_name_eq(name, "Natural") && !argument.contains('=') {
                result.natural_seams = true;
                8
            } else if option_name_eq(name, "StartPoint") {
                result.ends.start = Some(parse_point(&[value])?.0);
                16
            } else if option_name_eq(name, "EndPoint") {
                result.ends.end = Some(parse_point(&[value])?.0);
                32
            } else if option_name_eq(name, "Rebuild")
                || option_name_eq(name, "RebuildPointCount")
                || option_name_eq(name, "RebuildCount")
            {
                result.rebuild_point_count = value
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n <= viboceros_geometry::MAX_LOFT_SECTION_CONTROLS)
                    .ok_or(CommandError::Usage(USAGE))?;
                64
            } else if option_name_eq(name, "Refit") || option_name_eq(name, "RefitTolerance") {
                result.refit_tolerance = Some(
                    value
                        .parse::<f64>()
                        .ok()
                        .filter(|t| t.is_finite() && *t >= 0.)
                        .ok_or(CommandError::Usage(USAGE))?,
                );
                256
            } else if option_name_eq(name, "Simplify") {
                simplify = Some(
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "none" => 0,
                        "rebuild" => 1,
                        "refit" => 2,
                        _ => return Err(CommandError::Usage(USAGE)),
                    },
                );
                128
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            if seen & key != 0 {
                return Err(CommandError::Usage(USAGE));
            }
            seen |= key;
        }
        let rebuild_requested = seen & 64 != 0 || simplify == Some(1);
        let refit_requested = seen & 256 != 0 || simplify == Some(2);
        if rebuild_requested && refit_requested {
            return Err(CommandError::Usage(USAGE));
        }
        if seen & 64 != 0 {
            result.refit_tolerance = None;
        }
        if seen & 256 != 0 {
            result.rebuild_point_count = 0;
        }
        match simplify {
            Some(0) => {
                result.rebuild_point_count = 0;
                result.refit_tolerance = None;
            }
            Some(1) => {
                result.refit_tolerance = None;
                if result.rebuild_point_count == 0 {
                    result.rebuild_point_count = 10;
                }
            }
            Some(2) => {
                result.rebuild_point_count = 0;
                result
                    .refit_tolerance
                    .get_or_insert(Tolerance::DEFAULT.absolute());
            }
            _ => {}
        }
        if result.closed && !result.ends.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(result)
    }

    /// A lossless script spelling used when accepting the interactive getter.
    pub fn arguments(self) -> String {
        let style = match self.style {
            LoftStyle::Normal => "Normal",
            LoftStyle::Loose => "Loose",
            LoftStyle::Tight => "Tight",
            LoftStyle::Straight => "Straight",
            LoftStyle::Uniform => "Uniform",
        };
        let yes_no = |b| if b { "Yes" } else { "No" };
        let mut result = format!(
            "Type={style} Closed={} SplitAtTangents={}{}",
            yes_no(self.closed),
            yes_no(self.split_at_tangents),
            if self.natural_seams { " Natural" } else { "" },
        );
        for (name, point) in [("StartPoint", self.ends.start), ("EndPoint", self.ends.end)] {
            if let Some(point) = point {
                let [x, y, z] = point.to_array();
                result.push_str(&format!(" {name}={x},{y},{z}"));
            }
        }
        if self.rebuild_point_count != 0 {
            result.push_str(&format!(" Rebuild={}", self.rebuild_point_count));
        }
        if let Some(tolerance) = self.refit_tolerance {
            result.push_str(&format!(" Refit={tolerance}"));
        }
        result
    }
}
