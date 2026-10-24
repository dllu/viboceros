//! Native B-rep extraction uses transposed support nets and preserves repetitions.
use super::*;

pub(super) fn resolve(brep: &Brep, picks: &[(Kind, usize)]) -> Result<Vec<Point3>, CommandError> {
    let mut count = 0usize;
    for &(kind, index) in picks {
        let controls = controls(brep, kind, index)?;
        count = count
            .checked_add(controls.len())
            .filter(|&n| n <= MAX_EXTRACTED_POINTS)
            .ok_or(CommandError::TooManyExtractedPoints {
                maximum: MAX_EXTRACTED_POINTS,
            })?;
    }
    let mut points = Vec::with_capacity(count);
    for &(kind, index) in picks {
        match kind {
            Kind::Face => append_surface(&mut points, brep.faces()[index].surface()),
            Kind::Edge if brep.faces().len() == 1 => {
                append_surface(&mut points, brep.faces()[0].surface());
            }
            Kind::Edge => points.extend(
                brep.edges()[index]
                    .curve()
                    .control_points()
                    .iter()
                    .map(|p| p.point()),
            ),
            Kind::Vertex => unreachable!(),
        }
    }
    Ok(points)
}

pub(super) fn check_picks(brep: &Brep, picks: &[(Kind, usize)]) -> Result<(), CommandError> {
    for &(kind, index) in picks {
        controls(brep, kind, index)?;
    }
    Ok(())
}

fn controls(brep: &Brep, kind: Kind, index: usize) -> Result<&[WeightedPoint3], CommandError> {
    match kind {
        Kind::Face => brep
            .faces()
            .get(index)
            .map(|f| f.surface().control_points()),
        Kind::Edge => brep.edges().get(index).map(|e| {
            if brep.faces().len() == 1 {
                brep.faces()[0].surface().control_points()
            } else {
                e.curve().control_points()
            }
        }),
        Kind::Vertex => None,
    }
    .ok_or(CommandError::ExtractPointComponentUnavailable)
}

pub(super) fn append_surface(points: &mut Vec<Point3>, surface: &NurbsSurface) {
    for v in 0..surface.control_point_count_v() {
        for u in 0..surface.control_point_count_u() {
            points.push(surface.control_point(u, v).unwrap().point());
        }
    }
}
