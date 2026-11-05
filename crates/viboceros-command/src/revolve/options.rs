//! Shared scalar options for scripted execution and interactive axis getters.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Output {
    #[default]
    Surface,
    SubD,
}
impl Output {
    pub fn name(self) -> &'static str {
        match self {
            Self::Surface => "Surface",
            Self::SubD => "SubD",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub output: Output,
    pub segment_count: usize,
    pub start_angle_degrees: Real,
    pub sweep_degrees: Real,
    pub delete_input: bool,
    pub deformable: bool,
    pub point_count: usize,
    pub split_at_tangents: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            output: Output::Surface,
            segment_count: 10,
            start_angle_degrees: 0.,
            sweep_degrees: 360.,
            delete_input: false,
            deformable: false,
            point_count: 10,
            split_at_tangents: false,
        }
    }
}
impl Options {
    /// Apply one complete option group atomically; duplicate aliases fail.
    pub fn updated(mut self, arguments: &[&str]) -> Result<Self, CommandError> {
        let mut seen = BTreeSet::new();
        for argument in arguments {
            let (name, value) = argument
                .split_once('=')
                .or_else(|| option_name_eq(argument, "FullCircle").then_some((*argument, "Yes")))
                .ok_or(CommandError::Usage(REVOLVE_USAGE))?;
            let name = name.trim_start_matches(['_', '-']).to_ascii_lowercase();
            let key = match name.as_str() {
                "angle" | "sweepangle" | "fullcircle" => "sweep",
                _ => &name,
            };
            if !seen.insert(key.to_owned()) {
                return Err(CommandError::Usage(REVOLVE_USAGE));
            }
            let yes_no = || parse_yes_no(value).ok_or(CommandError::Usage(REVOLVE_USAGE));
            match name.as_str() {
                "startangle" => self.start_angle_degrees = parse_finite_real(value)?,
                "angle" | "sweepangle" => self.sweep_degrees = parse_finite_real(value)?,
                "fullcircle" => {
                    if yes_no()? {
                        self.sweep_degrees = 360.;
                    }
                }
                "deleteinput" => self.delete_input = yes_no()?,
                "deformable" => self.deformable = yes_no()?,
                "pointcount" => {
                    let count = value
                        .parse::<usize>()
                        .map_err(|_| CommandError::Usage(REVOLVE_USAGE))?;
                    if !(4..=viboceros_geometry::MAX_CURVE_REBUILD_POINT_COUNT).contains(&count) {
                        return Err(CommandError::Usage(REVOLVE_USAGE));
                    }
                    self.point_count = count;
                }
                "segmentcount" => {
                    let count = value
                        .parse::<usize>()
                        .map_err(|_| CommandError::Usage(REVOLVE_USAGE))?;
                    if count > 100_000 {
                        return Err(CommandError::Usage(REVOLVE_USAGE));
                    }
                    self.segment_count = count.max(4);
                }
                "output" => {
                    self.output = if value
                        .trim_start_matches('_')
                        .eq_ignore_ascii_case("Surface")
                    {
                        Output::Surface
                    } else if value.trim_start_matches('_').eq_ignore_ascii_case("SubD") {
                        Output::SubD
                    } else {
                        return Err(CommandError::UnsupportedRevolveOutput);
                    };
                }
                "splitattangents" => {
                    self.split_at_tangents = yes_no()?;
                }
                _ => return Err(CommandError::Usage(REVOLVE_USAGE)),
            }
        }
        self.angles()?;
        Ok(self)
    }
    pub(super) fn angles(self) -> Result<(Real, Real), CommandError> {
        if !self.start_angle_degrees.is_finite()
            || !self.sweep_degrees.is_finite()
            || self.sweep_degrees == 0.
            || self.sweep_degrees.abs() > 360.
        {
            return Err(CommandError::InvalidRevolutionAngle(
                self.sweep_degrees.to_string(),
            ));
        }
        let sweep = if self.sweep_degrees.abs() == 360. {
            self.sweep_degrees.signum() * std::f64::consts::TAU
        } else {
            self.sweep_degrees.to_radians()
        };
        Ok((self.start_angle_degrees.to_radians(), sweep))
    }
    pub fn command_options(self) -> String {
        let yes = |v| if v { "Yes" } else { "No" };
        format!(
            "Output={} SegmentCount={} StartAngle={} DeleteInput={} Deformable={} PointCount={} SplitAtTangents={}",
            self.output.name(),
            self.segment_count,
            self.start_angle_degrees,
            yes(self.delete_input),
            yes(self.deformable),
            self.point_count,
            yes(self.split_at_tangents)
        )
    }
}
