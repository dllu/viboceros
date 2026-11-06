//! Adjoining independent line objects, admitted atomically after an edit-free draft.
use super::*;
pub const MAX_SEGMENTS: usize = 100_000;
pub const USAGE: &str = "Lines point point [point ...] [Close]";
/// Public OpenNURBS ON_SQRT_EPSILON (its decimal definition is intentional).
pub const MINIMUM_POINT_DISTANCE: f64 = 1.490116119385e-8;

/// Shared admission for complete commands and incremental point getters.
pub fn segment(start: Point3, end: Point3) -> Result<LineSegment, CommandError> {
    let line = LineSegment::try_new(start, end, Tolerance::NUMERICAL_VALIDATION)?;
    if line.length()? < MINIMUM_POINT_DISTANCE {
        return Err(CommandError::Geometry(GeometryError::Degenerate {
            context: "Lines endpoints",
        }));
    }
    Ok(line)
}

pub fn prepare(points: &[Point3], close: bool) -> Result<Vec<LineSegment>, CommandError> {
    if points.len() < if close { 3 } else { 2 } {
        return Err(CommandError::Usage(USAGE));
    }
    let closing = close && points[0] != points[points.len() - 1];
    if points.len() - 1 + usize::from(closing) > MAX_SEGMENTS {
        return Err(CommandError::Usage(
            "Lines supports at most 100000 segments",
        ));
    }
    let mut lines = points
        .windows(2)
        .map(|p| segment(p[0], p[1]))
        .collect::<Result<Vec<_>, _>>()?;
    if closing {
        lines.push(segment(points[points.len() - 1], points[0])?);
    }
    Ok(lines)
}

pub(super) struct LinesCommand;
impl Command for LinesCommand {
    fn name(&self) -> &'static str {
        "Lines"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut points = Vec::new();
        let mut consumed = 0;
        while consumed < arguments.len()
            && !arguments[consumed]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Close")
        {
            if points.len() > MAX_SEGMENTS {
                return Err(CommandError::Usage(
                    "Lines supports at most 100000 segments",
                ));
            }
            let (point, n) = parse_point(&arguments[consumed..])?;
            points.push(point);
            consumed += n;
        }
        let close = consumed < arguments.len();
        require_consumed(arguments, consumed + usize::from(close), USAGE)?;
        let lines = prepare(&points, close)?;
        let count = lines.len();
        for line in lines {
            document.add_geometry(Geometry::Line(line))?;
        }
        Ok(format!("Added {count} adjoining line objects"))
    }
}
