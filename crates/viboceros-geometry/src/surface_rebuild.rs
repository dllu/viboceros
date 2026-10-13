//! Uniform surface rebuilding at midpoint-isocurve arc-length stations.
use crate::{CurveRef, Tolerance, curve::ArcLengthSampler, nurbs::stable_knot_mean};
use crate::{GeometryError, NurbsCurve, NurbsSurface, Point3, Real, SurfaceKnotDirection};
use faer::{Mat, prelude::*};

/// Collocation shared by curve and tensor construction/editing backends.
/// Targets and returned controls are row-major homogeneous channels. Periodic
/// parameters include the final domain endpoint without a repeated target row.
pub struct NurbsInterpolationSystem<'a> {
    pub degree: usize,
    pub knots: &'a [Real],
    pub parameters: &'a [Real],
    pub periodic: bool,
    pub columns: usize,
    pub targets: &'a [Real],
}

/// Compatibility name for uniform Rebuild jobs. Open jobs can also contain
/// explicitly supplied nonuniform knots and interpolation parameters.
pub type UniformInterpolationSystem<'a> = NurbsInterpolationSystem<'a>;

/// Rebuild a surface as a non-rational uniform tensor spline. Closed directions
/// use periodic interpolation with `degree` repeated controls beyond the count.
/// Counts are bounded at 256 per axis and must exceed their degrees. This interpolates
/// arc-length-spaced tensor stations, not the complete original surface locus.
pub fn try_rebuild_nurbs_surface(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
) -> Result<NurbsSurface, GeometryError> {
    try_rebuild_nurbs_surface_with_operations(
        source,
        count,
        degree,
        &mut portable_isocurve,
        &mut portable_solve,
    )
}

