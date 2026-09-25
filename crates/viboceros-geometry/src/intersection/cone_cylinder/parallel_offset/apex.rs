//! The cone/cylinder section with the cylinder wall through the cone apex.
//!
//! On either side of the apex, rho = 2c |cos(u/2)|. This removes the square
//! root singularity and gives an analytic tangent on each side. A triple cubic
//! knot at the apex retains its real corner without rounding it away.

use super::{
    MAX_SEGMENTS, Section, SurfaceSurfaceIntersectionEvent, TURN, active_intervals,
    angle_in_intervals, angular_cuts,
};
use crate::{GeometryError, NurbsCurve, Point3, Real, Vector3};

const PI: Real = std::f64::consts::PI;

pub(super) fn intersect(
    section: Section,
    (radial_low, radial_high): (Real, Real),
    (cylinder_start, cylinder_height, axis_dot): (Real, Real, Real),
    cone_height: Real,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    // The surface/surface API omits a contact consisting solely of the cone apex.
    if radial_high <= fit_tolerance {
        return Ok(Vec::new());
    }
    let cuts = angular_cuts(section, radial_low, radial_high);
    let intervals = active_intervals(section, &cuts, radial_low, radial_high);
    let mut events = Vec::new();
    for angle in cuts {
        let radial = section.radial(angle);
        if radial <= fit_tolerance
            || radial < radial_low - fit_tolerance
            || radial > radial_high + fit_tolerance
            || angle_in_intervals(angle, &intervals)
        {
            continue;
        }
        let axial = section.height_per_radius * radial;
        let cylinder_axial = (axial - cylinder_start) / axis_dot;
        let on_rim = axial.abs() <= fit_tolerance
            || (axial - cone_height).abs() <= fit_tolerance
            || cylinder_axial.abs() <= fit_tolerance
            || (cylinder_axial - cylinder_height).abs() <= fit_tolerance;
        if on_rim {
            let side = smooth_side(angle);
            let point = section.singular_sample(angle, side)?.0;
            if !events.iter().any(|event| {
                matches!(event, SurfaceSurfaceIntersectionEvent::Point(existing)
                    if existing.distance_to(point).is_ok_and(|distance| distance <= fit_tolerance))
            }) {
                events.push(SurfaceSurfaceIntersectionEvent::Point(point));
            }
        }
    }
    let derivative_bound = section
        .radius
        .hypot(section.radius * section.height_per_radius.abs() / 8.0);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "apex cone/cylinder fit is ill-conditioned",
        });
    }
    for (start, end) in intervals {
        events.push(SurfaceSurfaceIntersectionEvent::Curve(fit_curve(
            section,
            start,
            end,
            fit_tolerance,
            derivative_bound,
        )?));
    }
    Ok(events)
}

fn smooth_side(angle: Real) -> Real {
    if (0.5 * angle).cos() < 0.0 { -1.0 } else { 1.0 }
}

impl Section {
    fn singular_sample(self, angle: Real, side: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = angle.sin_cos();
        let [ux, uy] = self.direction;
        let half_cosine = (0.5 * angle).cos();
        let along = 2.0 * self.radius * half_cosine * half_cosine;
        let across = self.radius * sine;
        let radial = 2.0 * self.radius * side * half_cosine;
        let local = [
            ux.mul_add(along, -uy * across),
            uy.mul_add(along, ux * across),
            self.height_per_radius * radial,
        ];
        let derivative_along = -self.radius * sine;
        let derivative_across = self.radius * cosine;
        let derivative_radial = -self.radius * side * (0.5 * angle).sin();
        let derivative = [
            ux.mul_add(derivative_along, -uy * derivative_across),
            uy.mul_add(derivative_along, ux * derivative_across),
            self.height_per_radius * derivative_radial,
        ];
        let x = self.frame.x_axis().as_vector().to_array();
        let y = self.frame.y_axis().as_vector().to_array();
        let z = self.frame.z_axis().as_vector().to_array();
        let tangent = Vector3::try_from(std::array::from_fn(|i| {
            derivative[0].mul_add(x[i], derivative[1].mul_add(y[i], derivative[2] * z[i]))
        }))?;
        let apex = [-PI, PI, 3.0 * PI].contains(&angle);
        let point = if apex {
            self.frame.origin()
        } else {
            self.frame.point_at(local)?
        };
        Ok((point, tangent))
    }
}

