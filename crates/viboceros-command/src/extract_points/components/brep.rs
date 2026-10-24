//! Native B-rep extraction uses domain-aligned control windows without seam duplicates.
use super::*;

pub(super) fn resolve(brep: &Brep, picks: &[(Kind, usize)]) -> Result<Vec<Point3>, CommandError> {
    let mut count = 0usize;
    for &(kind, index) in picks {
        let controls = controls(brep, kind, index)?;
        count = count
            .checked_add(controls.count()?)
            .filter(|&n| n <= MAX_EXTRACTED_POINTS)
            .ok_or(CommandError::TooManyExtractedPoints {
                maximum: MAX_EXTRACTED_POINTS,
            })?;
    }
    let mut points = Vec::with_capacity(count);
    for &(kind, index) in picks {
        match controls(brep, kind, index)? {
            Controls::Surface(surface) => append_surface(&mut points, surface),
            Controls::Curve(curve) => {
                let start = curve.control_polygon_range().start;
                let count = curve.grip_count()?;
                points.extend(
                    curve.control_points()[start..start + count]
                        .iter()
                        .map(|p| p.point()),
                );
            }
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

enum Controls<'a> {
    Surface(&'a NurbsSurface),
    Curve(&'a NurbsCurve),
}
impl Controls<'_> {
    fn count(&self) -> Result<usize, CommandError> {
        Ok(match self {
            Self::Surface(surface) => surface.grip_dimensions().into_iter().product(),
            Self::Curve(curve) => curve.grip_count()?,
        })
    }
}

fn controls(brep: &Brep, kind: Kind, index: usize) -> Result<Controls<'_>, CommandError> {
    match kind {
        Kind::Face => brep
            .faces()
            .get(index)
            .map(|f| Controls::Surface(f.surface())),
        Kind::Edge => brep.edges().get(index).map(|e| {
            if brep.faces().len() == 1 {
                Controls::Surface(brep.faces()[0].surface())
            } else {
                Controls::Curve(e.curve())
            }
        }),
        Kind::Vertex => None,
    }
    .ok_or(CommandError::ExtractPointComponentUnavailable)
}

pub(super) fn append_surface(points: &mut Vec<Point3>, surface: &NurbsSurface) {
    let [count_u, count_v] = surface.grip_dimensions();
    let [u_range, v_range] = surface.control_polygon_ranges();
    for v in v_range.take(count_v) {
        for u in u_range.clone().take(count_u) {
            points.push(surface.control_point(u, v).unwrap().point());
        }
    }
}
