//! Forward equal-chord stations from exact dyadic Bernstein sphere equations.
use super::*;
use crate::exact_scalar::{Rational, rational};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};
mod first;
mod roots;
use roots::first_root;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveDivisionPoint {
    pub parameter: Real,
    pub point: Point3,
}

impl CurveRef<'_> {
    /// Finds the first forward sphere intersection after each accepted station.
    /// Includes the start and any exact end station; no remainder is added.
    pub fn divide_by_chord_length(
        self,
        distance: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<CurveDivisionPoint>, GeometryError> {
        if !distance.is_finite() || distance <= 0. {
            return Err(GeometryError::InvalidCurveDivisionLength);
        }
        if matches!(self, CurveRef::Line(_)) {
            return self
                .divide_by_length_samples(distance, true, tolerance)
                .map(|samples| {
                    samples
                        .into_iter()
                        .map(|sample| CurveDivisionPoint {
                            parameter: sample.parameter(),
                            point: sample.point(),
                        })
                        .collect()
                });
        }
        let circular = match self {
            CurveRef::Circle(c) => Some((c.radius(), std::f64::consts::TAU)),
            CurveRef::Arc(c) => Some((c.radius(), c.sweep_radians())),
            _ => None,
        };
        if let Some((radius, sweep)) = circular {
            let mut points = vec![CurveDivisionPoint {
                parameter: *self.domain().start(),
                point: self.start_point()?,
            }];
            let ratio = (distance / radius) * 0.5;
            if ratio > 1. {
                return Ok(points);
            }
            let angle = 2. * ratio.asin();
            let count = (sweep / angle).floor();
            if !count.is_finite() || count >= MAX_CURVE_DIVISION_POINTS as Real {
                return Err(GeometryError::TooManyCurveDivisionPoints {
                    maximum: MAX_CURVE_DIVISION_POINTS,
                });
            }
            for index in 1..=count as usize {
                let a = angle * index as Real;
                let parameter = if (a - sweep).abs() <= 32. * Real::EPSILON * sweep {
                    *self.domain().end()
                } else {
                    self.parameter_at(a / sweep)?
                };
                let point = if parameter == *self.domain().end() {
                    self.end_point()?
                } else {
                    self.evaluate(parameter)?
                };
                points.push(CurveDivisionPoint { parameter, point });
            }
            return Ok(points);
        }
        let nurbs = self.to_nurbs()?;
        let spans = validated_spans(&nurbs)?;
        let mut parameter = *nurbs.domain().start();
        let mut point = self.start_point()?;
        let mut stations = vec![CurveDivisionPoint {
            parameter: *self.domain().start(),
            point,
        }];
        loop {
            let mut next = None;
            for span in &spans {
                let domain = span.domain();
                if *domain.end() <= parameter {
                    continue;
                }
                let coefficients = sphere_coefficients(span, point, distance)?;
                if let Some(t) = first_root(
                    &coefficients,
                    [*domain.start(), *domain.end()],
                    Some(parameter),
                )? {
                    next = Some(t);
                    break;
                }
            }
            let Some(t) = next else { break };
            if stations.len() >= MAX_CURVE_DIVISION_POINTS {
                return Err(GeometryError::TooManyCurveDivisionPoints {
                    maximum: MAX_CURVE_DIVISION_POINTS,
                });
            }
            point = nurbs.evaluate(t)?;
            let native = native_parameter(self, &nurbs, t, point, tolerance)?;
            stations.push(CurveDivisionPoint {
                parameter: native,
                point,
            });
            parameter = t;
            if t == *nurbs.domain().end() {
                break;
            }
        }
        Ok(stations)
    }
}

