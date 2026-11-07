//! Rank neighboring contact branches without a quadratic scan of leaf pairs.
use super::*;
use viboceros_geometry::CircularArc3;

pub(super) fn candidate(
    source: &Curve3,
    picks: [Point3; 2],
    options: Preferences,
    tolerance: Tolerance,
) -> Result<Option<([Real; 2], CircularArc3)>, CommandError> {
    let pieces = leaf::all(source, tolerance)?;
    let seeds = [
        source.as_ref().closest_parameter(picks[0], tolerance)?,
        source.as_ref().closest_parameter(picks[1], tolerance)?,
    ];
    let index = |t| {
        pieces
            .iter()
            .position(|c| *c.as_ref().domain().end() > t)
            .unwrap_or(pieces.len() - 1)
    };
    let selected = [index(seeds[0]), index(seeds[1])];
    if selected[0] == selected[1] {
        return Ok(None);
    }
    // Coincident offset branches have no unique crossing angle. Check the
    // two picked supports against the pieces once, rather than all leaf pairs.
    for &picked in &selected {
        let Curve3::Line(a) = &pieces[picked] else {
            continue;
        };
        let u = a.direction(tolerance)?.as_vector();
        for (j, piece) in pieces.iter().enumerate() {
            if picked == j {
                continue;
            }
            let Curve3::Line(b) = piece else { continue };
            let v = b.direction(tolerance)?.as_vector();
            let separation = a.start().vector_to(b.start())?.cross(u)?.length()?;
            let b0 = a.start().vector_to(b.start())?.dot(u)?;
            let b1 = a.start().vector_to(b.end())?.dot(u)?;
            let overlap = a.length()?.min(b0.max(b1)) - 0_f64.max(b0.min(b1));
            if u.cross(v)?.length()? <= tolerance.angular()
                && (separation - 2. * options.radius).abs() <= tolerance.absolute()
                && overlap > tolerance.absolute()
            {
                return Err(CommandError::Usage(
                    "Unable to fillet the curves; the angle is too small",
                ));
            }
        }
    }
    let closed = source.as_ref().is_closed()?;
    let mut best = None;
    for i in 0..pieces.len() - 1 + usize::from(closed) {
        let first = &pieces[i];
        let second = &pieces[(i + 1) % pieces.len()];
        let result = super::super::super::curve_connections::fillet_parts(
            (first, first.as_ref().end_point()?),
            (second, second.as_ref().start_point()?),
            options.radius,
            true,
            options.styles,
            tolerance,
        );
        let parts = match result {
            Ok(parts) => parts,
            Err(CommandError::Geometry(_) | CommandError::Usage(_)) => continue,
            Err(error) => return Err(error),
        };
        let Some(Curve3::Arc(arc)) = parts.last() else {
            continue;
        };
        let (a, b) = (arc.start()?, arc.end()?);
        let parameters = [
            first.as_ref().closest_parameter(a, tolerance)?,
            second.as_ref().closest_parameter(b, tolerance)?,
        ];
        if first.as_ref().evaluate(parameters[0])?.distance_to(a)? > tolerance.absolute()
            || second.as_ref().evaluate(parameters[1])?.distance_to(b)? > tolerance.absolute()
        {
            continue;
        }
        let score = ((seeds[0] - parameters[0]).abs() + (seeds[1] - parameters[1]).abs())
            .min((seeds[0] - parameters[1]).abs() + (seeds[1] - parameters[0]).abs());
        let tie = 256.
            * Real::EPSILON
            * (*source.as_ref().domain().end() - *source.as_ref().domain().start());
        if best.as_ref().is_none_or(|(old, _, _)| score < *old - tie) {
            best = Some((score, parameters, *arc));
        }
    }
    best.map(|(_, p, a)| Some((p, a)))
        .ok_or(CommandError::Usage(
            "Unable to intersect curves or curve extensions",
        ))
}
