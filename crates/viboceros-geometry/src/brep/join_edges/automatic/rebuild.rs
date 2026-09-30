//! Spatial boundary adjustment, separate from geometry-preserving assembly.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};

mod bounds;
mod clusters;
pub(in crate::brep::join_edges) mod image;
#[cfg(test)]
mod tests;

const BOUND_EVALUATION_MARGIN: Real = 1.001;

fn sampled_boundary_error(error: &GeometryError) -> bool {
    matches!(
        error,
        GeometryError::InvalidBrepTopology { context }
            if *context == "a p-curve interior leaves its model-space edge"
                || *context == "a model-space edge interior leaves its lifted p-curve"
    )
}

fn edge_evaluation_margin(brep: &Brep, edge_index: usize) -> Result<Real, GeometryError> {
    let mut scale: Real = 0.0;
    let mut include = |point: Point3| {
        for coordinate in point.to_array() {
            scale = scale.max(coordinate.abs());
        }
    };
    for control in brep.edges[edge_index].curve.control_points() {
        include(control.point());
    }
    for usage in brep.trim_uses() {
        if usage.trim.edge == Some(edge_index) {
            for control in brep.faces[usage.face].surface.control_points() {
                include(control.point());
            }
        }
    }
    crate::brep::tolerance::scaled_tolerance(scale, 64.0 * Real::EPSILON)
}

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
    let certified_edges = bounds::update(source, &mut result, tolerance, budget)?;
    if let Err(error) = result.validate(tolerance) {
        if !sampled_boundary_error(&error) || certified_edges.is_empty() {
            return Err(error);
        }
        // Keep exact tolerances for ordinary joins. A later sampled evaluator
        // can round a certified boundary distance upward by model-coordinate
        // ULPs, so retry that case with a local numerical allowance.
        for edge in certified_edges {
            let margin = edge_evaluation_margin(&result, edge)?;
            result.edges[edge].tolerance =
                certificate::add_bound(result.edges[edge].tolerance, margin)?;
        }
        result.validate(tolerance)?;
    }
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
        let bound = if let Some(image) =
            image::BoundaryImage::new(&joined.faces[usage.face].surface, usage.trim, budget)?
        {
            image.bound(&joined.edges[e].curve, usage.trim.reversed_3d, true, budget)?
        } else {
            None
        };
        certified[e] = certified[e].zip(bound).map(|(a, b)| a.max(b));
    }
    let prior = joined.edges.iter().map(|e| e.tolerance).collect::<Vec<_>>();
    let mut tightened = Vec::new();
    for (i, e) in joined.edges.iter_mut().enumerate() {
        if requested[i]
            && let Some(bound) = certified[i]
        {
            e.tolerance = e.tolerance.min(bound.max(floors[i]));
            if e.tolerance < prior[i] {
                tightened.push(i);
            }
        }
    }
    if let Err(error) = joined.validate(tolerance) {
        if !sampled_boundary_error(&error) || tightened.is_empty() {
            return Err(error);
        }
        for edge in tightened {
            let margin = edge_evaluation_margin(joined, edge)?;
            let padded = certificate::add_bound(joined.edges[edge].tolerance, margin)?;
            joined.edges[edge].tolerance = padded.min(prior[edge]);
        }
        joined.validate(tolerance)?;
    }
    Ok(())
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
