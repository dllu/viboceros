//! Two smooth branches where an offset sphere cuts a cone away from its apex.
//!
//! Solving for cone height gives a quadratic transverse coordinate. A cosine
//! substitution removes the square-root endpoints at each radial turn, so
//! each branch can be fitted with a bounded-error cubic Hermite curve.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS: usize = 4096;
const HALF_TURN: Real = std::f64::consts::PI;

#[derive(Clone, Copy)]
struct Basis {
    frame: Frame3,
    axial_sign: Real,
    direction: [Real; 2],
    offset: Real,
    center_axial: Real,
    quadratic: Real,
    other_linear: Real,
    constant: Real,
    other_minimum: Real,
    axial_middle: Real,
    axial_radius: Real,
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
    let constant =
        (offset.hypot(center_axial) - sphere_radius) * (offset.hypot(center_axial) + sphere_radius);
    let cosine_coefficient = slope * offset;
    let maximum_linear = center_axial + cosine_coefficient;
    let root = (maximum_linear * maximum_linear - quadratic * constant).sqrt();
    let low = constant / (maximum_linear + root);
    let high = (maximum_linear + root) / quadratic;
    let other_linear = center_axial - cosine_coefficient;
    let other_minimum = constant - other_linear * other_linear / quadratic;
    let basis = Basis {
        frame: cone_frame,
        axial_sign: signed_height.signum(),
        direction: [center_x / offset, center_y / offset],
        offset,
        center_axial,
        quadratic,
        other_linear,
        constant,
        other_minimum,
        axial_middle: 0.5 * (low + high),
        axial_radius: 0.5 * (high - low),
    };
    // The opposite generator may have roots only at negative cone heights.
    // In that case its quadratic can have a negative minimum on the full
    // number line while remaining positive throughout this finite branch.
    let other_lower_bound = basis.other_quadratic((other_linear / quadratic).clamp(low, high));
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(height).max(sphere_radius))
        .max(coordinate_roundoff);
    if !fit_tolerance.is_finite() || !other_lower_bound.is_finite() || other_lower_bound <= 0.0 {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "turning sphere/cone fit is ill-conditioned",
        });
    }
    if low > height + fit_tolerance {
        return Ok(Vec::new());
    }
    if (height - low).abs() <= fit_tolerance {
        return Ok(vec![SurfaceSurfaceIntersectionEvent::Point(
            basis.sample(0.0, 1.0)?.0,
        )]);
    }
    let derivative_bound = fourth_derivative_bound(basis, other_lower_bound);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "turning sphere/cone derivative bound is not finite",
        });
    }
    let (first_end, second_start) = if height >= high - fit_tolerance {
        (HALF_TURN, 0.0)
    } else {
        let first_end = ((basis.axial_middle - height) / basis.axial_radius)
            .clamp(-1.0, 1.0)
            .acos();
        let second_start = ((height - basis.axial_middle) / basis.axial_radius)
            .clamp(-1.0, 1.0)
            .acos();
        (first_end, second_start)
    };
    Ok(vec![
        SurfaceSurfaceIntersectionEvent::Curve(fit_curve(
            basis,
            0.0,
            first_end,
            1.0,
            fit_tolerance,
            derivative_bound,
        )?),
        SurfaceSurfaceIntersectionEvent::Curve(fit_curve(
            basis,
            second_start,
            HALF_TURN,
            -1.0,
            fit_tolerance,
            derivative_bound,
        )?),
    ])
}

impl Basis {
    fn other_quadratic(self, axial: Real) -> Real {
        if self.other_linear <= 0.0 {
            return (self.quadratic * axial - 2.0 * self.other_linear)
                .mul_add(axial, self.constant);
        }
        let difference = axial - self.other_linear / self.quadratic;
        self.quadratic
            .mul_add(difference * difference, self.other_minimum)
    }

