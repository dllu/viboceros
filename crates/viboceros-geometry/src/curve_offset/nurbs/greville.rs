//! Adaptive original-basis normal loci for curvature-partitioned Inset caps.
use super::*;
use crate::{spline_collocation::Axis, spline_collocation::general_banded};
use faer::Mat;

const MAX_CONTROLS: usize = 1000;

pub(crate) fn fit_normal_locus(
    source: &NurbsCurve,
    distance: Real,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    if source.degree() > 17 || source.control_points().len() > MAX_CONTROLS {
        return Err(GeometryError::NurbsOffsetFitLimit);
    }
    let source = source.clamped_to_active_domain()?;
    let mut knots = source.knots().to_vec();
    for _ in 0..32 {
        let candidate = interpolate(&source, knots.clone(), distance, normal)?;
        let mut inserted = Vec::new();
        for (a, b) in candidate.spans() {
            let mut largest = 0_f64;
            for i in 0..=4 {
                let t = match i {
                    0 => a,
                    4 => b,
                    _ => a.midpoint(b) + (i as Real - 2.) * (b - a) * 0.25,
                };
                let side = if i == 4 {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                };
                let expected =
                    offset_sample_with_reversals(&source, distance, normal, t, side, true)?;
                let tangent = source.tangent_at_on_side(t, side)?;
                let left = normal.as_vector().cross(tangent.as_vector())?;
                let delta = expected.point.vector_to(candidate.evaluate(t)?)?;
                largest = largest.max(delta.dot(left)?.abs());
            }
            if largest > tolerance.absolute() {
                let t = a.midpoint(b);
                if t <= a || t >= b {
                    return Err(GeometryError::NurbsOffsetFitLimit);
                }
                inserted.push(t);
            }
        }
        if inserted.is_empty() {
            return Ok(candidate);
        }
        if candidate
            .control_points()
            .len()
            .saturating_add(inserted.len())
            > MAX_CONTROLS
        {
            return Err(GeometryError::NurbsOffsetFitLimit);
        }
        // Merge sorted knot streams once. Homogeneous collocation below
        // refines the denominator with no sequence of whole-curve insertions.
        let previous = std::mem::take(&mut knots);
        knots.reserve(previous.len() + inserted.len());
        let mut cursor = 0;
        for k in previous {
            while cursor < inserted.len() && inserted[cursor] < k {
                knots.push(inserted[cursor]);
                cursor += 1;
            }
            knots.push(k);
        }
        knots.extend_from_slice(&inserted[cursor..]);
    }
    Err(GeometryError::NurbsOffsetFitLimit)
}

