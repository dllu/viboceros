//! Uniform surface rebuilding at midpoint-isocurve arc-length stations.
use crate::{CurveRef, Tolerance, curve::ArcLengthSampler, nurbs::stable_knot_mean};
use crate::{GeometryError, NurbsSurface, Point3, Real};
use faer::{Mat, prelude::*};

/// Rebuild a surface as a non-rational uniform tensor spline. Closed directions
/// use periodic interpolation with `degree` repeated controls beyond the count.
/// Counts are bounded at 256 per axis and must exceed their degrees. This interpolates
/// arc-length-spaced tensor stations, not the complete original surface locus.
pub fn try_rebuild_nurbs_surface(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
) -> Result<NurbsSurface, GeometryError> {
    rebuild(
        source,
        count,
        degree,
        [source.is_closed_u()?, source.is_closed_v()?],
    )
}

pub(crate) fn rebuild_open(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
) -> Result<NurbsSurface, GeometryError> {
    rebuild(source, count, degree, [false; 2])
}

fn rebuild(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
    closed: [bool; 2],
) -> Result<NurbsSurface, GeometryError> {
    if (0..2).any(|axis| degree[axis] == 0 || count[axis] <= degree[axis] || count[axis] > 256) {
        return Err(GeometryError::InvalidSurfaceRebuild {
            context: "requires positive degrees and degree+1..=256 controls per axis",
        });
    }
    if source.control_points().iter().any(|c| c.weight() <= 0.) {
        return Err(GeometryError::InvalidSurfaceRebuild {
            context: "requires positive weights",
        });
    }
    let axis = |axis: usize| -> Result<_, GeometryError> {
        let n = count[axis];
        let p = degree[axis];
        let spans = if closed[axis] { n } else { n - p };
        let output_count = n + if closed[axis] { p } else { 0 };
        let mut knots = if closed[axis] {
            (0..output_count + p + 1)
                .map(|i| i as Real - p as Real)
                .collect::<Vec<_>>()
        } else {
            (0..n + p + 1)
                .map(|i| i.saturating_sub(p).min(spans) as Real)
                .collect::<Vec<_>>()
        };
        if closed[axis] {
            let last = knots.len() - 1;
            knots[0] = knots[1];
            knots[last] = knots[last - 1];
        }
        let stations = if closed[axis] {
            let offset = if p.is_multiple_of(2) { 0.5 } else { 0. };
            (0..n).map(|i| i as Real + offset).collect::<Vec<_>>()
        } else {
            (0..n)
                .map(|i| stable_knot_mean(&knots[i + 1..=i + p]))
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut source_parameters = vec![0.; n];
        let domain = if axis == 0 {
            source.domain_v()
        } else {
            source.domain_u()
        };
        let c = if axis == 0 {
            source.isocurve_u(domain.start().midpoint(*domain.end()))?
        } else {
            source.isocurve_v(domain.start().midpoint(*domain.end()))?
        };
        let mut sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&c), Tolerance::DEFAULT)?;
        sampler.prepare_budgeted_repeated_sampling(32)?;
        for (i, &t) in stations.iter().enumerate() {
            if !closed[axis] && (i == 0 || i == n - 1) {
                continue;
            }
            source_parameters[i] = sampler.parameter_at_distance(
                sampler.total_length() * (t.rem_euclid(spans as Real) / spans as Real),
            )?;
        }
        let active = if axis == 0 {
            source.domain_u()
        } else {
            source.domain_v()
        };
        if !closed[axis] {
            source_parameters[0] = *active.start();
            source_parameters[n - 1] = *active.end();
        }
        let rows = stations
            .iter()
            .map(|&t| {
                let basis =
                    crate::nurbs::bspline_basis_values_extended(&knots, p, output_count, t)?;
                let mut row = vec![0.; n];
                for (i, b) in basis.into_iter().enumerate() {
                    row[i % n] += b;
                }
                Ok(row)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok((
            knots,
            source_parameters,
            Mat::from_fn(n, n, |i, j| rows[i][j]),
        ))
    };
    let (ku, u, au) = axis(0)?;
    let (kv, v, av) = axis(1)?;
    let mut points = Vec::with_capacity(count[0] * count[1]);
    for &y in &v {
        for &x in &u {
            points.push(source.evaluate(x, y)?);
        }
    }
    let mut origin = points[0].to_array();
    if points
        .iter()
        .any(|p| (0..3).any(|k| !(p.to_array()[k] - origin[k]).is_finite()))
    {
        origin = [0.; 3];
    }
    let scale = points
        .iter()
        .flat_map(|p| (0..3).map(move |k| (p.to_array()[k] - origin[k]).abs()))
        .fold(1., Real::max);
    let rhs = Mat::from_fn(count[0], count[1] * 3, |i, j| {
        (points[j / 3 * count[0] + i].to_array()[j % 3] - origin[j % 3]) / scale
    });
    let first = solve(&au, &rhs)?;
    let rhs = Mat::from_fn(count[1], count[0] * 3, |i, j| first[(j / 3, i * 3 + j % 3)]);
    let solved = solve(&av, &rhs)?;
    let output_count = std::array::from_fn::<_, 2, _>(|axis| {
        count[axis] + if closed[axis] { degree[axis] } else { 0 }
    });
    let mut controls = Vec::with_capacity(output_count[0] * output_count[1]);
    for j in 0..output_count[1] {
        for i in 0..output_count[0] {
            controls.push(Point3::try_from(std::array::from_fn(|k| {
                solved[(j % count[1], (i % count[0]) * 3 + k)].mul_add(scale, origin[k])
            }))?);
        }
    }
    if !closed[0] && !closed[1] {
        for index in [0, count[0] - 1, (count[1] - 1) * count[0], points.len() - 1] {
            controls[index] = points[index];
        }
    }
    // Exact constant boundary data must remain a pole after both tensor solves.
    // LU roundoff can otherwise turn a singular side into a tiny spatial loop.
    for j in [0, count[1] - 1] {
        let side = if j == 0 {
            source.domain_v().start().to_owned()
        } else {
            source.domain_v().end().to_owned()
        };
        let curve = source.isocurve_u(side)?;
        let point = curve.control_points()[0].point();
        if !closed[1] && curve.control_points().iter().all(|p| p.point() == point) {
            let target_j = if j == 0 { 0 } else { output_count[1] - 1 };
            for i in 0..output_count[0] {
                controls[target_j * output_count[0] + i] = point;
            }
        }
    }
    for i in [0, count[0] - 1] {
        let side = if i == 0 {
            source.domain_u().start().to_owned()
        } else {
            source.domain_u().end().to_owned()
        };
        let curve = source.isocurve_v(side)?;
        let point = curve.control_points()[0].point();
        if !closed[0] && curve.control_points().iter().all(|p| p.point() == point) {
            let target_i = if i == 0 { 0 } else { output_count[0] - 1 };
            for j in 0..output_count[1] {
                controls[j * output_count[0] + target_i] = point;
            }
        }
    }
    NurbsSurface::try_new(
        degree[0],
        degree[1],
        output_count[0],
        output_count[1],
        controls,
        ku,
        kv,
    )
}

fn solve(matrix: &Mat<Real>, targets: &Mat<Real>) -> Result<Mat<Real>, GeometryError> {
    let lu = matrix.full_piv_lu();
    if (0..matrix.nrows()).any(|i| !lu.U()[(i, i)].is_finite() || lu.U()[(i, i)] == 0.) {
        return Err(GeometryError::SingularSystem);
    }
    let solution = lu.solve(targets);
    for j in 0..targets.ncols() {
        for i in 0..matrix.nrows() {
            let mut residual = -targets[(i, j)];
            let mut magnitude = targets[(i, j)].abs();
            for k in 0..matrix.ncols() {
                let x = solution[(k, j)];
                if !x.is_finite() {
                    return Err(GeometryError::SingularSystem);
                }
                residual = matrix[(i, k)].mul_add(x, residual);
                magnitude += (matrix[(i, k)] * x).abs();
            }
            if !residual.is_finite()
                || !magnitude.is_finite()
                || residual.abs()
                    > 128. * Real::EPSILON * matrix.ncols() as Real * magnitude.max(1.)
            {
                return Err(GeometryError::SingularSystem);
            }
        }
    }
    Ok(solution)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_primitives_preserve_periodic_controls_and_exact_poles_in_both_charts() {
        let tolerance = crate::Tolerance::DEFAULT;
        let frame = crate::Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            crate::Vector3::try_new(0., 0., 1.).unwrap(),
            tolerance,
        )
        .unwrap();
        for (source, solid) in [
            (NurbsSurface::try_sphere(frame, 2.).unwrap(), true),
            (
                NurbsSurface::try_cylinder(frame, 2., 0., 4.).unwrap(),
                false,
            ),
            (NurbsSurface::try_cone(frame, 2., 4.).unwrap(), false),
            (NurbsSurface::try_torus(frame, 4., 1.).unwrap(), true),
        ] {
            for swap in [false, true] {
                let source = if swap {
                    source.try_swapped_uv().unwrap()
                } else {
                    source.clone()
                };
                let before = source.clone();
                let original = crate::Brep::try_surface_face(source.clone(), tolerance).unwrap();
                let result = try_rebuild_nurbs_surface(&source, [10, 10], [3, 3]).unwrap();
                assert_eq!(result.is_closed_u().unwrap(), source.is_closed_u().unwrap());
                assert_eq!(result.is_closed_v().unwrap(), source.is_closed_v().unwrap());
                assert_eq!(result.is_periodic_u(), source.is_closed_u().unwrap());
                assert_eq!(result.is_periodic_v(), source.is_closed_v().unwrap());
                let n = result.control_point_count_u();
                let m = result.control_point_count_v();
                if result.is_periodic_u() {
                    for j in 0..m {
                        for i in 0..3 {
                            assert_eq!(
                                result.control_points()[j * n + i],
                                result.control_points()[j * n + 10 + i]
                            );
                        }
                    }
                }
                if result.is_periodic_v() {
                    for j in 0..3 {
                        for i in 0..n {
                            assert_eq!(
                                result.control_points()[j * n + i],
                                result.control_points()[(10 + j) * n + i]
                            );
                        }
                    }
                }
                let brep = crate::Brep::try_surface_face(result.clone(), tolerance).unwrap();
                assert_eq!(brep.is_solid(), solid);
                let retrim = original
                    .try_retrimmed_single_surface(result, tolerance)
                    .unwrap();
                assert_eq!(retrim.is_solid(), solid);
                assert_eq!(retrim.edges().len(), original.edges().len());
                assert_eq!(retrim.vertices().len(), original.vertices().len());
                for trim in retrim.faces()[0]
                    .loops()
                    .iter()
                    .flat_map(|l| l.trims())
                    .filter(|t| t.edge().is_none())
                {
                    let surface = retrim.faces()[0].surface();
                    let a = trim.curve().start_point().unwrap();
                    let b = trim.curve().end_point().unwrap();
                    let iso = if a.y() == b.y() {
                        surface.isocurve_u(a.y()).unwrap()
                    } else {
                        surface.isocurve_v(a.x()).unwrap()
                    };
                    let point = iso.control_points()[0].point();
                    assert!(iso.control_points().iter().all(|p| p.point() == point));
                }
                assert_eq!(source, before);
            }
        }
    }
    #[test]
    fn closed_rebuild_matches_native_sdk_control_nets() {
        for text in [
            include_str!("../../../tools/rhino_oracle/observations/surface_rebuild_closed.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/surface_rebuild_closed_degrees.json"
            ),
        ] {
            let q: serde_json::Value = serde_json::from_str(text).unwrap();
            for row in q["results"].as_array().unwrap() {
                let v = &row["value"];
                let source = crate::surface_tween::tests::surface(&v["before"][0]["definition"]);
                let before = source.clone();
                let count = serde_json::from_value(v["spec"]["count"].clone()).unwrap();
                let degree = serde_json::from_value(v["spec"]["degree"].clone()).unwrap();
                let local = try_rebuild_nurbs_surface(&source, count, degree).unwrap();
                let native = crate::surface_tween::tests::surface(&v["rebuild_sdk"]);
                assert_eq!(local.knots_u(), native.knots_u());
                assert_eq!(local.knots_v(), native.knots_v());
                assert_eq!(local.control_points().len(), native.control_points().len());
                for (a, b) in local.control_points().iter().zip(native.control_points()) {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-6,
                        "{} {:?} {:?}",
                        v["case"],
                        a,
                        b
                    );
                }
                assert_eq!(source, before);
            }
        }
    }
    #[test]
    fn standalone_rebuild_replays_native_commands_and_public_sdk_controls() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/surface_rebuild.json"
        ))
        .unwrap();
        assert_eq!(q["results"].as_array().unwrap().len(), 12);
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            let source = crate::surface_tween::tests::surface(&v["before"][0]["definition"]);
            let before = source.clone();
            let count = serde_json::from_value(v["spec"]["count"].clone()).unwrap();
            let degree = serde_json::from_value(v["spec"]["degree"].clone()).unwrap();
            let local = try_rebuild_nurbs_surface(&source, count, degree).unwrap();
            for definition in [
                &v["rebuild_sdk"],
                &v["command"]["after_script"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["definition"],
            ] {
                let native = crate::surface_tween::tests::surface(definition);
                assert_eq!(
                    (local.degree_u(), local.degree_v()),
                    (native.degree_u(), native.degree_v())
                );
                assert_eq!(local.knots_u(), native.knots_u());
                assert_eq!(local.knots_v(), native.knots_v());
                assert_eq!(local.control_points().len(), native.control_points().len());
                for (a, b) in local.control_points().iter().zip(native.control_points()) {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-6,
                        "{}",
                        v["case"]
                    );
                    assert_eq!(a.weight(), b.weight());
                }
            }
            assert_eq!(source, before);
        }
    }
    #[test]
    fn invalid_rebuild_requests_fail_before_preparation() {
        let s = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 0.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap();
        for (count, degree) in [
            ([0, 4], [3, 3]),
            ([4, 4], [0, 3]),
            ([4, 4], [4, 3]),
            ([257, 4], [3, 3]),
        ] {
            assert!(matches!(
                try_rebuild_nurbs_surface(&s, count, degree),
                Err(GeometryError::InvalidSurfaceRebuild { .. })
            ));
        }
    }
}