/// Keep tensor sampling, basis, topology and pole policy while delegating
/// isocurve production and uniform interpolation to external backends.
pub fn try_rebuild_nurbs_surface_with_operations<E: From<GeometryError>>(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
    isocurve: &mut impl FnMut(&NurbsSurface, SurfaceKnotDirection, Real) -> Result<NurbsCurve, E>,
    solve: &mut impl FnMut(&UniformInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
) -> Result<NurbsSurface, E> {
    rebuild(
        source,
        count,
        degree,
        [source.is_closed_u()?, source.is_closed_v()?],
        isocurve,
        solve,
    )
}

pub(crate) fn rebuild_open(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
) -> Result<NurbsSurface, GeometryError> {
    try_rebuild_open_nurbs_surface_with_operations(
        source,
        count,
        degree,
        &mut portable_isocurve,
        &mut portable_solve,
    )
}

/// Force open tensor preparation, including geometrically closed sources.
/// TweenSurfaces uses this distinct policy for unequal control nets.
pub fn try_rebuild_open_nurbs_surface_with_operations<E: From<GeometryError>>(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
    isocurve: &mut impl FnMut(&NurbsSurface, SurfaceKnotDirection, Real) -> Result<NurbsCurve, E>,
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
) -> Result<NurbsSurface, E> {
    rebuild(source, count, degree, [false; 2], isocurve, solve)
}

fn rebuild<E: From<GeometryError>>(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
    closed: [bool; 2],
    isocurve: &mut impl FnMut(&NurbsSurface, SurfaceKnotDirection, Real) -> Result<NurbsCurve, E>,
    solve: &mut impl FnMut(&UniformInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
) -> Result<NurbsSurface, E> {
    if (0..2).any(|axis| degree[axis] == 0 || count[axis] <= degree[axis] || count[axis] > 256) {
        return Err(GeometryError::InvalidSurfaceRebuild {
            context: "requires positive degrees and degree+1..=256 controls per axis",
        }
        .into());
    }
    if source.control_points().iter().any(|c| c.weight() <= 0.) {
        return Err(GeometryError::InvalidSurfaceRebuild {
            context: "requires positive weights",
        }
        .into());
    }
    let mut axis = |axis: usize| -> Result<_, E> {
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
        let direction = if axis == 0 {
            SurfaceKnotDirection::U
        } else {
            SurfaceKnotDirection::V
        };
        let c = isocurve(source, direction, domain.start().midpoint(*domain.end()))?;
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
        let mut parameters = stations;
        if closed[axis] {
            parameters.push(spans as Real);
        }
        Ok((knots, source_parameters, parameters))
    };
    let (ku, u, tu) = axis(0)?;
    let (kv, v, tv) = axis(1)?;
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
    let local = points
        .iter()
        .map(|p| std::array::from_fn(|k| (p.to_array()[k] - origin[k]) / scale))
        .collect::<Vec<_>>();
    let solved =
        interpolate_tensor_local(&local, count, degree, [&ku, &kv], [&tu, &tv], closed, solve)?;
    let output_count = std::array::from_fn::<_, 2, _>(|axis| {
        count[axis] + if closed[axis] { degree[axis] } else { 0 }
    });
    let mut controls = Vec::with_capacity(output_count[0] * output_count[1]);
    for j in 0..output_count[1] {
        for i in 0..output_count[0] {
            controls.push(Point3::try_from(std::array::from_fn(|k| {
                solved[(j % count[1]) * count[0] + (i % count[0])][k].mul_add(scale, origin[k])
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
        let curve = isocurve(source, SurfaceKnotDirection::U, side)?;
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
        let curve = isocurve(source, SurfaceKnotDirection::V, side)?;
        let point = curve.control_points()[0].point();
        if !closed[0] && curve.control_points().iter().all(|p| p.point() == point) {
            let target_i = if i == 0 { 0 } else { output_count[0] - 1 };
            for j in 0..output_count[1] {
                controls[j * output_count[0] + target_i] = point;
            }
        }
    }
    Ok(NurbsSurface::try_new(
        degree[0],
        degree[1],
        output_count[0],
        output_count[1],
        controls,
        ku,
        kv,
    )?)
}

/// Solve both tensor directions using one homogeneous batch per axis. Input
/// and output use U-fast normalized Cartesian coordinates; weights stay one.
pub(crate) fn interpolate_tensor_local<E: From<GeometryError>>(
    points: &[[Real; 3]],
    count: [usize; 2],
    degree: [usize; 2],
    knots: [&[Real]; 2],
    parameters: [&[Real]; 2],
    periodic: [bool; 2],
    solve: &mut (impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E> + ?Sized),
) -> Result<Vec<[Real; 3]>, E> {
    let columns_u = count[1] * 4;
    let columns_v = count[0] * 4;
    let rhs = (0..count[0] * columns_u)
        .map(|index| {
            let i = index / columns_u;
            let j = index % columns_u;
            if j % 4 == 3 {
                1.
            } else {
                points[j / 4 * count[0] + i][j % 4]
            }
        })
        .collect::<Vec<_>>();
    let first = checked_solve(
        &NurbsInterpolationSystem {
            degree: degree[0],
            knots: knots[0],
            parameters: parameters[0],
            periodic: periodic[0],
            columns: columns_u,
            targets: &rhs,
        },
        solve,
    )?;
    let rhs = (0..count[1] * columns_v)
        .map(|index| {
            let i = index / columns_v;
            let j = index % columns_v;
            if j % 4 == 3 {
                1.
            } else {
                first[j / 4 * columns_u + i * 4 + j % 4]
            }
        })
        .collect::<Vec<_>>();
    let second = checked_solve(
        &NurbsInterpolationSystem {
            degree: degree[1],
            knots: knots[1],
            parameters: parameters[1],
            periodic: periodic[1],
            columns: columns_v,
            targets: &rhs,
        },
        solve,
    )?;
    Ok(second.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect())
}

fn portable_isocurve(
    source: &NurbsSurface,
    direction: SurfaceKnotDirection,
    parameter: Real,
) -> Result<NurbsCurve, GeometryError> {
    match direction {
        SurfaceKnotDirection::U => source.isocurve_u(parameter),
        SurfaceKnotDirection::V => source.isocurve_v(parameter),
        SurfaceKnotDirection::Both => Err(GeometryError::InvalidSurfaceRebuild {
            context: "isocurve preparation requires one direction",
        }),
    }
}

pub(crate) fn checked_solve<E: From<GeometryError>>(
    input: &UniformInterpolationSystem<'_>,
    solve: &mut (impl FnMut(&UniformInterpolationSystem<'_>) -> Result<Vec<Real>, E> + ?Sized),
) -> Result<Vec<Real>, E> {
    let mut solution = solve(input)?;
    if solution.len() != input.targets.len() {
        return Err(GeometryError::InvalidControlNet {
            context: "tensor interpolation solver returned an inconsistent layout",
        }
        .into());
    }
    crate::require_finite(solution.iter().copied(), "surface rebuild solution")?;
    // Partition of unity makes a constant target channel exactly constant.
    // Preserve it before the other tensor pass can amplify first-pass noise.
    for column in 0..input.columns {
        let value = input.targets[column];
        if input
            .targets
            .chunks_exact(input.columns)
            .all(|row| row[column] == value)
        {
            for row in solution.chunks_exact_mut(input.columns) {
                row[column] = value;
            }
        }
    }
    Ok(solution)
}

fn portable_solve(input: &UniformInterpolationSystem<'_>) -> Result<Vec<Real>, GeometryError> {
    let n = input.parameters.len() - usize::from(input.periodic);
    let controls = n + if input.periodic { input.degree } else { 0 };
    let rows = input.parameters[..n]
        .iter()
        .map(|&t| {
            let basis = crate::nurbs::bspline_basis_values_extended(
                input.knots,
                input.degree,
                controls,
                t,
            )?;
            let mut row = vec![0.; n];
            for (i, b) in basis.into_iter().enumerate() {
                row[i % n] += b;
            }
            Ok(row)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let matrix = Mat::from_fn(n, n, |i, j| rows[i][j]);
    let targets = Mat::from_fn(n, input.columns, |i, j| {
        input.targets[i * input.columns + j]
    });
    let solution = solve(&matrix, &targets)?;
    Ok((0..n * input.columns)
        .map(|i| solution[(i / input.columns, i % input.columns)])
        .collect())
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
    fn external_tensor_operations_share_layout_and_reject_bad_solver_outputs() {
        let source = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 1.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap();
        let before = source.clone();
        let mut iso_calls = 0;
        let mut solve_calls = 0;
        let result = try_rebuild_nurbs_surface_with_operations(
            &source,
            [5, 4],
            [3, 2],
            &mut |s, d, t| {
                iso_calls += 1;
                portable_isocurve(s, d, t)
            },
            &mut |input| {
                solve_calls += 1;
                assert_eq!(input.columns, if solve_calls == 1 { 16 } else { 20 });
                assert!(!input.periodic);
                assert!(input.targets.chunks_exact(4).all(|p| p[3] == 1.));
                portable_solve(input)
            },
        )
        .unwrap();
        assert_eq!(iso_calls, 6);
        assert_eq!(solve_calls, 2);
        assert_eq!(
            result,
            try_rebuild_nurbs_surface(&source, [5, 4], [3, 2]).unwrap()
        );
        assert_eq!(source, before);
        let bad = try_rebuild_nurbs_surface_with_operations(
            &source,
            [5, 4],
            [3, 2],
            &mut portable_isocurve,
            &mut |_| -> Result<Vec<Real>, GeometryError> { Ok(Vec::new()) },
        );
        assert!(matches!(bad, Err(GeometryError::InvalidControlNet { .. })));
        let invalid = try_rebuild_nurbs_surface_with_operations(
            &source,
            [3, 4],
            [3, 2],
            &mut |_, _, _| -> Result<NurbsCurve, GeometryError> {
                panic!("invalid request entered extraction")
            },
            &mut |_| -> Result<Vec<Real>, GeometryError> {
                panic!("invalid request entered solver")
            },
        );
        assert!(matches!(
            invalid,
            Err(GeometryError::InvalidSurfaceRebuild { .. })
        ));
    }
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