fn interpolate(
    source: &NurbsCurve,
    knots: Vec<Real>,
    distance: Real,
    normal: UnitVector3,
) -> Result<NurbsCurve, GeometryError> {
    let p = source.degree();
    let axis = Axis::sites(p, knots)?;
    let count = axis.stations.len();
    let origin = source.evaluate(*source.domain().start())?;
    let scale = source
        .control_points()
        .iter()
        .map(|c| c.weight().abs())
        .fold(0., Real::max);
    let mut rows = Vec::with_capacity(count);
    let mut rhs = Mat::zeros(count, 4);
    for (i, station) in axis.stations.iter().enumerate() {
        rows.push(if station.fixed {
            (i, vec![1.])
        } else {
            crate::nurbs::bspline_basis_nonzero(&axis.knots, p, count, station.parameter)?
        });
        let weight = source.evaluate_scaled_homogeneous(station.parameter, scale)?[3];
        let sample = offset_sample_with_reversals(
            source,
            distance,
            normal,
            station.parameter,
            station.side,
            true,
        )?;
        let point = origin.vector_to(sample.point)?.to_array();
        for k in 0..3 {
            rhs[(i, k)] = point[k] * weight;
        }
        rhs[(i, 3)] = weight;
    }
    let result = general_banded::solve(p, &rows, rhs)?;
    let controls = (0..count)
        .map(|i| {
            let w = result[(i, 3)];
            let point = origin.translated(Vector3::try_from(std::array::from_fn(|k| {
                result[(i, k)] / w
            }))?)?;
            WeightedPoint3::try_new(point, w * scale)
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(p, controls, axis.knots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ellipse3;

    fn ellipse(center: Point3, rx: Real, ry: Real) -> Ellipse3 {
        let axis = |x, y, z| {
            Vector3::try_new(x, y, z)
                .unwrap()
                .normalized_nonzero()
                .unwrap()
        };
        Ellipse3::try_new(
            center,
            rx,
            ry,
            axis(1., 0., 0.),
            axis(0., 1., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn circle_loci_retain_degree_and_accurate_radius_after_translation() {
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
        for center in [
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(100_000., -200_000., 300_000.).unwrap(),
        ] {
            let circle = ellipse(center, 5., 5.);
            for degree in [2, 3, 7, 17] {
                let source = circle
                    .to_nurbs()
                    .unwrap()
                    .try_change_degree(degree, false)
                    .unwrap();
                for distance in [-1.5, 1.5] {
                    let offset =
                        fit_normal_locus(&source, distance, circle.normal().unwrap(), tolerance)
                            .unwrap();
                    assert_eq!(offset.degree(), degree);
                    assert_eq!(offset.control_points().len(), source.control_points().len());
                    let domain = offset.domain();
                    for i in 0..=256 {
                        let t = domain.start() + (domain.end() - domain.start()) * i as Real / 256.;
                        let radius = center.distance_to(offset.evaluate(t).unwrap()).unwrap();
                        assert!(
                            (radius - (5. - distance)).abs() < tolerance.absolute() * 0.1,
                            "degree {degree}: {radius}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn refined_ellipse_locus_has_constant_analytic_nearest_distance() {
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
        let ellipse = ellipse(Point3::try_new(0., 0., 0.).unwrap(), 6., 2.);
        for degree in [2, 3, 7] {
            let source = ellipse
                .to_nurbs()
                .unwrap()
                .try_change_degree(degree, false)
                .unwrap();
            let offset =
                fit_normal_locus(&source, -2., ellipse.normal().unwrap(), tolerance).unwrap();
            assert_eq!(offset.degree(), degree);
            assert!(offset.control_points().len() > source.control_points().len());
            let domain = offset.domain();
            for i in 0..1024 {
                let t =
                    domain.start() + (domain.end() - domain.start()) * (i as Real + 0.37) / 1024.;
                let point = offset.evaluate(t).unwrap();
                let closest = ellipse
                    .evaluate(ellipse.closest_parameter(point).unwrap())
                    .unwrap();
                let distance = point.distance_to(closest).unwrap();
                assert!(
                    (distance - 2.).abs() <= 2. * tolerance.absolute(),
                    "degree {degree}: {distance}"
                );
            }
        }
    }

    #[test]
    fn oversized_source_basis_declines_before_interpolation() {
        let circle = ellipse(Point3::try_new(0., 0., 0.).unwrap(), 5., 5.);
        let high_degree = circle
            .to_nurbs()
            .unwrap()
            .try_change_degree(18, false)
            .unwrap();
        let controls = (0..1001)
            .map(|i| Point3::try_new(i as Real, 0., 0.).unwrap())
            .collect();
        let knots = std::iter::once(0.)
            .chain((0..=1000).map(|i| i as Real))
            .chain(std::iter::once(1000.))
            .collect();
        let many_controls = NurbsCurve::try_new(1, controls, knots).unwrap();
        for source in [high_degree, many_controls] {
            assert!(matches!(
                fit_normal_locus(&source, 1., circle.normal().unwrap(), Tolerance::DEFAULT),
                Err(GeometryError::NurbsOffsetFitLimit)
            ));
        }
    }
}
