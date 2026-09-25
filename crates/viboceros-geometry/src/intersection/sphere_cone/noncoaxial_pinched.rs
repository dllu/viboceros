//! The nodal section where two offset sphere/cone loops touch.
//!
//! At the transverse angle opposite the sphere center, the generator
//! discriminant has a double zero. A signed half-angle factor traverses both
//! axial roots smoothly over two turns and visits the node twice.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS: usize = 4096;
const TURN: Real = std::f64::consts::TAU;
const PERIOD: Real = 2.0 * TURN;
const PI: Real = std::f64::consts::PI;

#[derive(Clone, Copy)]
struct Basis {
    frame: Frame3,
    slope: Real,
    axial_sign: Real,
    direction: [Real; 2],
    center_axial: Real,
    cosine_coefficient: Real,
    quadratic: Real,
    minimum_linear: Real,
    canonical_constant: Real,
}

pub(super) fn intersect(
    sphere_center: Point3,
    sphere_radius: Real,
    (cone_frame, cone_radius, signed_height): (Frame3, Real, Real),
    tolerance: Tolerance,
    coordinate_roundoff: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let [center_x, center_y, center_z] = cone_frame.coordinates_of(sphere_center)?;
    let offset = center_x.hypot(center_y);
    let height = signed_height.abs();
    let center_axial = center_z * signed_height.signum();
    let slope = cone_radius / height;
    let quadratic = slope.mul_add(slope, 1.0);
    let cosine_coefficient = slope * offset;
    let minimum_linear = center_axial - cosine_coefficient;
    let basis = Basis {
        frame: cone_frame,
        slope,
        axial_sign: signed_height.signum(),
        direction: [center_x / offset, center_y / offset],
        center_axial,
        cosine_coefficient,
        quadratic,
        minimum_linear,
        canonical_constant: minimum_linear * minimum_linear / quadratic,
    };
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(height).max(sphere_radius))
        .max(coordinate_roundoff);
    if !fit_tolerance.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "pinched sphere/cone fit tolerance is not finite",
        });
    }
    let derivative_bound = fourth_derivative_bound(basis);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "pinched sphere/cone derivative bound is not finite",
        });
    }
    let cuts = angular_cuts(basis, height, fit_tolerance);
    let intervals = active_intervals(basis, &cuts, height);
    let mut events = Vec::new();
    for angle in cuts {
        if (basis.axial(angle) - height).abs() <= fit_tolerance
            && !angle_in_intervals(angle, &intervals)
        {
            let point = basis.sample(angle)?.0;
            if !events.iter().any(|event| {
                matches!(event, SurfaceSurfaceIntersectionEvent::Point(existing)
                    if existing.distance_to(point).is_ok_and(|distance| distance <= fit_tolerance))
            }) {
                events.push(SurfaceSurfaceIntersectionEvent::Point(point));
            }
        }
    }
    for (start, end) in intervals {
        events.push(SurfaceSurfaceIntersectionEvent::Curve(fit_curve(
            basis,
            start,
            end,
            fit_tolerance,
            derivative_bound,
        )?));
    }
    Ok(events)
}

impl Basis {
    fn linear(self, parameter: Real) -> Real {
        self.cosine_coefficient
            .mul_add(parameter.cos(), self.center_axial)
    }

    fn factor(self, linear: Real) -> Real {
        (2.0 * self.cosine_coefficient * (linear + self.minimum_linear)).sqrt()
    }

    fn axial(self, parameter: Real) -> Real {
        let linear = self.linear(parameter);
        let radical = (0.5 * parameter).cos() * self.factor(linear);
        (linear + radical) / self.quadratic
    }

