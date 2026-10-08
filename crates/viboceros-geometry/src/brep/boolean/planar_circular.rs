//! Circular planar regions retain rational arc boundaries through set operations.
use super::*;
use crate::{Circle3, CircularArc3, CurveSegment3, PolyCurve3};
use std::f64::consts::TAU;

/// Planar Boolean using exact polygon projection or certified circular loops.
/// Unsupported polygon certificates select the circular path; work, arithmetic
/// and invalid output errors never become a different geometry approximation.
pub fn boolean_planar_breps(
    breps: &[&Brep],
    operation: BrepBooleanOperation,
    tolerance: Tolerance,
) -> Result<Vec<Brep>, GeometryError> {
    match boolean_projected_planar_breps(breps, operation, tolerance) {
        Ok(values) => Ok(values.into_iter().map(|p| p.brep).collect()),
        Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. }) => {
            circular_regions(breps, operation, tolerance)
        }
        Err(error) => Err(error),
    }
}
fn unsupported() -> GeometryError {
    GeometryError::UnsupportedPolyhedralBrepBoolean {
        context: "certified complete circular planar loops required",
    }
}
fn circular_regions(
    breps: &[&Brep],
    operation: BrepBooleanOperation,
    tolerance: Tolerance,
) -> Result<Vec<Brep>, GeometryError> {
    if breps.is_empty() {
        return Err(unsupported());
    }
    if breps.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    let mut circles = Vec::new();
    let mut reference = None;
    let mut controls = 0usize;
    for b in breps {
        let [face] = b.faces.as_slice() else {
            return Err(unsupported());
        };
        let [loop_] = face.loops.as_slice() else {
            return Err(unsupported());
        };
        let [trim] = loop_.trims.as_slice() else {
            return Err(unsupported());
        };
        let edge = &b.edges[trim.edge.ok_or_else(unsupported)?];
        let curve = &edge.curve;
        controls +=
            curve.control_points().len() + curve.knots().len() + trim.curve.control_points().len();
        if controls > 16384 {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        BrepFace::try_from_certified_boundaries(
            face.surface.clone(),
            face.reversed,
            vec![loop_.trims.clone()],
        )?;
        if !curve.is_closed()? {
            return Err(unsupported());
        }
        let center = curve.circular_center(tolerance)?.ok_or_else(unsupported)?;
        let radius = curve.circular_radius(tolerance)?.ok_or_else(unsupported)?;
        if (curve.length(tolerance)? - TAU * radius).abs() > tolerance.absolute() * 16. {
            return Err(unsupported());
        }
        let plane = face.surface.plane(tolerance)?.ok_or_else(unsupported)?;
        let first = *reference.get_or_insert(plane);
        if first
            .normal()
            .as_vector()
            .cross(plane.normal().as_vector())?
            .length()?
            > tolerance.angular()
        {
            return Err(unsupported());
        }
        let original_center = center;
        let center = center.translated(
            first
                .normal()
                .as_vector()
                .scaled(-first.signed_distance_to(center)?)?,
        )?;
        let x = original_center
            .vector_to(curve.evaluate(*curve.domain().start())?)?
            .normalized_nonzero()?;
        circles.push(Circle3::try_from_frame(
            center,
            radius,
            x,
            first.normal(),
            tolerance,
        )?);
    }
    let plane = reference.unwrap();
    let mut cuts = vec![vec![0., TAU]; circles.len()];
    let mut duplicates = vec![None; circles.len()];
    for j in 0..circles.len() {
        for i in 0..j {
            let a = circles[i];
            let b = circles[j];
            let delta = a.center().vector_to(b.center())?;
            let distance = delta.length()?;
            if distance <= tolerance.absolute()
                && (a.radius() - b.radius()).abs() <= tolerance.absolute()
            {
                if duplicates[j].is_none() {
                    duplicates[j] = Some(duplicates[i].unwrap_or(i));
                }
                continue;
            }
            if distance <= tolerance.absolute() {
                continue;
            }
            let sum = a.radius() + b.radius();
            let difference = (a.radius() - b.radius()).abs();
            // Tangent circles do not create finite arc intervals. Exact pinch
            // subtraction is rejected by validated trim topology below.
            if distance >= sum - tolerance.absolute()
                || distance <= difference + tolerance.absolute()
            {
                continue;
            }
            let scale = distance.max(a.radius()).max(b.radius());
            let d = distance / scale;
            let ra = a.radius() / scale;
            let rb = b.radius() / scale;
            let along = ((d * d + ra * ra - rb * rb) / (2. * d)) * scale;
            let height = ((ra * ra - (along / scale).powi(2)).max(0.)).sqrt() * scale;
            let axis = delta.normalized_nonzero()?.as_vector();
            let perpendicular = plane
                .normal()
                .as_vector()
                .cross(axis)?
                .normalized_nonzero()?
                .as_vector();
            for sign in [-1., 1.] {
                let p = a
                    .center()
                    .translated(axis.scaled(along)?)?
                    .translated(perpendicular.scaled(sign * height)?)?;
                for (owner, circle) in [(i, a), (j, b)] {
                    let radial = circle.center().vector_to(p)?;
                    let angle = radial
                        .dot(circle.y_axis().as_vector())?
                        .atan2(radial.dot(circle.x_axis().as_vector())?)
                        .rem_euclid(TAU);
                    cuts[owner].push(angle);
                }
            }
        }
    }
    let mut arcs = Vec::new();
    for (i, circle) in circles.iter().copied().enumerate() {
        if duplicates[i].is_some() {
            continue;
        }
        cuts[i].sort_by(f64::total_cmp);
        cuts[i].dedup_by(|a, b| (*a - *b).abs() <= 64. * f64::EPSILON);
        for interval in cuts[i].windows(2) {
            if arcs.len() >= 4096 {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
            let middle = circle.point_at_angle(interval[0].midpoint(interval[1]))?;
            let mut inside = Vec::new();
            for (other, c) in circles.iter().copied().enumerate() {
                inside.push(
                    other == i
                        || duplicates[other] == Some(i)
                        || middle.distance_to(c.center())? < c.radius(),
                );
            }
            let mut outside = inside.clone();
            outside[i] = false;
            for (other, duplicate) in duplicates.iter().enumerate() {
                if *duplicate == Some(i) {
                    outside[other] = false;
                }
            }
            let includes = |values: &[bool]| match operation {
                BrepBooleanOperation::Union => values.iter().any(|v| *v),
                BrepBooleanOperation::Intersection => values.iter().all(|v| *v),
                BrepBooleanOperation::Difference => values[0] && !values.iter().skip(1).any(|v| *v),
            };
            let left = includes(&inside);
            let right = includes(&outside);
            if left == right {
                continue;
            }
            let mut arc = CircularArc3::try_from_circle_angles(circle, interval[0]..=interval[1])?;
            if right {
                arc = arc.reversed(tolerance)?;
            }
            arcs.push(arc);
        }
    }
    let mut loops = Vec::new();
    let mut work = 0usize;
    while !arcs.is_empty() {
        let first = arcs.remove(0);
        let start = first.start()?;
        let mut boundary = vec![first];
        let mut end = first.end()?;
        while !end.is_near(start, tolerance) {
            work += arcs.len();
            if work > 2_000_000 {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
            let mut matches = Vec::new();
            for (i, arc) in arcs.iter().enumerate() {
                if arc.start()?.is_near(end, tolerance) {
                    matches.push(i);
                }
            }
            let [index] = matches.as_slice() else {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            };
            let arc = arcs.remove(*index);
            end = arc.end()?;
            boundary.push(arc);
        }
        let composite = PolyCurve3::try_new(
            boundary
                .iter()
                .copied()
                .map(CurveSegment3::Arc)
                .collect::<Vec<_>>(),
        )?;
        let curve = composite.to_nurbs()?;
        let origin = plane.origin();
        let x = circles[0].x_axis().as_vector();
        let y = circles[0].y_axis().as_vector();
        let mut signed = crate::FiniteSum::default();
        for arc in &boundary {
            let center = origin.vector_to(arc.center())?;
            let from = origin.vector_to(arc.start()?)?;
            let to = origin.vector_to(arc.end()?)?;
            let sign = arc
                .normal()?
                .as_vector()
                .dot(plane.normal().as_vector())?
                .signum();
            signed.add(center.dot(x)? * (to.dot(y)? - from.dot(y)?))?;
            signed.add(-center.dot(y)? * (to.dot(x)? - from.dot(x)?))?;
            signed.add(sign * arc.radius() * arc.radius() * arc.sweep_radians())?;
        }
        loops.push((curve, composite.parameters().to_vec(), signed.total()?));
    }
    let mut result = Vec::new();
    for (outer, parameters, signed) in &loops {
        if *signed <= 0. {
            continue;
        }
        let outer_face = Brep::try_planar_face(outer, tolerance)?;
        let mut holes = Vec::new();
        let mut hole_parameters = Vec::new();
        for (hole, parameters, area) in &loops {
            if *area < 0. {
                let point = hole.evaluate(hole.domain().start().midpoint(*hole.domain().end()))?;
                let (u, v) = outer_face.faces()[0]
                    .surface()
                    .closest_parameters(point, tolerance)?;
                if outer_face.faces()[0].contains_parameters(u, v, tolerance)? {
                    holes.push(hole.clone());
                    hole_parameters.push(parameters.clone());
                }
            }
        }
        let mut brep = Brep::try_planar_face_with_holes(outer, &holes, tolerance)?;
        let mut splits = Vec::new();
        for (edge, breaks) in std::iter::once(parameters)
            .chain(&hole_parameters)
            .enumerate()
        {
            if breaks.len() > 2 {
                splits.push((edge, breaks[1..breaks.len() - 1].to_vec()));
            }
        }
        if !splits.is_empty() {
            brep = brep.try_split_edges_at_parameters(&splits, tolerance)?;
        }
        result.push(brep);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn disk(x: f64, r: f64) -> Brep {
        let c = Circle3::try_new(
            Point3::try_new(x, 0., 0.).unwrap(),
            r,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        Brep::try_planar_face(&c.to_nurbs().unwrap(), Tolerance::DEFAULT).unwrap()
    }
    #[test]
    fn circular_regions_keep_exact_arc_loci_and_analytic_overlap_areas() {
        let a = disk(0., 2.);
        let b = disk(2., 2.);
        let before = (a.clone(), b.clone());
        let common = 8. * (std::f64::consts::PI / 3.) - 2. * 3f64.sqrt();
        for (op, area) in [
            (
                BrepBooleanOperation::Union,
                8. * std::f64::consts::PI - common,
            ),
            (
                BrepBooleanOperation::Difference,
                4. * std::f64::consts::PI - common,
            ),
            (BrepBooleanOperation::Intersection, common),
        ] {
            let out = boolean_planar_breps(&[&a, &b], op, Tolerance::DEFAULT).unwrap();
            assert_eq!(out.len(), 1);
            assert!((out[0].area(Tolerance::DEFAULT).unwrap() - area).abs() < 1e-9);
            assert_eq!(
                out[0].edges().len(),
                if op == BrepBooleanOperation::Difference {
                    2
                } else {
                    3
                }
            );
            for e in out[0].edges() {
                for i in 0..17 {
                    let d = e.curve().domain();
                    let p = e
                        .curve()
                        .evaluate(*d.start() + (*d.end() - *d.start()) * i as f64 / 16.)
                        .unwrap();
                    let error = (p.distance_to(Point3::try_new(0., 0., 0.).unwrap()).unwrap() - 2.)
                        .abs()
                        .min(
                            (p.distance_to(Point3::try_new(2., 0., 0.).unwrap()).unwrap() - 2.)
                                .abs(),
                        );
                    assert!(error < 1e-12);
                }
            }
        }
        assert_eq!((a, b), before);
    }
    #[test]
    fn circular_regions_keep_disconnected_tangent_disks_and_contained_holes() {
        let a = disk(0., 2.);
        let b = disk(4., 2.);
        assert_eq!(
            boolean_planar_breps(&[&a, &b], BrepBooleanOperation::Union, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            2
        );
        assert!(
            boolean_planar_breps(
                &[&a, &b],
                BrepBooleanOperation::Intersection,
                Tolerance::DEFAULT
            )
            .unwrap()
            .is_empty()
        );
        let inner = disk(0.5, 1.);
        let ring = boolean_planar_breps(
            &[&a, &inner],
            BrepBooleanOperation::Difference,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(ring.len(), 1);
        assert_eq!(ring[0].faces()[0].loops().len(), 2);
        assert!(
            (ring[0].area(Tolerance::DEFAULT).unwrap() - 3. * std::f64::consts::PI).abs() < 1e-9
        );
        let equal = boolean_planar_breps(
            &[&a, &a, &a],
            BrepBooleanOperation::Union,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(equal.len(), 1);
        assert!(
            (equal[0].area(Tolerance::DEFAULT).unwrap() - 4. * std::f64::consts::PI).abs() < 1e-9
        );
        let b = disk(2., 2.);
        let c = disk(4., 2.);
        let chain = boolean_planar_breps(
            &[&a, &b, &c],
            BrepBooleanOperation::Union,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].edges().len(), 5);
    }
}