fn validated_spans(nurbs: &NurbsCurve) -> Result<Vec<NurbsCurve>, GeometryError> {
    let spans = nurbs.try_bezier_spans()?;
    for span in &spans {
        let weights = span
            .control_points()
            .iter()
            .map(|p| rational(p.weight()))
            .collect::<Vec<_>>();
        if first_root(
            &weights,
            [*span.domain().start(), *span.domain().end()],
            None,
        )?
        .is_some()
        {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
    }
    Ok(spans)
}

fn native_parameter(
    source: CurveRef<'_>,
    nurbs: &NurbsCurve,
    t: Real,
    point: Point3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    if t == *nurbs.domain().end() {
        return Ok(*source.domain().end());
    }
    match source {
        CurveRef::Circle(_) | CurveRef::Arc(_) => source.closest_parameter(point, tolerance),
        CurveRef::PolyCurve(c) => {
            let index = c
                .parameters()
                .partition_point(|v| *v <= t)
                .saturating_sub(1)
                .min(c.segments().len() - 1);
            let segment = c.segments()[index].as_ref();
            if matches!(segment, CurveRef::Arc(_)) {
                let local = segment.closest_parameter(point, tolerance)?;
                crate::parameter::map_parameter(
                    local,
                    segment.domain(),
                    c.parameters()[index]..=c.parameters()[index + 1],
                )
            } else {
                Ok(t)
            }
        }
        _ => Ok(t),
    }
}

fn binomial(n: usize) -> Vec<BigInt> {
    let mut values = vec![BigInt::from(1)];
    for i in 0..n {
        values.push(&values[i] * BigInt::from(n - i) / BigInt::from(i + 1));
    }
    values
}

fn sphere_coefficients(
    span: &NurbsCurve,
    center: Point3,
    radius: Real,
) -> Result<Vec<Rational>, GeometryError> {
    let n = span.degree();
    if n.checked_mul(n).is_none_or(|work| work > 33_554_432) {
        return Err(GeometryError::BezierDecompositionLimit);
    }
    let b = binomial(n);
    let product = binomial(2 * n);
    let r2 = rational(radius).pow(2);
    let center = center.to_array().map(rational);
    let controls = span
        .control_points()
        .iter()
        .map(|p| {
            let w = rational(p.weight());
            (
                std::array::from_fn::<_, 3, _>(|i| {
                    (rational(p.point().to_array()[i]) - &center[i]) * &w
                }),
                w,
            )
        })
        .collect::<Vec<_>>();
    let mut coefficients = vec![Rational::zero(); 2 * n + 1];
    for i in 0..=n {
        for j in 0..=n {
            let dot = (0..3)
                .map(|axis| &controls[i].0[axis] * &controls[j].0[axis])
                .sum::<Rational>()
                - &r2 * &controls[i].1 * &controls[j].1;
            coefficients[i + j] += dot * Rational::new(&b[i] * &b[j], product[i + j].clone());
        }
    }
    Ok(coefficients)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsCurve, Polyline3};
    fn p(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.).unwrap()
    }
    #[test]
    fn forward_chords_cross_polyline_corners_without_using_arc_length() {
        let curve = Polyline3::try_new(
            vec![p(0., 0.), p(3., 0.), p(3., 4.), p(8., 4.)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let samples = CurveRef::Polyline(&curve)
            .divide_by_chord_length(2.5, Tolerance::DEFAULT)
            .unwrap();
        assert!((samples[1].point.x() - 2.5).abs() < 1e-12);
        assert!((samples[2].point.y() - 6_f64.sqrt()).abs() < 1e-12);
        for pair in samples.windows(2) {
            assert!(pair[1].parameter > pair[0].parameter);
            assert!((pair[0].point.distance_to(pair[1].point).unwrap() - 2.5).abs() < 1e-12);
        }
    }
    #[test]
    fn even_multiplicity_sphere_contact_is_not_lost_at_a_turnaround() {
        let curve = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(2., 0.), p(0., 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let samples = CurveRef::NurbsCurve(&curve)
            .divide_by_chord_length(1., Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[1].parameter, 0.5);
        assert_eq!(samples[1].point, p(1., 0.));
        assert_eq!(samples[2].parameter, 1.);
    }
    #[test]
    fn invalid_chord_distances_are_rejected_before_sampling() {
        let curve = LineSegment::try_new(p(0., 0.), p(3., 0.), Tolerance::DEFAULT).unwrap();
        for value in [0., -1., Real::INFINITY, Real::NAN] {
            assert!(
                CurveRef::Line(&curve)
                    .divide_by_chord_length(value, Tolerance::DEFAULT)
                    .is_err()
            );
        }
    }

    #[test]
    fn rational_line_chords_resolve_stations_before_sixty_four_subdivisions() {
        let curve = NurbsCurve::try_new_rational(
            1,
            vec![
                crate::WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
                crate::WeightedPoint3::try_new(p(2., 0.), 2_f64.powi(100)).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let stations = CurveRef::NurbsCurve(&curve)
            .divide_by_chord_length(1., Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(stations.len(), 3);
        assert!(stations[1].parameter < 2_f64.powi(-64));
        assert!((stations[1].point.x() - 1.).abs() < 1e-12);
        assert_eq!(stations[2].point, p(2., 0.));
    }

    #[test]
    fn non_dyadic_turnaround_contact_does_not_require_an_arc_length_integral() {
        let curve = NurbsCurve::try_new(
            2,
            vec![p(0., 0.), p(2.5, 0.), p(-1.25, 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let stations = CurveRef::NurbsCurve(&curve)
            .divide_by_chord_length(1., Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(stations.len(), 4);
        for (station, expected) in stations.iter().zip([0., 1., 0., -1.]) {
            assert!((station.point.x() - expected).abs() < 1e-12);
        }
        for pair in stations.windows(2) {
            assert!(pair[1].parameter > pair[0].parameter);
        }
    }

    #[test]
    fn denominator_poles_are_rejected_even_when_no_chord_is_reachable() {
        for weights in [[1., -1., 1.], [1., -2., 1.]] {
            let curve = NurbsCurve::try_new_rational(
                2,
                weights
                    .into_iter()
                    .enumerate()
                    .map(|(i, w)| crate::WeightedPoint3::try_new(p(i as Real, 0.), w).unwrap())
                    .collect(),
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap();
            assert_eq!(
                CurveRef::NurbsCurve(&curve).divide_by_chord_length(1000., Tolerance::DEFAULT),
                Err(GeometryError::ZeroWeightAtParameter)
            );
        }
    }
}