    fn sample(self, parameter: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = parameter.sin_cos();
        let linear = self.cosine_coefficient.mul_add(cosine, self.center_axial);
        let linear_derivative = -self.cosine_coefficient * sine;
        let factor = self.factor(linear);
        let (half_sine, half_cosine) = (0.5 * parameter).sin_cos();
        let factor_derivative = self.cosine_coefficient * linear_derivative / factor;
        let radical = half_cosine * factor;
        let radical_derivative = -0.5 * half_sine * factor + half_cosine * factor_derivative;
        let axial = (linear + radical) / self.quadratic;
        let axial_derivative = (linear_derivative + radical_derivative) / self.quadratic;
        let radial = self.slope * axial;
        let radial_derivative = self.slope * axial_derivative;
        let [ux, uy] = self.direction;
        let local = [
            radial * cosine.mul_add(ux, -sine * uy),
            radial * cosine.mul_add(uy, sine * ux),
            self.axial_sign * axial,
        ];
        let derivative = [
            radial_derivative * cosine.mul_add(ux, -sine * uy)
                + radial * (-sine * ux - cosine * uy),
            radial_derivative * cosine.mul_add(uy, sine * ux) + radial * (-sine * uy + cosine * ux),
            self.axial_sign * axial_derivative,
        ];
        let point = if [-PI, PI, 3.0 * PI, 5.0 * PI].contains(&parameter) {
            let node_axial = self.minimum_linear / self.quadratic;
            self.frame.point_at([
                -self.slope * node_axial * ux,
                -self.slope * node_axial * uy,
                self.axial_sign * node_axial,
            ])?
        } else {
            self.frame.point_at(local)?
        };
        Ok((point, self.frame.vector_at(derivative)?))
    }
}

fn fourth_derivative_bound(basis: Basis) -> Real {
    let b = basis.cosine_coefficient;
    let lower = 4.0 * b * basis.minimum_linear;
    let upper = 4.0 * b * basis.center_axial;
    let derivative = 2.0 * b * b;
    let root = lower.sqrt();
    let lower3 = lower * root;
    let lower5 = lower * lower3;
    let lower7 = lower * lower5;
    let g0 = upper.sqrt();
    let g1 = derivative / (2.0 * root);
    let g2 = derivative / (2.0 * root) + derivative * derivative / (4.0 * lower3);
    let g3 = derivative / (2.0 * root)
        + 3.0 * derivative * derivative / (4.0 * lower3)
        + 3.0 * derivative.powi(3) / (8.0 * lower5);
    let g4 = derivative / (2.0 * root)
        + 7.0 * derivative * derivative / (4.0 * lower3)
        + 9.0 * derivative.powi(3) / (4.0 * lower5)
        + 15.0 * derivative.powi(4) / (16.0 * lower7);
    let w1 = g1 + 0.5 * g0;
    let w2 = g2 + g1 + 0.25 * g0;
    let w3 = g3 + 1.5 * g2 + 0.75 * g1 + 0.125 * g0;
    let w4 = g4 + 2.0 * g3 + 1.5 * g2 + 0.5 * g1 + g0 / 16.0;
    let t0 = (basis.center_axial + b + g0) / basis.quadratic;
    let t1 = (b + w1) / basis.quadratic;
    let t2 = (b + w2) / basis.quadratic;
    let t3 = (b + w3) / basis.quadratic;
    let t4 = (b + w4) / basis.quadratic;
    (basis.slope * (t0 + 4.0 * t1 + 6.0 * t2 + 4.0 * t3 + t4)).hypot(t4)
}

fn angular_cuts(basis: Basis, height: Real, fit_tolerance: Real) -> Vec<Real> {
    let mut cuts = vec![0.0, PI, TURN, 3.0 * PI, PERIOD];
    if height > fit_tolerance {
        let required_linear =
            (basis.quadratic * height * height + basis.canonical_constant) / (2.0 * height);
        let cosine = (required_linear - basis.center_axial) / basis.cosine_coefficient;
        if (-1.0..=1.0).contains(&cosine) {
            let angle = cosine.acos();
            for candidate in [angle, TURN - angle, TURN + angle, PERIOD - angle] {
                if (basis.axial(candidate) - height).abs() <= fit_tolerance {
                    cuts.push(candidate);
                }
            }
        }
    }
    cuts.sort_by(Real::total_cmp);
    cuts.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);
    cuts
}

fn active_intervals(basis: Basis, cuts: &[Real], height: Real) -> Vec<(Real, Real)> {
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in cuts.windows(2) {
        if basis.axial(0.5 * (pair[0] + pair[1])) <= height {
            if let Some(last) = intervals.last_mut()
                && (last.1 - pair[0]).abs() <= 32.0 * Real::EPSILON
            {
                last.1 = pair[1];
            } else {
                intervals.push((pair[0], pair[1]));
            }
        }
    }
    if intervals.len() > 1
        && intervals[0].0 == 0.0
        && intervals.last().is_some_and(|last| last.1 == PERIOD)
    {
        let first = intervals.remove(0);
        let last = intervals.pop().expect("at least two intervals");
        intervals.insert(0, (last.0 - PERIOD, first.1));
    }
    intervals
}