fn fit_curve(
    section: Section,
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
    let mut breaks = vec![start];
    for index in -2..=2 {
        let apex = (2 * index + 1) as Real * PI;
        if apex > start + 32.0 * Real::EPSILON && apex < end - 32.0 * Real::EPSILON {
            breaks.push(apex);
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
    let closed = (end - start - TURN).abs() <= 32.0 * Real::EPSILON;
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let first_side = smooth_side(0.5 * (breaks[0] + breaks[1]));
    let (first_point, mut previous_tangent) = section.singular_sample(start, first_side)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for (index, (span, &count)) in breaks.windows(2).zip(&counts).enumerate() {
        let side = smooth_side(0.5 * (span[0] + span[1]));
        if index > 0 {
            previous_tangent = section.singular_sample(span[0], side)?.1;
        }
        for segment in 1..=count {
            let angle = if segment == count {
                span[1]
            } else {
                span[0] + (span[1] - span[0]) * (segment as Real / count as Real)
            };
            let (mut point, tangent) = section.singular_sample(angle, side)?;
            if closed && angle == end {
                point = first_point;
            }
            let handle = (angle - previous_angle) / 3.0;
            controls.push(previous_point.translated(previous_tangent.scaled(handle)?)?);
            controls.push(point.translated(tangent.scaled(-handle)?)?);
            controls.push(point);
            knots.extend([angle; 3]);
            previous_point = point;
            previous_tangent = tangent;
            previous_angle = angle;
        }
    }
    knots.push(end);
    NurbsCurve::try_new(3, controls, knots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame3, NurbsSurface, Tolerance, surface_surface_intersection_events};

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

    fn cone() -> NurbsSurface {
        NurbsSurface::try_cone(frame(), 3.0, 4.0).unwrap()
    }

    fn cylinder(start: Real, end: Real) -> NurbsSurface {
        NurbsSurface::try_cylinder(
            frame().with_origin(point(1.5, 0.0, start)),
            1.5,
            0.0,
            end - start,
        )
        .unwrap()
    }

    fn assert_on_walls(location: Point3, low: Real, high: Real) {
        let radial = location.x().hypot(location.y());
        assert!((location.z() - 4.0 * radial / 3.0).abs() < 5e-9);
        assert!(((location.x() - 1.5).hypot(location.y()) - 1.5).abs() < 5e-9);
        assert!((low - 5e-9..=high + 5e-9).contains(&location.z()));
    }

    #[test]
    fn apex_cone_cylinder_has_one_closed_curve_with_an_exact_corner() {
        let cone = cone();
        let cylinder = cylinder(0.0, 4.0);
        for (left, right) in [(&cone, &cylinder), (&cylinder, &cone)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("apex section must form one closed curve, got {events:#?}")
            };
            assert_eq!(curve.degree(), 3);
            assert!(curve.is_closed().unwrap());
            assert!(
                curve
                    .evaluate(PI)
                    .unwrap()
                    .distance_to(point(0.0, 0.0, 0.0))
                    .unwrap()
                    < 1e-12
            );
            let before = curve.evaluate(PI - 1e-4).unwrap();
            let after = curve.evaluate(PI + 1e-4).unwrap();
            assert!((before.z() - after.z()).abs() < 1e-8);
            assert!((before.y() - after.y()).abs() > 2e-4);
            let left_tangent = curve.derivative_at(PI - 1e-5).unwrap();
            let right_tangent = curve.derivative_at(PI + 1e-5).unwrap();
            assert!(left_tangent.cross(right_tangent).unwrap().length().unwrap() > 1.0);
            for index in 0..=128 {
                let angle = TURN * (index as Real / 128.0);
                assert_on_walls(curve.evaluate(angle).unwrap(), 0.0, 4.0);
            }
        }
    }

    #[test]
    fn apex_cone_cylinder_clips_to_a_cornered_arc_and_two_smooth_arcs() {
        let low_band =
            surface_surface_intersection_events(&cone(), &cylinder(0.0, 2.0), Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(cornered)] = low_band.as_slice() else {
            panic!("lower height band must retain one cornered arc, got {low_band:#?}")
        };
        assert!(!cornered.is_closed().unwrap());
        assert!(cornered.domain().contains(&PI));
        assert!(
            cornered
                .evaluate(PI)
                .unwrap()
                .distance_to(point(0.0, 0.0, 0.0))
                .unwrap()
                < 1e-12
        );
        for index in 0..=64 {
            let domain = cornered.domain();
            let angle =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            assert_on_walls(cornered.evaluate(angle).unwrap(), 0.0, 2.0);
        }

        let middle_band =
            surface_surface_intersection_events(&cone(), &cylinder(1.0, 3.0), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(middle_band.len(), 2);
        for event in middle_band {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("middle height band must leave two smooth arcs")
            };
            assert!(!curve.domain().contains(&PI));
            for index in 0..=32 {
                let domain = curve.domain();
                let angle =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                assert_on_walls(curve.evaluate(angle).unwrap(), 1.0, 3.0);
            }
        }
    }

    #[test]
    fn apex_cone_cylinder_can_be_clipped_by_the_cone_base() {
        let wide =
            NurbsSurface::try_cylinder(frame().with_origin(point(2.0, 0.0, 0.0)), 2.0, 0.0, 5.0)
                .unwrap();
        let events =
            surface_surface_intersection_events(&cone(), &wide, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(arc)] = events.as_slice() else {
            panic!("cone base must clip the apex curve, got {events:#?}")
        };
        assert!(!arc.is_closed().unwrap());
        assert!(arc.domain().contains(&PI));
        assert!(
            arc.evaluate(PI)
                .unwrap()
                .distance_to(frame().origin())
                .unwrap()
                < 1e-12
        );
        for end in [*arc.domain().start(), *arc.domain().end()] {
            assert!((arc.evaluate(end).unwrap().z() - 4.0).abs() < 5e-9);
        }
        for index in 0..=64 {
            let domain = arc.domain();
            let angle =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            let location = arc.evaluate(angle).unwrap();
            assert!((location.z() - 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
            assert!(((location.x() - 2.0).hypot(location.y()) - 2.0).abs() < 5e-9);
            assert!((-5e-9..=4.0 + 5e-9).contains(&location.z()));
        }
    }

    #[test]
    fn apex_cone_cylinder_keeps_base_rim_contact_and_omits_apex_only_contact() {
        let events =
            surface_surface_intersection_events(&cone(), &cylinder(4.0, 5.0), Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("base rim must touch at one point, got {events:#?}")
        };
        assert!(contact.distance_to(point(3.0, 0.0, 4.0)).unwrap() < 1e-9);

        let apex_only =
            surface_surface_intersection_events(&cone(), &cylinder(-1.0, 0.0), Tolerance::DEFAULT)
                .unwrap();
        assert!(apex_only.is_empty());
    }

    #[test]
    fn apex_cone_cylinder_handles_negative_height_and_distant_rotated_frames() {
        let negative_cone = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let negative_cylinder = cylinder(-4.0, 0.0);
        let events = surface_surface_intersection_events(
            &negative_cone,
            &negative_cylinder,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("negative cone height must retain the apex curve")
        };
        assert!(curve.is_closed().unwrap());
        assert!(
            curve
                .evaluate(PI)
                .unwrap()
                .distance_to(frame().origin())
                .unwrap()
                < 1e-12
        );
        for index in 0..=64 {
            let location = curve.evaluate(TURN * (index as Real / 64.0)).unwrap();
            assert!((location.z() + 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
            assert!(((location.x() - 1.5).hypot(location.y()) - 1.5).abs() < 5e-9);
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder_frame = rotated.with_origin(rotated.point_at([1.5, 0.0, 0.0]).unwrap());
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let cylinder = NurbsSurface::try_cylinder(cylinder_frame, 1.5, 0.0, 4.0).unwrap();
        for (left, right) in [(&cone, &cylinder), (&cylinder, &cone)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("distant rotated apex section must be a curve")
            };
            assert!(curve.is_closed().unwrap());
            assert!(
                curve
                    .evaluate(PI)
                    .unwrap()
                    .distance_to(rotated.origin())
                    .unwrap()
                    < 1e-9
            );
            for index in 0..=64 {
                let location = curve.evaluate(TURN * (index as Real / 64.0)).unwrap();
                let local = rotated.coordinates_of(location).unwrap();
                let cylinder_local = cylinder_frame.coordinates_of(location).unwrap();
                assert!((local[2] - 4.0 * local[0].hypot(local[1]) / 3.0).abs() < 4e-7);
                assert!((cylinder_local[0].hypot(cylinder_local[1]) - 1.5).abs() < 4e-7);
            }
        }
    }
}