    fn sample(self, parameter: Real, side: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = parameter.sin_cos();
        let axial = self.axial_radius.mul_add(-side * cosine, self.axial_middle);
        let axial_derivative = side * self.axial_radius * sine;
        let along = (self.quadratic * axial * axial - 2.0 * self.center_axial * axial
            + self.constant)
            / (2.0 * self.offset);
        let along_derivative =
            (self.quadratic * axial - self.center_axial) * axial_derivative / self.offset;
        let other = self.other_quadratic(axial);
        let root = other.sqrt();
        let transverse_scale = self.quadratic.sqrt() * self.axial_radius / (2.0 * self.offset);
        let transverse = side * transverse_scale * sine * root;
        let other_derivative =
            2.0 * (self.quadratic * axial - self.other_linear) * axial_derivative;
        let transverse_derivative =
            side * transverse_scale * (cosine * root + sine * other_derivative / (2.0 * root));
        let [ux, uy] = self.direction;
        let local = [
            ux.mul_add(along, -uy * transverse),
            uy.mul_add(along, ux * transverse),
            self.axial_sign * axial,
        ];
        let derivative = [
            ux.mul_add(along_derivative, -uy * transverse_derivative),
            uy.mul_add(along_derivative, ux * transverse_derivative),
            self.axial_sign * axial_derivative,
        ];
        let x = self.frame.x_axis().as_vector().to_array();
        let y = self.frame.y_axis().as_vector().to_array();
        let z = self.frame.z_axis().as_vector().to_array();
        let tangent = Vector3::try_from(std::array::from_fn(|i| {
            derivative[0].mul_add(x[i], derivative[1].mul_add(y[i], derivative[2] * z[i]))
        }))?;
        Ok((self.frame.point_at(local)?, tangent))
    }
}

fn fourth_derivative_bound(basis: Basis, other_lower_bound: Real) -> Real {
    let middle = basis.axial_middle;
    let radius = basis.axial_radius;
    let a = basis.quadratic;
    let b = basis.other_linear;
    let low = middle - radius;
    let high = middle + radius;
    let root = other_lower_bound.sqrt();
    let lower3 = other_lower_bound * root;
    let lower5 = other_lower_bound * lower3;
    let lower7 = other_lower_bound * lower5;
    let g0 = basis
        .other_quadratic(low)
        .max(basis.other_quadratic(high))
        .sqrt();
    let first_harmonic = 2.0 * radius * (a * middle - b);
    let second_harmonic = a * radius * radius / 2.0;
    let q1 = first_harmonic.abs() + 2.0 * second_harmonic;
    let q2 = first_harmonic.abs() + 4.0 * second_harmonic;
    let q3 = first_harmonic.abs() + 8.0 * second_harmonic;
    let q4 = first_harmonic.abs() + 16.0 * second_harmonic;
    let g1 = q1 / (2.0 * root);
    let g2 = q2 / (2.0 * root) + q1 * q1 / (4.0 * lower3);
    let g3 = q3 / (2.0 * root) + 3.0 * q1 * q2 / (4.0 * lower3) + 3.0 * q1.powi(3) / (8.0 * lower5);
    let g4 = q4 / (2.0 * root)
        + (4.0 * q1 * q3 + 3.0 * q2 * q2) / (4.0 * lower3)
        + 9.0 * q1 * q1 * q2 / (4.0 * lower5)
        + 15.0 * q1.powi(4) / (16.0 * lower7);
    let transverse_scale = a.sqrt() * radius / (2.0 * basis.offset);
    let transverse_fourth = transverse_scale * (g0 + 4.0 * g1 + 6.0 * g2 + 4.0 * g3 + g4);
    let along_first_harmonic = radius * (a * middle - basis.center_axial) / basis.offset;
    let along_second_harmonic = a * radius * radius / (4.0 * basis.offset);
    let along_fourth = along_first_harmonic.abs() + 16.0 * along_second_harmonic.abs();
    along_fourth.hypot(transverse_fourth).hypot(radius)
}

