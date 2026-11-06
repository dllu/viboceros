use super::*;
use viboceros_geometry::{MAX_POINT_GRID_AXIS_COUNT, MAX_POINT_GRID_DEGREE};

pub(super) struct PointGridCommand {
    pub control: bool,
    preferences: remembered::Remembered<Preferences>,
}
impl PointGridCommand {
    pub fn new(control: bool) -> Self {
        Self {
            control,
            preferences: remembered::Remembered::new(Preferences::initial(control)),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    pub degree: [usize; 2],
    pub counts: [usize; 2],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            degree: [3; 2],
            counts: [4; 2],
        }
    }
}

impl Preferences {
    pub fn initial(control: bool) -> Self {
        if control {
            Self::default()
        } else {
            Self {
                degree: [2; 2],
                counts: [3; 2],
            }
        }
    }
}
/// Finite integer tokens, including the scientific notation accepted by Rhino.
pub fn integer(text: &str) -> Option<usize> {
    text.parse::<f64>()
        .ok()
        .filter(|n| {
            n.is_finite() && n.fract() == 0. && (0. ..=MAX_POINT_GRID_AXIS_COUNT as f64).contains(n)
        })
        .map(|n| n as usize)
}

const USAGE: &str = "SrfPtGrid [DegreeU=n DegreeV=n ClosedU=Yes|No ClosedV=Yes|No] [KeepPoints=Yes|No] u-count v-count points... | SrfControlPtGrid [KeepPoints=Yes|No] [Degree=n] u-count [Degree=n] v-count points...";

impl Command for PointGridCommand {
    fn name(&self) -> &'static str {
        if self.control {
            "SrfControlPtGrid"
        } else {
            "SrfPtGrid"
        }
    }

    fn surface_point_grid_preferences(&self) -> Option<Preferences> {
        Some(self.preferences.get())
    }
    fn remember_surface_point_grid_preferences(&self, value: Preferences) -> bool {
        self.preferences.set(value);
        true
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut options = Options {
            degree: self.preferences.get().degree,
            ..Options::default()
        };
        let mut count = Vec::new();
        let mut cursor = 0;
        while cursor < arguments.len() {
            if let Some(consumed) =
                options.parse(&arguments[cursor..], self.control, count.len())?
            {
                let mut preferences = self.preferences.get();
                preferences.degree = options.degree;
                self.preferences.set(preferences);
                cursor += consumed;
            } else if count.len() < 2 {
                count.push(
                    integer(arguments[cursor])
                        .filter(|n| (2..=MAX_POINT_GRID_AXIS_COUNT).contains(n))
                        .ok_or(CommandError::Usage(USAGE))?,
                );
                let axis = count.len() - 1;
                if count[axis] > options.degree[axis] {
                    let mut preferences = self.preferences.get();
                    preferences.counts[axis] = count[axis];
                    self.preferences.set(preferences);
                }
                cursor += 1;
            } else {
                break;
            }
        }
        let count: [usize; 2] = count.try_into().map_err(|_| CommandError::Usage(USAGE))?;
        // The interactive count prompts require at least degree + 1 points.
        // The public construction APIs instead lower excessive degrees.
        if (0..2).any(|axis| count[axis] <= options.degree[axis]) {
            return Err(CommandError::Usage(USAGE));
        }
        let mut entered = Vec::with_capacity(count[0] * count[1]);
        for _ in 0..count[0] * count[1] {
            let (p, consumed) = parse_point(&arguments[cursor..])?;
            entered.push(p);
            cursor += consumed;
        }
        require_consumed(arguments, cursor, USAGE)?;
        construct(self.control, count, options, entered, document.tolerance())?.insert(document)?;
        Ok(format!(
            "Created {} by {} point-grid surface",
            count[0], count[1]
        ))
    }
}

/// Prepared output shared by scripted and incremental point-grid commands.
pub struct Plan {
    pub surface: Brep,
    pub points: Option<PointCloud3>,
}
impl Plan {
    /// Insert inside the caller's atomic command transaction.
    pub fn insert(self, document: &mut Document) -> Result<(), CommandError> {
        document.add_geometry(Geometry::Brep(self.surface))?;
        if let Some(points) = self.points {
            document.add_geometry(Geometry::PointCloud(points))?;
        }
        Ok(())
    }
}

