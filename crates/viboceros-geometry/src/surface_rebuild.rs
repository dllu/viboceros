//! Uniform surface rebuilding at midpoint-isocurve arc-length stations.
use crate::{
    CurveRef, Tolerance,
    curve::ArcLengthSampler,
    nurbs::{bspline_basis_values, stable_knot_mean},
};
use crate::{GeometryError, NurbsSurface, Point3, Real};
use faer::{Mat, prelude::*};

/// Rebuild an open surface as a non-rational uniform tensor spline.
/// Counts are bounded at 256 per axis and must exceed their degrees. This interpolates
/// arc-length-spaced tensor stations, not the complete original surface locus.
pub fn try_rebuild_nurbs_surface(
    source: &NurbsSurface,
    count: [usize; 2],
    degree: [usize; 2],
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
        for (i, &t) in stations.iter().enumerate().take(n - 1).skip(1) {
            source_parameters[i] =
                sampler.parameter_at_distance(sampler.total_length() * (t / spans as Real))?;
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

#[cfg(test)]
mod tests {
    use super::*;
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
