//! Finite coplanar edge contacts, including whole coincident facet seams.
use super::*;
use viboceros_geometry::{CurveCurveIntersectionEvent, LineSegment};

pub(super) struct Contact {
    pub segment: [Point3; 2],
    pub whole_edge: bool,
    pub boundary: bool,
}

pub(super) fn collect(
    a: [Point3; 3],
    b: [Point3; 3],
    masks: [u32; 2],
    tolerance: Tolerance,
) -> Result<Vec<Contact>, Error> {
    let mut out = Vec::new();
    for i in 0..3 {
        let a = LineSegment::try_new(a[i], a[(i + 1) % 3], Tolerance::MESH_VALIDATION)?;
        let direction = a
            .start()
            .vector_to(a.end())?
            .normalized_nonzero()?
            .as_vector();
        for j in 0..3 {
            let b = LineSegment::try_new(b[j], b[(j + 1) % 3], Tolerance::MESH_VALIDATION)?;
            let other = b
                .start()
                .vector_to(b.end())?
                .normalized_nonzero()?
                .as_vector();
            if direction.cross(other)?.length()? > tolerance.angular().sin().abs()
                || direction.cross(a.start().vector_to(b.start())?)?.length()?
                    > tolerance.absolute()
            {
                continue;
            }
            let whole_edge = (a.start().is_near(b.start(), tolerance)
                && a.end().is_near(b.end(), tolerance))
                || (a.start().is_near(b.end(), tolerance) && a.end().is_near(b.start(), tolerance));
            for event in a
                .to_nurbs()?
                .intersection_events_with_curve(&b.to_nurbs()?, tolerance)?
            {
                if let CurveCurveIntersectionEvent::Overlap(overlap) = event {
                    let segment = [overlap.start().point(), overlap.end().point()];
                    if segment[0] != segment[1] {
                        if out.len() >= 9 {
                            return Err(Error::InvalidOutput);
                        }
                        out.push(Contact {
                            segment,
                            whole_edge,
                            boundary: masks[0] & (1 << i) != 0 && masks[1] & (1 << j) != 0,
                        });
                    }
                }
            }
        }
    }
    Ok(out)
}