fn fit_curve(
    basis: Basis,
    start: Real,
    end: Real,
    side: Real,
    fit_tolerance: Real,
    derivative_bound: Real,
) -> Result<NurbsCurve, GeometryError> {
    let max_span = (384.0 * fit_tolerance / derivative_bound).powf(0.25);
    if !max_span.is_finite() || max_span <= 0.0 {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let required = ((end - start) / max_span).ceil();
    if !required.is_finite() || required > MAX_SEGMENTS as Real {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let segments = (required as usize).max(1);
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let (first_point, mut previous_tangent) = basis.sample(start, side)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_parameter = start;
    for segment in 1..=segments {
        let parameter = if segment == segments {
            end
        } else {
            start + (end - start) * (segment as Real / segments as Real)
        };
        let (point, tangent) = basis.sample(parameter, side)?;
        let handle = (parameter - previous_parameter) / 3.0;
        controls.push(previous_point.translated(previous_tangent.scaled(handle)?)?);
        controls.push(point.translated(tangent.scaled(-handle)?)?);
        controls.push(point);
        knots.extend([parameter; 3]);
        previous_point = point;
        previous_tangent = tangent;
        previous_parameter = parameter;
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
        NurbsSurface::try_sphere(frame().with_origin(point(0.5, 0.0, 2.0)), 1.5).unwrap()
    }

    fn assert_on_walls(location: Point3, height: Real) {
        assert!((location.z() - 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
        assert!((location.distance_to(point(0.5, 0.0, 2.0)).unwrap() - 1.5).abs() < 5e-9);
        assert!((-5e-9..=height + 5e-9).contains(&location.z()));
    }

    #[test]
    fn turning_sphere_cone_matches_saved_rhino_two_branch_fixture() {
        let cone = cone(4.0);
        let sphere = sphere();
        for (left, right) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for (index, event) in events.iter().enumerate() {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("turning section must produce two curves, got {events:#?}")
                };
                assert_eq!(curve.degree(), 3);
                assert!(!curve.is_closed().unwrap());
                // Saved Rhino 8 observation for the noncoaxial fixture.
                assert!((curve.length(Tolerance::DEFAULT).unwrap() - 5.5724217392).abs() < 2e-6);
                let domain = curve.domain();
                for sample in 0..=64 {
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (sample as Real / 64.0);
                    let location = curve.evaluate(parameter).unwrap();
                    assert_on_walls(location, 4.0);
                    if sample == 32 {
                        assert!(location.y() * if index == 0 { 1.0 } else { -1.0 } > 0.0);
                    }
                }
            }
            let [
                SurfaceSurfaceIntersectionEvent::Curve(first),
                SurfaceSurfaceIntersectionEvent::Curve(second),
            ] = events.as_slice()
            else {
                unreachable!()
            };
            assert!(
                first
                    .evaluate(*first.domain().start())
                    .unwrap()
                    .distance_to(second.evaluate(*second.domain().end()).unwrap())
                    .unwrap()
                    < 5e-9
            );
            assert!(
                first
                    .evaluate(*first.domain().end())
                    .unwrap()
                    .distance_to(second.evaluate(*second.domain().start()).unwrap())
                    .unwrap()
                    < 5e-9
            );
        }
    }

    #[test]
    fn turning_sphere_cone_clips_at_base_and_keeps_tangent_contact() {
        let events =
            surface_surface_intersection_events(&sphere(), &cone(1.5), Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("cone base must clip both turning branches")
            };
            for sample in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (sample as Real / 32.0);
                assert_on_walls(curve.evaluate(parameter).unwrap(), 1.5);
            }
            let ends = [
                curve.evaluate(*curve.domain().start()).unwrap(),
                curve.evaluate(*curve.domain().end()).unwrap(),
            ];
            assert!(
                ends.iter()
                    .any(|location| (location.z() - 1.5).abs() < 5e-9)
            );
        }
        let low = 0.5049137967640384;
        let events =
            surface_surface_intersection_events(&sphere(), &cone(low), Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("base must touch at the lower turn, got {events:#?}")
        };
        assert_on_walls(*contact, low);
        assert!((contact.z() - low).abs() < 5e-9);
        assert!(
            surface_surface_intersection_events(&sphere(), &cone(0.2), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn turning_sphere_cone_handles_negative_and_distant_rotated_frames() {
        let negative = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let negative_sphere =
            NurbsSurface::try_sphere(frame().with_origin(point(0.5, 0.0, -2.0)), 1.5).unwrap();
        let events =
            surface_surface_intersection_events(&negative_sphere, &negative, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("negative height must keep both branches")
            };
            for sample in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (sample as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!((location.z() + 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
                assert!((location.distance_to(point(0.5, 0.0, -2.0)).unwrap() - 1.5).abs() < 5e-9);
            }
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let center = rotated.point_at([0.3, 0.4, 2.0]).unwrap();
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let sphere = NurbsSurface::try_sphere(rotated.with_origin(center), 1.5).unwrap();
        for (left, right) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("rotated turning section must produce a curve")
                };
                for sample in 0..=32 {
                    let domain = curve.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (sample as Real / 32.0);
                    let location = curve.evaluate(parameter).unwrap();
                    let local = rotated.coordinates_of(location).unwrap();
                    assert!((local[2] - 4.0 * local[0].hypot(local[1]) / 3.0).abs() < 4e-7);
                    assert!((location.distance_to(center).unwrap() - 1.5).abs() < 4e-7);
                }
            }
        }
    }

    #[test]
    fn turning_section_remains_on_positive_cone_when_opposite_roots_are_negative() {
        let center = point(2.0, 0.0, 0.5);
        let sphere = NurbsSurface::try_sphere(frame().with_origin(center), 2.0).unwrap();
        for height in [4.0, 1.5] {
            let cone = cone(height);
            for (first, second) in [(&sphere, &cone), (&cone, &sphere)] {
                let events =
                    surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
                let [
                    SurfaceSurfaceIntersectionEvent::Curve(upper),
                    SurfaceSurfaceIntersectionEvent::Curve(lower),
                ] = events.as_slice()
                else {
                    panic!("expected two turning branches, got {events:#?}")
                };
                for curve in [upper, lower] {
                    assert!(!curve.is_closed().unwrap());
                    for sample in 0..=64 {
                        let domain = curve.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (sample as Real / 64.0);
                        let location = curve.evaluate(parameter).unwrap();
                        assert!((location.distance_to(center).unwrap() - 2.0).abs() < 5e-9);
                        assert!(
                            (location.z() - 4.0 * location.x().hypot(location.y()) / 3.0).abs()
                                < 5e-9
                        );
                        assert!((-5e-9..=height + 5e-9).contains(&location.z()));
                    }
                }
                assert!(
                    upper
                        .evaluate(*upper.domain().start())
                        .unwrap()
                        .distance_to(lower.evaluate(*lower.domain().end()).unwrap())
                        .unwrap()
                        < 5e-9
                );
                if height == 4.0 {
                    assert!(
                        upper
                            .evaluate(*upper.domain().end())
                            .unwrap()
                            .distance_to(lower.evaluate(*lower.domain().start()).unwrap())
                            .unwrap()
                            < 5e-9
                    );
                } else {
                    for curve in [upper, lower] {
                        let end = curve.evaluate(*curve.domain().end()).unwrap();
                        let start = curve.evaluate(*curve.domain().start()).unwrap();
                        assert!(
                            (end.z() - height).abs() < 5e-9 || (start.z() - height).abs() < 5e-9
                        );
                    }
                }
            }
        }
        assert!(
            surface_surface_intersection_events(&sphere, &cone(0.03), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn turning_section_with_negative_opposite_roots_supports_signed_and_rotated_cones() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (cone_frame, sign) in [(frame(), -1.0), (rotated, 1.0)] {
            let center = cone_frame.point_at([1.2, 1.6, sign * 0.5]).unwrap();
            let cone = NurbsSurface::try_cone(cone_frame, 3.0, sign * 4.0).unwrap();
            let sphere = NurbsSurface::try_sphere(cone_frame.with_origin(center), 2.0).unwrap();
            let events =
                surface_surface_intersection_events(&sphere, &cone, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("expected a turning branch")
                };
                for sample in 0..=32 {
                    let domain = curve.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (sample as Real / 32.0);
                    let location = curve.evaluate(parameter).unwrap();
                    let local = cone_frame.coordinates_of(location).unwrap();
                    assert!((sign * local[2] - 4.0 * local[0].hypot(local[1]) / 3.0).abs() < 4e-7);
                    assert!((location.distance_to(center).unwrap() - 2.0).abs() < 4e-7);
                }
            }
        }
    }
}
