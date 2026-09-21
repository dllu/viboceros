//! Spatial boundary adjustment, separate from geometry-preserving assembly.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};

mod bounds;
mod clusters;
#[cfg(test)]
mod tests;

pub(super) fn apply(
    source: &Brep,
    contacts: &[(usize, usize)],
    distance: Real,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Option<Brep>, GeometryError> {
    let groups = clusters::groups(source, contacts, budget)?;
    let mut valence = vec![0; source.vertices.len()];
    for edge in &source.edges {
        for &v in &edge.vertices {
            valence[v] += 1;
        }
    }
    let mut vertices = source.vertices.clone();
    let mut changed = false;
    for group in groups.iter().filter(|g| g.len() > 1) {
        let first = source.vertices[group[0]].point;
        if group.iter().all(|&i| source.vertices[i].point == first) {
            continue;
        }
        budget.charge(group.iter().map(|&i| valence[i]).sum())?;
        let center = mean(
            group
                .iter()
                .flat_map(|&i| std::iter::repeat_n(source.vertices[i].point, valence[i])),
        )?;
        for &i in group {
            certificate::point_bound(source.vertices[i].point, center, distance)
                .ok_or_else(|| invalid("adjusted boundary cluster exceeds the join distance"))?;
            vertices[i].point = center;
            changed |= source.vertices[i].point != center;
        }
    }
    if !changed {
        return Ok(None);
    }
    let mut result = source.clone();
    result.vertices = vertices;
    for edge in &mut result.edges {
        let moved = edge
            .vertices
            .map(|v| result.vertices[v].point != source.vertices[v].point);
        if !moved.into_iter().any(|m| m) {
            continue;
        }
        budget.charge(edge.curve.control_points().len())?;
        if !clamped(&edge.curve) {
            // Clamping introduces rounded knot-insertion controls. Until that
            // change has a whole-curve certificate, keep the original assembly
            // policy for the entire operation rather than silently refitting.
            return Ok(None);
        }
        let points = edge
            .curve
            .control_points()
            .iter()
            .map(|c| c.point())
            .collect::<Vec<_>>();
        let adjusted = crate::opennurbs_chord_adjust::adjust(
            &points,
            edge.vertices.map(|v| result.vertices[v].point),
        )?;
        let controls = adjusted
            .into_iter()
            .zip(edge.curve.control_points())
            .map(|(p, c)| WeightedPoint3::try_new(p, c.weight()))
            .collect::<Result<Vec<_>, _>>()?;
        let curve = NurbsCurve::try_new_rational(
            edge.curve.degree(),
            controls,
            edge.curve.knots().to_vec(),
        )?;
        certificate::curve_bound(&edge.curve, &curve, false, distance)
            .ok_or_else(|| invalid("adjusted edge exceeds the join distance"))?;
        edge.curve = curve;
    }
    bounds::update(source, &mut result, tolerance, budget)?;
    result.validate(tolerance)?;
    Ok(Some(result))
}

fn clamped(curve: &NurbsCurve) -> bool {
    let d = curve.domain();
    curve.knots()[..=curve.degree()]
        .iter()
        .all(|k| k == d.start())
        && curve.knots()[curve.knots().len() - curve.degree() - 1..]
            .iter()
            .all(|k| k == d.end())
}

/// Replace propagated uncertainty only where every incident surface image has
/// an independent whole-curve certificate. Never suppress input uncertainty.
pub(super) fn tighten_joined_edges(
    joined: &mut Brep,
    pairs: &[(usize, usize, bool)],
    mut minimum: Vec<Real>,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    let mut retained = vec![false; minimum.len()];
    let mut removed = vec![false; minimum.len()];
    for &(a, b, _) in pairs {
        retained[a] = true;
        removed[b] = true;
        minimum[a] = minimum[a].max(minimum[b]);
    }
    let mut floors = Vec::with_capacity(joined.edges.len());
    let mut requested = Vec::with_capacity(joined.edges.len());
    for i in 0..minimum.len() {
        if removed[i] {
            continue;
        }
        let e = floors.len();
        floors.push(minimum[i]);
        requested.push(retained[i] && joined.edges[e].tolerance > minimum[i]);
    }
    if !requested.iter().any(|&b| b) {
        return Ok(());
    }
    let mut certified = vec![Some(0_f64); joined.edges.len()];
    for usage in joined.trim_uses() {
        let Some(e) = usage.trim.edge.filter(|&e| requested[e]) else {
            continue;
        };
        let bound = if let Some((image, backwards)) =
            bounds::natural_image(&joined.faces[usage.face].surface, usage.trim, budget)?
        {
            certificate::refined_curve_bound(
                &joined.edges[e].curve,
                &image,
                backwards ^ usage.trim.reversed_3d,
                Real::MAX,
                |n| budget.charge(n),
            )?
        } else {
            None
        };
        certified[e] = certified[e].zip(bound).map(|(a, b)| a.max(b));
    }
    for (i, e) in joined.edges.iter_mut().enumerate() {
        if requested[i]
            && let Some(bound) = certified[i]
        {
            e.tolerance = e.tolerance.min(bound.max(floors[i]));
        }
    }
    joined.validate(tolerance)
}

fn mean(points: impl Iterator<Item = Point3>) -> Result<Point3, GeometryError> {
    let mut sum: [Rational; 3] = std::array::from_fn(|_| rational(0.));
    let mut count = 0_usize;
    for point in points {
        count += 1;
        for (sum, coordinate) in sum.iter_mut().zip(point.to_array()) {
            *sum += rational(coordinate);
        }
    }
    let count = Rational::from_integer(count.into());
    Point3::try_from([
        scalar(&(&sum[0] / &count))?,
        scalar(&(&sum[1] / &count))?,
        scalar(&(&sum[2] / &count))?,
    ])
}
