//! Collinear cap simplification keeps native triangular source strips.
use super::*;

pub(super) fn simplify(
    offsets: Vec<NurbsCurve>,
    tolerance: Tolerance,
) -> Result<Vec<Option<NurbsCurve>>, GeometryError> {
    let lines = offsets
        .iter()
        .map(|c| groups::is_line(c, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let n = offsets.len();
    let mut retained = Vec::new();
    for j in 0..n {
        let i = (j + n - 1) % n;
        let collinear = if lines[i] && lines[j] {
            let a =
                offsets[i].tangent_at_on_side(*offsets[i].domain().end(), ParameterSide::Left)?;
            let b = offsets[j]
                .tangent_at_on_side(*offsets[j].domain().start(), ParameterSide::Right)?;
            a.as_vector().dot(b.as_vector())? > 0.
                && a.as_vector().cross(b.as_vector())?.length()? <= 64. * Real::EPSILON
        } else {
            false
        };
        if !collinear {
            retained.push(j);
        }
    }
    if retained.is_empty() {
        return Err(invalid(
            "inset boundary has no retained curved or linear corner",
        ));
    }
    let mut offsets = offsets.into_iter().map(Some).collect::<Vec<_>>();
    for (i, &start) in retained.iter().enumerate() {
        let end = retained.get(i + 1).copied().unwrap_or(retained[0] + n);
        if end == start + 1 {
            continue;
        }
        let first = offsets[start].as_ref().unwrap();
        let last = offsets[(end - 1) % n].as_ref().unwrap();
        let a = first.evaluate(*first.domain().start())?;
        let b = last.evaluate(*last.domain().end())?;
        let merged = crate::LineSegment::try_new(a, b, tolerance)?.to_nurbs()?;
        let mut owner = start;
        for j in start + 1..end {
            let curve = offsets[j % n].as_ref().unwrap();
            let p = curve.evaluate(*curve.domain().start())?;
            if p.distance_to(a)? < p.distance_to(b)? {
                owner = j;
            }
        }
        for j in start..end {
            offsets[j % n] = None;
        }
        offsets[owner % n] = Some(merged);
    }
    Ok(offsets)
}
