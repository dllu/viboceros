//! Rectangular point-cloud creation, separate from NURBS point-grid surfaces.

use super::*;

const USAGE: &str = "PointGrid [3Point|Center] first-point second-point [third-base-point] [height] [XCount=n YCount=n ZCount=n]";
const MAX_POINTS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum BaseMode {
    #[default]
    Corners,
    ThreePoint,
    Center,
}

/// Grid base mode and requested counts; omitted counts retain remembered settings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointGridOptions {
    counts: [Option<usize>; 3],
    base_mode: BaseMode,
}

impl PointGridOptions {
    /// Whether the base is defined by an edge and a third, off-edge point.
    pub const fn three_point(self) -> bool {
        matches!(self.base_mode, BaseMode::ThreePoint)
    }

    /// Whether the first point is the base center rather than a corner.
    pub const fn centered(self) -> bool {
        matches!(self.base_mode, BaseMode::Center)
    }

    /// Parses an options-only interactive command prefix without changing settings.
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        let (options, coordinates) = Self::split(arguments)?;
        if !coordinates.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        // Even the smallest possible unspecified counts must fit the budget.
        options.resolve([2, 2, 1])?;
        Ok(options)
    }

    fn split<'a>(arguments: &[&'a str]) -> Result<(Self, Vec<&'a str>), CommandError> {
        let mut options = Self::default();
        let mut coordinates = Vec::new();
        let mut cursor = 0;
        while cursor < arguments.len() {
            if option_name_eq(arguments[cursor], "3Point")
                || option_name_eq(arguments[cursor], "Center")
            {
                if options.base_mode != BaseMode::Corners {
                    return Err(CommandError::Usage(USAGE));
                }
                options.base_mode = if option_name_eq(arguments[cursor], "3Point") {
                    BaseMode::ThreePoint
                } else {
                    BaseMode::Center
                };
                cursor += 1;
            } else if arguments[cursor].contains('=')
                || ["XCount", "YCount", "ZCount"]
                    .iter()
                    .any(|name| option_name_eq(arguments[cursor], name))
            {
                let (name, value, consumed) = orient_option(arguments, cursor, USAGE)?;
                let axis = ["XCount", "YCount", "ZCount"]
                    .iter()
                    .position(|expected| option_name_eq(name, expected))
                    .ok_or(CommandError::Usage(USAGE))?;
                if options.counts[axis].is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
                options.counts[axis] = Some(
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|&n| n > 0)
                        .ok_or(CommandError::Usage(USAGE))?,
                );
                cursor += consumed;
            } else {
                coordinates.push(arguments[cursor]);
                cursor += 1;
            }
        }
        Ok((options, coordinates))
    }

    fn resolve(self, defaults: [usize; 3]) -> Result<([usize; 3], usize), CommandError> {
        let mut counts = std::array::from_fn(|axis| self.counts[axis].unwrap_or(defaults[axis]));
        counts[0] = counts[0].max(2);
        counts[1] = counts[1].max(2);
        let total = counts
            .into_iter()
            .try_fold(1usize, |product, count| product.checked_mul(count))
            .filter(|&n| n <= MAX_POINTS)
            .ok_or(CommandError::TooManyPointGridPoints {
                maximum: MAX_POINTS,
            })?;
        Ok((counts, total))
    }
}

impl std::fmt::Display for PointGridOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.base_mode {
            BaseMode::Corners => {}
            BaseMode::ThreePoint => write!(f, " 3Point")?,
            BaseMode::Center => write!(f, " Center")?,
        }
        for (name, count) in ["XCount", "YCount", "ZCount"].into_iter().zip(self.counts) {
            if let Some(count) = count {
                write!(f, " {name}={count}")?;
            }
        }
        Ok(())
    }
}

pub(super) struct PointMatrixCommand {
    counts: remembered::Remembered<[usize; 3]>,
}

impl Default for PointMatrixCommand {
    fn default() -> Self {
        Self {
            counts: remembered::Remembered::new([10, 10, 1]),
        }
    }
}

impl Command for PointMatrixCommand {
    fn name(&self) -> &'static str {
        "PointGrid"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (options, coordinates) = PointGridOptions::split(arguments)?;
        let (counts, total) = options.resolve(self.counts.get())?;
        let (first, consumed) = parse_point(&coordinates)?;
        let (opposite, more) = parse_point(&coordinates[consumed..])?;
        let mut consumed = consumed + more;
        let (frame, x, y) = if options.three_point() {
            let (third, more) = parse_point(&coordinates[consumed..])?;
            consumed += more;
            let frame = Frame3::try_from_points(first, opposite, third, document.tolerance())?;
            (
                frame,
                first.distance_to(opposite)?,
                frame.coordinates_of(third)?[1],
            )
        } else {
            let frame = context.construction_plane.with_origin(first);
            let [x, y, _] = frame.coordinates_of(opposite)?;
            (frame, x, y)
        };
        if x == 0.0 || y == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "point grid base rectangle",
            }
            .into());
        }
        let remaining = &coordinates[consumed..];
        let height = match remaining {
            [] => y.abs() * if options.centered() { 2.0 } else { 1.0 },
            [value] => parse_finite_real(value)?,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        if !height.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "point grid height",
            }
            .into());
        }
        if height == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "point grid height",
            }
            .into());
        }
        // Store centered endpoints directly: doubling a finite half-width can
        // overflow even when every requested point is representable.
        let x_interval = if options.centered() {
            [-x.abs(), x.abs()]
        } else {
            [x.min(0.0), x.max(0.0)]
        };
        let y_interval = if options.centered() {
            [-y.abs(), y.abs()]
        } else {
            [y.min(0.0), y.max(0.0)]
        };
        let intervals = [
            x_interval,
            if height > 0.0 {
                [y_interval[1], y_interval[0]]
            } else {
                y_interval
            },
            [0.0, height],
        ];
        let mut points = Vec::with_capacity(total);
        for k in 0..counts[2] {
            for j in 0..counts[1] {
                for i in 0..counts[0] {
                    let index = [i, j, k];
                    let local = std::array::from_fn(|axis| {
                        let t = if counts[axis] == 1 {
                            0.0
                        } else {
                            index[axis] as Real / (counts[axis] - 1) as Real
                        };
                        // Convex interpolation avoids an overflowing endpoint difference.
                        intervals[axis][0] * (1.0 - t) + intervals[axis][1] * t
                    });
                    points.push(frame.point_at(local)?);
                }
            }
        }
        let cloud = PointCloud3::try_new(points)?;
        document.add_geometry(Geometry::PointCloud(cloud))?;
        self.counts.set(counts);
        Ok(format!("Created point grid with {total} points"))
    }
}

#[cfg(test)]
mod tests;