pub fn construct(
    control: bool,
    count: [usize; 2],
    options: Options,
    entered: Vec<Point3>,
    tolerance: Tolerance,
) -> Result<Plan, CommandError> {
    if (0..2).any(|axis| {
        !(2..=MAX_POINT_GRID_AXIS_COUNT).contains(&count[axis])
            || !(1..=MAX_POINT_GRID_DEGREE).contains(&options.degree[axis])
            || count[axis] <= options.degree[axis]
            || options.closed[axis] && (control || count[axis] < 3)
    }) || entered.len() != count[0] * count[1]
    {
        return Err(CommandError::Usage(USAGE));
    }
    // Rhino input traverses V first; kernel tensor storage traverses U first.
    let mut points = Vec::with_capacity(entered.len());
    for v in 0..count[1] {
        for u in 0..count[0] {
            points.push(entered[u * count[1] + v]);
        }
    }
    let surface = if control {
        Brep::try_control_point_grid(&points, count, options.degree, tolerance)?
    } else {
        through_grid(&points, count, options.degree, options.closed, tolerance)?
    };
    let points = options
        .keep_points
        .then(|| PointCloud3::try_new(entered))
        .transpose()?;
    Ok(Plan { surface, points })
}

fn through_grid(
    points: &[Point3],
    count: [usize; 2],
    degree: [usize; 2],
    closed: [bool; 2],
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::point_grid_brep(points, count, degree, closed, tolerance)
            .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(Brep::try_through_point_grid(
            points, count, degree, closed, tolerance,
        )?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub degree: [usize; 2],
    pub closed: [bool; 2],
    pub keep_points: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            degree: [3; 2],
            closed: [false; 2],
            keep_points: false,
        }
    }
}

impl Options {
    pub fn parse(
        &mut self,
        args: &[&str],
        control: bool,
        count_axis: usize,
    ) -> Result<Option<usize>, CommandError> {
        let Some(word) = args.first() else {
            return Ok(None);
        };
        let (name, inline) = word
            .split_once('=')
            .map_or((*word, None), |(n, v)| (n, Some(v)));
        let degree_axis = if control {
            (count_axis < 2 && option_name_eq(name, "Degree")).then_some(count_axis)
        } else {
            ["DegreeU", "DegreeV"]
                .iter()
                .position(|s| option_name_eq(name, s))
        };
        if let Some(axis) = degree_axis {
            let value = inline
                .or_else(|| args.get(1).copied())
                .ok_or(CommandError::Usage(USAGE))?;
            self.degree[axis] = integer(value)
                .filter(|d| (1..=MAX_POINT_GRID_DEGREE).contains(d))
                .ok_or(CommandError::Usage(USAGE))?;
            return Ok(Some(if inline.is_some() { 1 } else { 2 }));
        }
        let closed_axis = ["ClosedU", "ClosedV"]
            .iter()
            .position(|s| option_name_eq(name, s));
        let target = if let Some(axis) = closed_axis {
            if control {
                return Err(CommandError::Usage(USAGE));
            }
            &mut self.closed[axis]
        } else if option_name_eq(name, "KeepPoints") {
            &mut self.keep_points
        } else {
            return Ok(None);
        };
        *target = if let Some(value) = inline {
            parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
        } else {
            !*target
        };
        Ok(Some(1))
    }
}

#[cfg(test)]
mod tests;

impl CommandRegistry {
    pub fn surface_point_grid_preferences(&self, control: bool) -> Preferences {
        self.lookup
            .get(&normalize_command_name(if control {
                "SrfControlPtGrid"
            } else {
                "SrfPtGrid"
            }))
            .and_then(|i| self.commands[*i].surface_point_grid_preferences())
            .unwrap_or_else(|| Preferences::initial(control))
    }
    pub fn remember_surface_point_grid_preferences(
        &self,
        control: bool,
        value: Preferences,
    ) -> bool {
        self.lookup
            .get(&normalize_command_name(if control {
                "SrfControlPtGrid"
            } else {
                "SrfPtGrid"
            }))
            .is_some_and(|i| self.commands[*i].remember_surface_point_grid_preferences(value))
    }
}
