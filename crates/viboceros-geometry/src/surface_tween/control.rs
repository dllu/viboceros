//! Shared arc-length station preparation for uniform control matching.
use super::*;
use crate::{
    CurveRef, Tolerance,
    curve::ArcLengthSampler,
    nurbs::{bspline_basis_values, stable_knot_mean},
};
use faer::{Mat, prelude::*};

fn rebuilt(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
) -> Result<NurbsSurface, GeometryError> {
    check_count(
        count[0]
            .checked_mul(count[1])
            .ok_or_else(|| error("control preparation count overflow"))?,
        1,
    )?;
    if count.iter().any(|&n| n > 256) {
        return Err(error("control matching exceeds the tensor solve limit"));
    }
    let axis = |axis: usize| -> Result<_, GeometryError> {
        let n = count[axis];
        let p = degree[axis];
        let spans = n - p;
        let knots = (0..n + p + 1)
            .map(|i| i.saturating_sub(p).min(spans) as Real)
            .collect::<Vec<_>>();
        let stations = (0..n)
            .map(|i| stable_knot_mean(&knots[i + 1..=i + p]))
            .collect::<Result<Vec<_>, _>>()?;
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
            source_parameters[i] =
                sampler.parameter_at_distance(sampler.total_length() * t / spans as Real)?;
        }
        let active = if axis == 0 {
            source.domain_u()
        } else {
            source.domain_v()
        };
        source_parameters[0] = *active.start();
        source_parameters[n - 1] = *active.end();
        let rows = stations
            .iter()
            .map(|&t| bspline_basis_values(&knots, p, n, t))
            .collect::<Result<Vec<_>, _>>()?;
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
    let mut controls = Vec::with_capacity(points.len());
    for j in 0..count[1] {
        for i in 0..count[0] {
            controls.push(Point3::try_from(std::array::from_fn(|k| {
                solved[(j, i * 3 + k)].mul_add(scale, origin[k])
            }))?);
        }
    }
    // Clamped interpolation fixes the four corners exactly.
    for index in [0, count[0] - 1, (count[1] - 1) * count[0], points.len() - 1] {
        controls[index] = points[index];
    }
    NurbsSurface::try_new(degree[0], degree[1], count[0], count[1], controls, ku, kv)
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

pub(super) fn tweens(
    start: &NurbsSurface,
    end: &NurbsSurface,
    number: usize,
) -> Result<Vec<NurbsSurface>, GeometryError> {
    let count = [
        start
            .control_point_count_u()
            .max(end.control_point_count_u()),
        start
            .control_point_count_v()
            .max(end.control_point_count_v()),
    ];
    let degree = [
        start.degree_u().max(end.degree_u()),
        start.degree_v().max(end.degree_v()),
    ];
    check_count(
        count[0]
            .checked_mul(count[1])
            .ok_or_else(|| error("control matching count overflow"))?,
        number,
    )?;
    let incompatible = start.degree_u() != end.degree_u()
        || start.degree_v() != end.degree_v()
        || start.control_point_count_u() != end.control_point_count_u()
        || start.control_point_count_v() != end.control_point_count_v();
    let mut a = start.clone();
    let mut b = end.clone();
    let mut outputs = Vec::with_capacity(number);
    for index in 1..=number {
        if incompatible {
            a = rebuilt(&a, count, degree)?;
            b = rebuilt(&b, count, degree)?;
        }
        let t = index as Real / (number + 1) as Real;
        let controls = a
            .control_points()
            .iter()
            .zip(b.control_points())
            .map(|(a, b)| {
                let factor = t * (b.weight() / a.weight()).sqrt();
                let mut xyz = [0.; 3];
                for (k, x) in xyz.iter_mut().enumerate() {
                    *x = crate::exact_scalar::remap_scalar(
                        factor,
                        [0., 1.],
                        [a.point().to_array()[k], b.point().to_array()[k]],
                    )?;
                }
                WeightedPoint3::try_new(Point3::try_from(xyz)?, a.weight())
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        outputs.push(NurbsSurface::try_new_rational(
            degree[0],
            degree[1],
            count[0],
            count[1],
            controls,
            a.knots_u().to_vec(),
            a.knots_v().to_vec(),
        )?);
    }
    Ok(outputs)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unequal_nets_reproduce_analytic_planes_and_reject_excess_work() {
        let plane = |n: usize, z: Real| {
            let controls = (0..n)
                .flat_map(|j| {
                    (0..n).map(move |i| {
                        Point3::try_new(
                            4. * i as Real / (n - 1) as Real,
                            6. * j as Real / (n - 1) as Real,
                            z,
                        )
                        .unwrap()
                    })
                })
                .collect();
            let knots = vec![0., 0., 0., 1., 1., 1.];
            NurbsSurface::try_new(
                n - 1,
                n - 1,
                n,
                n,
                controls,
                if n == 2 {
                    vec![0., 0., 1., 1.]
                } else {
                    knots.clone()
                },
                if n == 2 { vec![0., 0., 1., 1.] } else { knots },
            )
            .unwrap()
        };
        let a = plane(2, 0.);
        let b = plane(3, 4.);
        let outputs = try_tween_nurbs_surfaces(&a, &b, 3).unwrap();
        for (index, s) in outputs.iter().enumerate() {
            for j in 0..=8 {
                for i in 0..=8 {
                    let p = s.evaluate(i as Real / 8., j as Real / 8.).unwrap();
                    let expected =
                        Point3::try_new(i as Real / 2., j as Real * 0.75, (index + 1) as Real)
                            .unwrap();
                    assert!(p.distance_to(expected).unwrap() < 1e-12);
                }
            }
        }
        assert!(try_tween_nurbs_surfaces(&a, &b, MAX_SURFACE_TWEEN_COUNT + 1).is_err());
        assert!(check_count(256 * 256, 16).is_err());
    }
    #[test]
    fn control_matching_replays_twenty_four_native_unequal_and_knot_cases() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/tween_surfaces_control_initial.json"
        ))
        .unwrap();
        assert_eq!(q["results"].as_array().unwrap().len(), 24);
        replay_controls(&q);
    }
    #[test]
    fn control_matching_replays_third_tweens_and_equal_count_degree_changes() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/tween_surfaces_control_followup.json"
        ))
        .unwrap();
        assert_eq!(q["results"].as_array().unwrap().len(), 14);
        replay_controls(&q);
    }
    fn replay_controls(q: &serde_json::Value) {
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            let a = super::super::tests::surface(&v["before"][0]["definition"]);
            let b = super::super::tests::surface(&v["before"][1]["definition"]);
            let number = v["spec"]["number"].as_u64().unwrap() as usize;
            let before = (a.clone(), b.clone());
            let output = tweens(&a, &b, number).unwrap();
            for (local, record) in output.iter().zip(
                v["command"]["after_script"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .skip(2),
            ) {
                let native = super::super::tests::surface(&record["definition"]);
                assert_eq!(
                    (
                        local.degree_u(),
                        local.degree_v(),
                        local.control_point_count_u(),
                        local.control_point_count_v()
                    ),
                    (
                        native.degree_u(),
                        native.degree_v(),
                        native.control_point_count_u(),
                        native.control_point_count_v()
                    )
                );
                for (a, b) in local
                    .knots_u()
                    .iter()
                    .chain(local.knots_v())
                    .zip(native.knots_u().iter().chain(native.knots_v()))
                {
                    assert!((a - b).abs() < 1e-12, "{}", v["case"]);
                }
                for (a, b) in local.control_points().iter().zip(native.control_points()) {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-6,
                        "{} {:?} {:?}",
                        v["case"],
                        a,
                        b
                    );
                    assert!((a.weight() - b.weight()).abs() < 1e-12);
                }
            }
            assert_eq!((a, b), before);
        }
    }
    #[test]
    fn control_preparation_matches_public_surface_rebuild_nets() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/rhino_oracle/observations/tween_surfaces_control_initial.json"
        ))
        .unwrap();
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            let a = super::super::tests::surface(&v["before"][0]["definition"]);
            let b = super::super::tests::surface(&v["before"][1]["definition"]);
            let count = [
                a.control_point_count_u().max(b.control_point_count_u()),
                a.control_point_count_v().max(b.control_point_count_v()),
            ];
            let degree = [
                a.degree_u().max(b.degree_u()),
                a.degree_v().max(b.degree_v()),
            ];
            for (s, n) in [&a, &b]
                .into_iter()
                .zip(v["rebuild_sdk"].as_array().unwrap())
            {
                let candidate = rebuilt(s, count, degree).unwrap();
                let native = super::super::tests::surface(n);
                for (a, b) in candidate
                    .control_points()
                    .iter()
                    .zip(native.control_points())
                {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-6,
                        "{}",
                        v["case"]
                    );
                }
            }
        }
    }
}