fn angle_in_intervals(parameter: Real, intervals: &[(Real, Real)]) -> bool {
    [-PERIOD, 0.0, PERIOD].into_iter().any(|shift| {
        intervals.iter().any(|(start, end)| {
            parameter + shift >= *start - 32.0 * Real::EPSILON
                && parameter + shift <= *end + 32.0 * Real::EPSILON
        })
    })
}

fn fit_curve(
    basis: Basis,
    start: Real,
    end: Real,
    fit_tolerance: Real,
    derivative_bound: Real,
) -> Result<NurbsCurve, GeometryError> {
    let max_span = (384.0 * fit_tolerance / derivative_bound).powf(0.25);
    if !max_span.is_finite() || max_span <= 0.0 {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let closed = (end - start - PERIOD).abs() <= 32.0 * Real::EPSILON;
    let mut breaks = vec![start];
    for index in -2..=2 {
        let node = (2 * index + 1) as Real * PI;
        if node > start + 32.0 * Real::EPSILON && node < end - 32.0 * Real::EPSILON {
            breaks.push(node);
        }
    }
    breaks.push(end);
    breaks.sort_by(Real::total_cmp);
    let mut counts = Vec::with_capacity(breaks.len() - 1);
    let mut segments = 0usize;
    for pair in breaks.windows(2) {
        let required = ((pair[1] - pair[0]) / max_span).ceil();
        if !required.is_finite() || required > (MAX_SEGMENTS - segments) as Real {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: 3 * MAX_SEGMENTS + 1,
            });
        }
        let count = (required as usize).max(1);
        if count > MAX_SEGMENTS - segments {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: 3 * MAX_SEGMENTS + 1,
            });
        }
        segments += count;
        counts.push(count);
    }
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let (first_point, mut previous_tangent) = basis.sample(start)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_parameter = start;
    for (span, &count) in breaks.windows(2).zip(&counts) {
        for segment in 1..=count {
            let parameter = if segment == count {
                span[1]
            } else {
                span[0] + (span[1] - span[0]) * (segment as Real / count as Real)
            };
            let (mut point, tangent) = basis.sample(parameter)?;
            if closed && parameter == end {
                point = first_point;
            }
            let handle = (parameter - previous_parameter) / 3.0;
            controls.push(previous_point.translated(previous_tangent.scaled(handle)?)?);
            controls.push(point.translated(tangent.scaled(-handle)?)?);
            controls.push(point);
            knots.extend([parameter; 3]);
            previous_point = point;
            previous_tangent = tangent;
            previous_parameter = parameter;
        }
    }
    knots.push(end);
    NurbsCurve::try_new(3, controls, knots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame() -> Frame3 {
        Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn cone(height: Real) -> NurbsSurface {
        NurbsSurface::try_cone(frame(), 0.75 * height, height).unwrap()
    }

    fn sphere() -> NurbsSurface {
        NurbsSurface::try_sphere(frame().with_origin(point(0.2, 0.0, 2.0)), 1.36).unwrap()
    }

    fn assert_on_walls(location: Point3, height: Real) {
        assert!((location.z() - 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
        assert!((location.distance_to(point(0.2, 0.0, 2.0)).unwrap() - 1.36).abs() < 5e-9);
        assert!((-5e-9..=height + 5e-9).contains(&location.z()));
    }

    #[test]
    fn pinched_sphere_cone_makes_one_nodal_closed_curve_in_both_orders() {
        let sphere = sphere();
        let cone = cone(4.0);
        for (left, right) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("sphere/cone internal tangency must make a nodal curve, got {events:#?}")
            };
            assert_eq!(curve.degree(), 3);
            assert!(curve.is_closed().unwrap());
            assert!((*curve.domain().end() - PERIOD).abs() < 1e-12);
            let first_node = curve.evaluate(PI).unwrap();
            let second_node = curve.evaluate(3.0 * PI).unwrap();
            assert!(first_node.distance_to(second_node).unwrap() < 1e-12);
            assert!(first_node.distance_to(point(-0.888, 0.0, 1.184)).unwrap() < 5e-9);
            let first_tangent = curve.derivative_at(PI).unwrap();
            let second_tangent = curve.derivative_at(3.0 * PI).unwrap();
            assert!(
                first_tangent
                    .cross(second_tangent)
                    .unwrap()
                    .length()
                    .unwrap()
                    > 0.1
            );
            for index in 0..=128 {
                let parameter = PERIOD * (index as Real / 128.0);
                assert_on_walls(curve.evaluate(parameter).unwrap(), 4.0);
            }
        }
    }

    #[test]
    fn pinched_sphere_cone_clips_to_arcs_and_isolated_rim_contact() {
        let events =
            surface_surface_intersection_events(&sphere(), &cone(1.3), Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(arc)] = events.as_slice() else {
            panic!("base must leave an arc through both node visits, got {events:#?}")
        };
        assert!(!arc.is_closed().unwrap());
        assert!(arc.domain().contains(&PI));
        assert!(arc.domain().contains(&(3.0 * PI)));
        for index in 0..=64 {
            let domain = arc.domain();
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            assert_on_walls(arc.evaluate(parameter).unwrap(), 1.3);
        }
        for endpoint in [*arc.domain().start(), *arc.domain().end()] {
            assert!((arc.evaluate(endpoint).unwrap().z() - 1.3).abs() < 5e-9);
        }

        let node_height = 1.184;
        let events =
            surface_surface_intersection_events(&sphere(), &cone(node_height), Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(lower_lobe)] = events.as_slice() else {
            panic!("base through node must retain the lower lobe, got {events:#?}")
        };
        assert!(lower_lobe.is_closed().unwrap());
        let domain = lower_lobe.domain();
        assert!(
            lower_lobe
                .evaluate(*domain.start())
                .unwrap()
                .distance_to(lower_lobe.evaluate(*domain.end()).unwrap())
                .unwrap()
                < 1e-12
        );
        for index in 0..=32 {
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
            assert_on_walls(lower_lobe.evaluate(parameter).unwrap(), node_height);
        }

        let events =
            surface_surface_intersection_events(&sphere(), &cone(0.9), Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(lower)] = events.as_slice() else {
            panic!("lower base must leave one lower arc, got {events:#?}")
        };
        assert!(!lower.domain().contains(&PI));
        for index in 0..=32 {
            let domain = lower.domain();
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
            assert_on_walls(lower.evaluate(parameter).unwrap(), 0.9);
        }

        let low = 2.1904 / (2.15 + 1.2_f64.sqrt());
        let events =
            surface_surface_intersection_events(&sphere(), &cone(low), Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("lower branch must touch the base in one point, got {events:#?}")
        };
        assert_on_walls(*contact, low);
        assert!((contact.z() - low).abs() < 5e-9);
        assert!(
            surface_surface_intersection_events(&sphere(), &cone(0.6), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn pinched_sphere_cone_handles_negative_and_distant_rotated_frames() {
        let negative = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let negative_sphere =
            NurbsSurface::try_sphere(frame().with_origin(point(0.2, 0.0, -2.0)), 1.36).unwrap();
        let events =
            surface_surface_intersection_events(&negative_sphere, &negative, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("negative cone height must keep nodal curve")
        };
        assert!(curve.is_closed().unwrap());
        for index in 0..=64 {
            let location = curve.evaluate(PERIOD * (index as Real / 64.0)).unwrap();
            assert!((location.z() + 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
            assert!((location.distance_to(point(0.2, 0.0, -2.0)).unwrap() - 1.36).abs() < 5e-9);
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let center = rotated.point_at([0.12, 0.16, 2.0]).unwrap();
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let sphere = NurbsSurface::try_sphere(rotated.with_origin(center), 1.36).unwrap();
        for (left, right) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("distant rotated sphere/cone must make a nodal curve")
            };
            assert!(curve.is_closed().unwrap());
            assert!(
                curve
                    .evaluate(PI)
                    .unwrap()
                    .distance_to(curve.evaluate(3.0 * PI).unwrap())
                    .unwrap()
                    < 1e-9
            );
            for index in 0..=64 {
                let location = curve.evaluate(PERIOD * (index as Real / 64.0)).unwrap();
                let local = rotated.coordinates_of(location).unwrap();
                assert!((local[2] - 4.0 * local[0].hypot(local[1]) / 3.0).abs() < 4e-7);
                assert!((location.distance_to(center).unwrap() - 1.36).abs() < 4e-7);
            }
        }
    }
}
