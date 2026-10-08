//! Analytic straight/circular boundaries over one planar region classification.
use super::*;
use crate::{Circle3, CircularArc3, CurveSegment3};
use std::f64::consts::TAU;
mod cuts;
#[derive(Clone)]
struct Edge {
    owner: usize,
    curve: CurveSegment3,
}
struct Region {
    boundary: Brep,
}
fn unsupported() -> GeometryError {
    GeometryError::UnsupportedPolyhedralBrepBoolean {
        context: "certified straight/circular planar boundary required",
    }
}
fn point_at(curve: &CurveSegment3, t: f64) -> Result<Point3, GeometryError> {
    let d = curve.domain();
    curve.evaluate(*d.start() + (*d.end() - *d.start()) * t)
}
fn portion(curve: &CurveSegment3, lo: f64, hi: f64) -> Result<CurveSegment3, GeometryError> {
    let d = curve.domain();
    let range =
        (*d.start() + (*d.end() - *d.start()) * lo)..=(*d.start() + (*d.end() - *d.start()) * hi);
    CurveSegment3::try_from_curve(&curve.as_ref().to_owned().try_trimmed(range)?)
}
fn xy(frame: Frame3, p: Point3) -> Result<[f64; 2], GeometryError> {
    let [x, y, _] = frame.coordinates_of(p)?;
    Ok([x, y])
}
fn cross2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn location(region: &Region, p: Point3, tolerance: Tolerance) -> Result<bool, GeometryError> {
    let face = &region.boundary.faces()[0];
    let (u, v) = face.surface().closest_parameters(p, tolerance)?;
    if face.surface().evaluate(u, v)?.distance_to(p)? > tolerance.absolute() {
        return Ok(false);
    }
    face.contains_parameters(u, v, tolerance)
}
fn project(frame: Frame3, p: Point3) -> Result<Point3, GeometryError> {
    let [x, y, _] = frame.coordinates_of(p)?;
    frame.point_at([x, y, 0.])
}
fn leaf(
    curve: &NurbsCurve,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<Option<CurveSegment3>, GeometryError> {
    let start = curve.evaluate(*curve.domain().start())?;
    let end = curve.evaluate(*curve.domain().end())?;
    if curve.is_linear_at_zero_tolerance()? {
        let a = project(frame, start)?;
        let b = project(frame, end)?;
        if a.is_near(b, tolerance) {
            return Ok(None);
        }
        let direction = start.vector_to(end)?;
        let length = direction.dot(direction)?;
        let mut previous = 0.;
        let sign = curve.control_points()[0].weight().is_sign_positive();
        for c in curve.control_points() {
            if c.weight().is_sign_positive() != sign {
                return Err(unsupported());
            }
            let t = start.vector_to(c.point())?.dot(direction)? / length;
            if t < previous - 64. * f64::EPSILON || t > 1. + 64. * f64::EPSILON {
                return Err(unsupported());
            }
            previous = t;
        }
        return Ok(Some(CurveSegment3::Line(LineSegment::try_new(
            a, b, tolerance,
        )?)));
    }
    let center = curve.circular_center(tolerance)?.ok_or_else(unsupported)?;
    let radius = curve.circular_radius(tolerance)?.ok_or_else(unsupported)?;
    let radial = center.vector_to(start)?.normalized_nonzero()?;
    let tangent = curve
        .derivative_at(*curve.domain().start())?
        .normalized_nonzero()?;
    let normal = radial
        .as_vector()
        .cross(tangent.as_vector())?
        .normalized_nonzero()?;
    if normal
        .as_vector()
        .cross(frame.z_axis().as_vector())?
        .length()?
        > tolerance.angular()
    {
        return Err(unsupported());
    }
    let projected_center = project(frame, center)?;
    let projected_start = project(frame, start)?;
    let normal = if normal.as_vector().dot(frame.z_axis().as_vector())? > 0. {
        frame.z_axis()
    } else {
        frame.z_axis().opposite()
    };
    let circle = Circle3::try_from_frame(
        projected_center,
        radius,
        projected_center
            .vector_to(projected_start)?
            .normalized_nonzero()?,
        normal,
        tolerance,
    )?;
    let sweep = if curve.is_closed()? {
        TAU
    } else {
        let r = center.vector_to(end)?;
        r.dot(circle.y_axis().as_vector())?
            .atan2(r.dot(circle.x_axis().as_vector())?)
            .rem_euclid(TAU)
    };
    if (curve.length(tolerance)? - radius * sweep).abs() > 16. * tolerance.absolute() {
        return Err(unsupported());
    }
    Ok(Some(CurveSegment3::Arc(
        CircularArc3::try_from_circle_angles(circle, 0. ..=sweep)?,
    )))
}
fn charge(work: &mut usize, count: usize) -> Result<(), GeometryError> {
    *work = work
        .checked_add(count)
        .ok_or(GeometryError::BrepBooleanWorkLimit)?;
    if *work > 2_000_000 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    Ok(())
}
fn validate_ring(
    pieces: &[CurveSegment3],
    frame: Frame3,
    tolerance: Tolerance,
    work: &mut usize,
) -> Result<(), GeometryError> {
    for (i, piece) in pieces.iter().enumerate() {
        if !piece.as_ref().end_point()?.is_near(
            pieces[(i + 1) % pieces.len()].as_ref().start_point()?,
            tolerance,
        ) {
            return Err(unsupported());
        }
    }
    for j in 0..pieces.len() {
        for i in 0..j {
            charge(work, 1)?;
            let mut cuts = [Vec::new(), Vec::new()];
            let mut overlap = Vec::new();
            intersections(
                &pieces[i],
                &pieces[j],
                frame,
                tolerance,
                &mut cuts,
                [0, 1],
                &mut overlap,
            )?;
            let adjacent = j == i + 1 || (i == 0 && j == pieces.len() - 1);
            if !adjacent && !cuts[0].is_empty() {
                return Err(unsupported());
            }
            if cuts.iter().flatten().any(|t| *t > 1e-7 && *t < 1. - 1e-7) {
                return Err(unsupported());
            }
            if !overlap.is_empty()
                && (parameter(&pieces[i], point_at(&pieces[j], 0.5)?, tolerance)?.is_some()
                    || parameter(&pieces[j], point_at(&pieces[i], 0.5)?, tolerance)?.is_some())
            {
                return Err(unsupported());
            }
        }
    }
    Ok(())
}
pub(super) fn mixed_regions(
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
    let first = breps[0].faces.first().ok_or_else(unsupported)?;
    let plane = first.surface.plane(tolerance)?.ok_or_else(unsupported)?;
    let frame = Frame3::try_from_normal(plane.origin(), plane.normal().as_vector(), tolerance)?;
    let mut edges = Vec::new();
    let mut regions = Vec::new();
    let mut controls = 0;
    let mut work = 0usize;
    for (owner, b) in breps.iter().enumerate() {
        let [face] = b.faces.as_slice() else {
            return Err(unsupported());
        };
        if face.surface.plane(tolerance)?.is_none() {
            return Err(unsupported());
        }
        let mut loops = Vec::new();
        for boundary in &face.loops {
            let mut pieces = Vec::new();
            for trim in &boundary.trims {
                let e = &b.edges[trim.edge.ok_or_else(unsupported)?];
                let source = if trim.reversed_3d {
                    e.curve.reversed()?
                } else {
                    e.curve.clone()
                };
                controls += source.control_points().len() + source.knots().len();
                if controls > 16384 {
                    return Err(GeometryError::BrepBooleanWorkLimit);
                }
                match leaf(&source, frame, tolerance) {
                    Ok(Some(value)) => pieces.push(value),
                    Ok(None) => {}
                    Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. }) => {
                        for span in source.try_bezier_spans()? {
                            if let Some(value) = leaf(&span, frame, tolerance)? {
                                pieces.push(value);
                            }
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
            let mut signed = crate::FiniteSum::default();
            for piece in &pieces {
                let a = xy(frame, piece.as_ref().start_point()?)?;
                let z = xy(frame, piece.as_ref().end_point()?)?;
                match piece {
                    CurveSegment3::Line(_) => signed.add(cross2(a, z))?,
                    CurveSegment3::Arc(arc) => {
                        let c = xy(frame, arc.center())?;
                        let sign = arc
                            .normal()?
                            .as_vector()
                            .dot(plane.normal().as_vector())?
                            .signum();
                        signed.add(
                            c[0] * (z[1] - a[1]) - c[1] * (z[0] - a[0])
                                + sign * arc.radius() * arc.radius() * arc.sweep_radians(),
                        )?;
                    }
                    _ => return Err(unsupported()),
                }
            }
            let area = signed.total()?;
            if area == 0. {
                continue;
            }
            validate_ring(&pieces, frame, tolerance, &mut work)?;
            let positive = boundary.loop_type == BrepLoopType::Outer;
            if (area > 0.) != positive {
                pieces = pieces
                    .into_iter()
                    .rev()
                    .map(|p| p.reversed())
                    .collect::<Result<Vec<_>, _>>()?;
            }
            let composite = super::planar_circular::boundary_curve(&pieces, tolerance)?;
            loops.push(composite.to_nurbs()?);
            edges.extend(pieces.into_iter().map(|curve| Edge { owner, curve }));
        }
        if loops.is_empty() {
            return Err(unsupported());
        }
        let projected = Brep::try_planar_face_with_holes(&loops[0], &loops[1..], tolerance)?;
        regions.push(Region {
            boundary: projected,
        });
    }
    let mut cuts = vec![vec![0., 1.]; edges.len()];
    let mut overlap = Vec::<(usize, usize)>::new();
    for j in 0..edges.len() {
        for i in 0..j {
            charge(&mut work, 1)?;
            if edges[i].owner == edges[j].owner {
                continue;
            }
            intersections(
                &edges[i].curve,
                &edges[j].curve,
                frame,
                tolerance,
                &mut cuts,
                [i, j],
                &mut overlap,
            )?;
        }
    }
    let mut selected = Vec::new();
    for (i, edge) in edges.iter().enumerate() {
        cuts[i].sort_by(f64::total_cmp);
        cuts[i].dedup_by(|a, b| (*a - *b).abs() < 64. * f64::EPSILON);
        for station in cuts[i].windows(2) {
            charge(&mut work, controls + regions.len())?;
            let middle = point_at(&edge.curve, station[0].midpoint(station[1]))?;
            let mut left = Vec::new();
            for (owner, region) in regions.iter().enumerate() {
                left.push(owner == edge.owner || location(region, middle, tolerance)?);
            }
            let mut right = left.clone();
            right[edge.owner] = false;
            let mut alias = false;
            for &(a, b) in &overlap {
                let other = if a == i {
                    Some(b)
                } else if b == i {
                    Some(a)
                } else {
                    None
                };
                if let Some(other) = other {
                    let curve = &edges[other].curve;
                    let t = curve.as_ref().closest_parameter(middle, tolerance)?;
                    let p = curve.evaluate(t)?;
                    if p.is_near(middle, tolerance) {
                        if other < i {
                            alias = true;
                            break;
                        }
                        let own_t = edge.curve.as_ref().closest_parameter(middle, tolerance)?;
                        let own = edge
                            .curve
                            .as_ref()
                            .evaluate_with_tangent(own_t)?
                            .tangent()
                            .as_vector();
                        let theirs = curve
                            .as_ref()
                            .evaluate_with_tangent(t)?
                            .tangent()
                            .as_vector();
                        let same = own.dot(theirs)? > 0.;
                        left[edges[other].owner] = same;
                        right[edges[other].owner] = !same;
                    }
                }
            }
            if alias {
                continue;
            }
            let includes = |v: &[bool]| match operation {
                BrepBooleanOperation::Union => v.iter().any(|v| *v),
                BrepBooleanOperation::Intersection => v.iter().all(|v| *v),
                BrepBooleanOperation::Difference => v[0] && !v.iter().skip(1).any(|v| *v),
            };
            let l = includes(&left);
            let r = includes(&right);
            if l == r {
                continue;
            }
            let mut piece = portion(&edge.curve, station[0], station[1])?;
            if r {
                piece = piece.reversed()?;
            }
            selected.push(piece);
            if selected.len() > 4096 {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
        }
    }
    super::planar_circular::assemble_regions(
        selected,
        plane,
        frame.x_axis().as_vector(),
        frame.y_axis().as_vector(),
        tolerance,
    )
}
fn parameter(
    curve: &CurveSegment3,
    p: Point3,
    tolerance: Tolerance,
) -> Result<Option<f64>, GeometryError> {
    let t = curve.as_ref().closest_parameter(p, tolerance)?;
    if !curve.evaluate(t)?.is_near(p, tolerance) {
        return Ok(None);
    }
    let d = curve.domain();
    Ok(Some((t - *d.start()) / (*d.end() - *d.start())))
}
fn intersections(
    a: &CurveSegment3,
    b: &CurveSegment3,
    frame: Frame3,
    tolerance: Tolerance,
    cuts: &mut [Vec<f64>],
    indices: [usize; 2],
    overlap: &mut Vec<(usize, usize)>,
) -> Result<(), GeometryError> {
    let [i, j] = indices;
    let mut points = Vec::new();
    match (a, b) {
        (CurveSegment3::Line(a), CurveSegment3::Line(b)) => {
            match cuts::line_line(
                [xy(frame, a.start())?, xy(frame, a.end())?],
                [xy(frame, b.start())?, xy(frame, b.end())?],
            )? {
                cuts::LineCuts::None => {}
                cuts::LineCuts::Point([t, u]) => {
                    cuts[i].push(t);
                    cuts[j].push(u);
                }
                cuts::LineCuts::Overlap { first, second } => {
                    overlap.push((i, j));
                    cuts[i].extend(first);
                    cuts[j].extend(second);
                }
            }
        }
        (CurveSegment3::Line(line), CurveSegment3::Arc(arc))
        | (CurveSegment3::Arc(arc), CurveSegment3::Line(line)) => {
            for t in cuts::line_circle(
                [xy(frame, line.start())?, xy(frame, line.end())?],
                xy(frame, arc.center())?,
                arc.radius(),
            )? {
                points.push(line.point_at(t)?);
            }
        }
        (CurveSegment3::Arc(a), CurveSegment3::Arc(b)) => {
            let delta = a.center().vector_to(b.center())?;
            let distance = delta.length()?;
            match super::planar_circle_cuts::coefficients(
                distance,
                a.radius(),
                b.radius(),
                tolerance,
            )? {
                super::planar_circle_cuts::CircleCuts::Coincident => {
                    overlap.push((i, j));
                    points.extend([a.start()?, a.end()?, b.start()?, b.end()?]);
                }
                super::planar_circle_cuts::CircleCuts::None => {}
                super::planar_circle_cuts::CircleCuts::Cross { along, height } => {
                    let axis = delta.normalized_nonzero()?.as_vector();
                    let perp = frame
                        .z_axis()
                        .as_vector()
                        .cross(axis)?
                        .normalized_nonzero()?
                        .as_vector();
                    for sign in [-1., 1.] {
                        points.push(
                            a.center()
                                .translated(axis.scaled(along)?)?
                                .translated(perp.scaled(sign * height)?)?,
                        );
                    }
                }
            }
        }
        _ => return Err(unsupported()),
    }
    for p in points {
        if let (Some(t), Some(u)) = (parameter(a, p, tolerance)?, parameter(b, p, tolerance)?) {
            cuts[i].push(t.clamp(0., 1.));
            cuts[j].push(u.clamp(0., 1.));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_line_intersections_keep_exact_stations_without_point_roundtrips() {
        let n = 2f64.powi(27);
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let line = |a: [f64; 2], b: [f64; 2]| {
            CurveSegment3::Line(
                LineSegment::try_new(
                    Point3::try_new(a[0], a[1], 0.).unwrap(),
                    Point3::try_new(b[0], b[1], 0.).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            )
        };
        let a = line([0., 0.], [n + 1., n]);
        let b = line([0.5, 0.5], [n + 0.5, n - 0.5]);
        let mut cuts = [vec![], vec![]];
        let mut overlap = Vec::new();
        intersections(
            &a,
            &b,
            frame,
            Tolerance::DEFAULT,
            &mut cuts,
            [0, 1],
            &mut overlap,
        )
        .unwrap();
        assert_eq!(cuts, [vec![0.5], vec![0.5]]);
        assert!(overlap.is_empty());
    }
    fn disk() -> Brep {
        let c = Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            2.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        Brep::try_planar_face(&c.to_nurbs().unwrap(), Tolerance::DEFAULT).unwrap()
    }
    fn rectangle(lo: f64, hi: f64) -> Brep {
        Brep::try_surface_face(
            NurbsSurface::try_bilinear(
                [[lo, -3., 0.], [hi, -3., 0.], [hi, 3., 0.], [lo, 3., 0.]]
                    .map(|p| Point3::try_from(p).unwrap()),
            )
            .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    #[test]
    fn mixed_regions_chain_partial_results_and_reject_crossing_input_loops() {
        let a = disk();
        let b = rectangle(0., 3.);
        let half = super::super::planar_circular::boolean_planar_breps(
            &[&a, &b],
            BrepBooleanOperation::Difference,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let strip = rectangle(-0.5, 0.5);
        let cap = super::super::planar_circular::boolean_planar_breps(
            &[&half[0], &strip],
            BrepBooleanOperation::Difference,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(cap.len(), 1);
        let expected = 4. * (0.25f64).acos() - 0.5 * 3.75f64.sqrt();
        assert!((cap[0].area(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-9);
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let points = [[0., 0., 0.], [3., 3., 0.], [0., 2., 0.], [4., 0., 0.]];
        let pieces = (0..4)
            .map(|i| {
                CurveSegment3::Line(
                    LineSegment::try_new(
                        Point3::try_from(points[i]).unwrap(),
                        Point3::try_from(points[(i + 1) % 4]).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert!(validate_ring(&pieces, frame, Tolerance::DEFAULT, &mut 0).is_err());
        assert!(matches!(
            charge(&mut 2_000_000, 1),
            Err(GeometryError::BrepBooleanWorkLimit)
        ));
    }

    #[test]
    fn mixed_line_arc_regions_preserve_half_disk_loci_and_split_strips() {
        let a = disk();
        let b = rectangle(0., 3.);
        let before = (a.clone(), b.clone());
        for (op, area, edges) in [
            (
                BrepBooleanOperation::Union,
                18. + 2. * std::f64::consts::PI,
                6,
            ),
            (
                BrepBooleanOperation::Difference,
                2. * std::f64::consts::PI,
                2,
            ),
            (
                BrepBooleanOperation::Intersection,
                2. * std::f64::consts::PI,
                3,
            ),
        ] {
            let result = super::super::planar_circular::boolean_planar_breps(
                &[&a, &b],
                op,
                Tolerance::DEFAULT,
            )
            .unwrap_or_else(|e| panic!("{op:?}: {e:?}"));
            assert_eq!(result.len(), 1);
            assert!((result[0].area(Tolerance::DEFAULT).unwrap() - area).abs() < 1e-9);
            assert_eq!(result[0].edges().len(), edges);
        }
        let strip = rectangle(-0.5, 0.5);
        let halves = super::super::planar_circular::boolean_planar_breps(
            &[&a, &strip],
            BrepBooleanOperation::Difference,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(halves.len(), 2);
        assert_eq!((a, b), before);
    }
}
