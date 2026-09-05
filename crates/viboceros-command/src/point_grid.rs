use super::*;
use viboceros_geometry::{MAX_POINT_GRID_AXIS_COUNT, MAX_POINT_GRID_DEGREE};

pub(super) struct PointGridCommand {
    pub control: bool,
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

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut options = Options::default();
        let mut count = Vec::new();
        let mut cursor = 0;
        while cursor < arguments.len() {
            if let Some(consumed) =
                options.parse(&arguments[cursor..], self.control, count.len())?
            {
                cursor += consumed;
            } else if count.len() < 2 {
                count.push(
                    arguments[cursor]
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (2..=MAX_POINT_GRID_AXIS_COUNT).contains(n))
                        .ok_or(CommandError::Usage(USAGE))?,
                );
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
        // Command and Rhino API input traverse V first; the kernel consistently
        // stores tensor data U first. Retained point-cloud order stays untouched.
        let mut points = Vec::with_capacity(entered.len());
        for v in 0..count[1] {
            for u in 0..count[0] {
                points.push(entered[u * count[1] + v]);
            }
        }
        let surface = if self.control {
            Brep::try_control_point_grid(&points, count, options.degree, document.tolerance())?
        } else {
            Brep::try_through_point_grid(
                &points,
                count,
                options.degree,
                options.closed,
                document.tolerance(),
            )?
        };
        let cloud = options
            .keep_points
            .then(|| PointCloud3::try_new(entered))
            .transpose()?;
        document.add_geometry(Geometry::Brep(surface))?;
        if let Some(cloud) = cloud {
            document.add_geometry(Geometry::PointCloud(cloud))?;
        }
        Ok(format!(
            "Created {} by {} point-grid surface",
            count[0], count[1]
        ))
    }
}

struct Options {
    degree: [usize; 2],
    closed: [bool; 2],
    keep_points: bool,
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
    fn parse(
        &mut self,
        args: &[&str],
        control: bool,
        count_axis: usize,
    ) -> Result<Option<usize>, CommandError> {
        let (name, inline) = args[0]
            .split_once('=')
            .map_or((args[0], None), |(n, v)| (n, Some(v)));
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
            self.degree[axis] = value
                .parse::<usize>()
                .ok()
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
