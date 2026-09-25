//! Smooth intersections of a cone and an offset, parallel cylinder.
//!
//! At cylinder angle u the distance to the cone axis is
//! sqrt(d²+c²+2dc cos(u)). The cone height is proportional to that distance.
//! Cubic Hermite spans have a fourth-derivative error bound. The d = c case
//! has a separate analytic branch at the singular cone apex.

mod apex;

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS: usize = 4096;
const TURN: Real = std::f64::consts::TAU;

#[derive(Clone, Copy)]
struct Section {
    frame: Frame3,
    radius: Real,
    height_per_radius: Real,
    offset: Real,
    direction: [Real; 2],
}

pub(super) fn intersect(
    (cone_frame, cone_radius, cone_height): (Frame3, Real, Real),
    (cylinder_radius, cylinder_height): (Real, Real),
    (offset_x, offset_y, cylinder_start, axis_dot): (Real, Real, Real, Real),
    tolerance: Tolerance,
    coordinate_roundoff: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let offset = offset_x.hypot(offset_y);
    let section = Section {
        frame: cone_frame,
        radius: cylinder_radius,
        height_per_radius: cone_height / cone_radius,
        offset,
        direction: [offset_x / offset, offset_y / offset],
    };
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(cone_height.abs()).max(cylinder_radius))
        .max(coordinate_roundoff);
    if !fit_tolerance.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "offset cone/cylinder fit tolerance is not finite",
        });
    }
    let cylinder_end = axis_dot.mul_add(cylinder_height, cylinder_start);
    let axial_low = cylinder_start.min(cylinder_end).max(cone_height.min(0.0));
    let axial_high = cylinder_start.max(cylinder_end).min(cone_height.max(0.0));
    if axial_low > axial_high + fit_tolerance {
        return Ok(Vec::new());
    }
    let radial_a = axial_low / cone_height * cone_radius;
    let radial_b = axial_high / cone_height * cone_radius;
    let radial_low = radial_a.min(radial_b).max(0.0);
    let radial_high = radial_a.max(radial_b).min(cone_radius);
    let minimum = (offset - cylinder_radius).abs();
    let maximum = offset + cylinder_radius;
    if radial_low > maximum + fit_tolerance || radial_high < minimum - fit_tolerance {
        return Ok(Vec::new());
    }
    if minimum <= coordinate_roundoff {
        return apex::intersect(
            Section {
                offset: cylinder_radius,
                ..section
            },
            (radial_low, radial_high),
            (cylinder_start, cylinder_height, axis_dot),
            cone_height,
            fit_tolerance,
        );
    }
    let derivative_bound = fourth_derivative_bound(section, minimum);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "offset cone/cylinder fit is ill-conditioned",
        });
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
            let point = section.sample(angle)?.0;
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
            section,
            start,
            end,
            fit_tolerance,
            derivative_bound,
        )?));
    }
    Ok(events)
}

impl Section {
    fn radicand(self, angle: Real) -> Real {
        let difference = self.offset - self.radius;
        let half_cosine = (0.5 * angle).cos();
        difference.mul_add(
            difference,
            4.0 * self.offset * self.radius * half_cosine * half_cosine,
        )
    }

    fn radial(self, angle: Real) -> Real {
        self.radicand(angle).sqrt()
    }

    fn sample(self, angle: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = angle.sin_cos();
        let [ux, uy] = self.direction;
        let along = self.offset + self.radius * cosine;
        let across = self.radius * sine;
        let radial = self.radial(angle);
        let local = [
            ux.mul_add(along, -uy * across),
            uy.mul_add(along, ux * across),
            self.height_per_radius * radial,
        ];
        let derivative_along = -self.radius * sine;
        let derivative_across = self.radius * cosine;
        let derivative_radial = -self.offset * self.radius * sine / radial;
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
        Ok((self.frame.point_at(local)?, tangent))
    }
}

fn angular_cuts(section: Section, radial_low: Real, radial_high: Real) -> Vec<Real> {
    let minimum = (section.offset - section.radius).abs();
    let maximum = section.offset + section.radius;
    let mut cuts = vec![0.0, std::f64::consts::PI, TURN];
    for radius in [radial_low, radial_high] {
        if radius > minimum && radius < maximum {
            let half_cosine = ((radius - minimum) * (radius + minimum)
                / (4.0 * section.offset * section.radius))
                .clamp(0.0, 1.0)
                .sqrt();
            let angle = 2.0 * half_cosine.acos();
            cuts.push(angle);
            cuts.push(TURN - angle);
        }
    }
    cuts.sort_by(Real::total_cmp);
    cuts.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);
    cuts
}

fn fourth_derivative_bound(section: Section, minimum: Real) -> Real {
    let b = 2.0 * section.offset * section.radius;
    let lower = minimum * minimum;
    let root3 = lower * minimum;
    let root5 = lower * root3;
    let root7 = lower * root5;
    let radial_fourth = b / (2.0 * minimum)
        + 7.0 * b * b / (4.0 * root3)
        + 9.0 * b.powi(3) / (4.0 * root5)
        + 15.0 * b.powi(4) / (16.0 * root7);
    section
        .radius
        .hypot(section.height_per_radius.abs() * radial_fourth)
}

fn active_intervals(
    section: Section,
    cuts: &[Real],
    radial_low: Real,
    radial_high: Real,
) -> Vec<(Real, Real)> {
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in cuts.windows(2) {
        let radius = section.radial(0.5 * (pair[0] + pair[1]));
        if (radial_low..=radial_high).contains(&radius) {
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
        && intervals.last().is_some_and(|last| last.1 == TURN)
    {
        let first = intervals.remove(0);
        let last = intervals.pop().expect("at least two intervals");
        intervals.insert(0, (last.0 - TURN, first.1));
    }
    intervals
}

fn angle_in_intervals(angle: Real, intervals: &[(Real, Real)]) -> bool {
    [-TURN, 0.0, TURN].into_iter().any(|shift| {
        intervals.iter().any(|(start, end)| {
            angle + shift >= *start - 32.0 * Real::EPSILON
                && angle + shift <= *end + 32.0 * Real::EPSILON
        })
    })
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
    let required = ((end - start) / max_span).ceil();
    if !required.is_finite() || required > MAX_SEGMENTS as Real {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let closed = (end - start - TURN).abs() <= 32.0 * Real::EPSILON;
    let segments = (required as usize).max(if closed { 4 } else { 1 });
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let (first_point, mut previous_tangent) = section.sample(start)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for segment in 1..=segments {
        let angle = if segment == segments {
            end
        } else {
            start + (end - start) * (segment as Real / segments as Real)
        };
        let (mut point, tangent) = section.sample(angle)?;
        if closed && segment == segments {
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

    fn cone() -> NurbsSurface {
        NurbsSurface::try_cone(frame(), 3.0, 4.0).unwrap()
    }

    fn cylinder(start: Real, end: Real) -> NurbsSurface {
        let origin = point(0.5, 0.0, start);
        NurbsSurface::try_cylinder(frame().with_origin(origin), 1.5, 0.0, end - start).unwrap()
    }

    fn assert_on_walls(point: Point3, z_low: Real, z_high: Real) {
        let rho = point.x().hypot(point.y());
        assert!((point.z() - 4.0 * rho / 3.0).abs() < 5e-9);
        assert!(((point.x() - 0.5).hypot(point.y()) - 1.5).abs() < 5e-9);
        assert!((z_low - 5e-9..=z_high + 5e-9).contains(&point.z()));
    }

    #[test]
    fn parallel_offset_cone_cylinder_makes_closed_curve_in_both_orders() {
        let cone = cone();
        let cylinder = cylinder(0.0, 4.0);
        for (first, second) in [(&cone, &cylinder), (&cylinder, &cone)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("expected one closed curve, got {events:#?}")
            };
            assert_eq!(curve.degree(), 3);
            assert!(curve.is_closed().unwrap());
            // Saved Rhino 8 observation for the parallel_noncoaxial fixture.
            assert!((curve.length(Tolerance::DEFAULT).unwrap() - 9.874243699048625).abs() < 2e-6);
            let domain = curve.domain();
            for index in 0..=128 {
                let angle =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 128.0);
                assert_on_walls(curve.evaluate(angle).unwrap(), 0.0, 4.0);
            }
        }
    }

    #[test]
    fn parallel_offset_cone_cylinder_clips_to_arcs_and_rim_points() {
        let events =
            surface_surface_intersection_events(&cone(), &cylinder(1.7, 2.3), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite cylinder must leave two arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let angle =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                assert_on_walls(curve.evaluate(angle).unwrap(), 1.7, 2.3);
            }
        }
        let events = surface_surface_intersection_events(
            &cone(),
            &cylinder(8.0 / 3.0, 4.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("cylinder rim must touch at one point, got {events:#?}")
        };
        assert_on_walls(*contact, 8.0 / 3.0, 4.0);
        assert!(
            surface_surface_intersection_events(&cone(), &cylinder(3.0, 4.0), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn parallel_offset_cone_cylinder_handles_opposed_and_negative_axes() {
        let down = Frame3::try_from_normal(
            point(0.5, 0.0, 4.0),
            Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let opposed = NurbsSurface::try_cylinder(down, 1.5, 0.0, 4.0).unwrap();
        let events =
            surface_surface_intersection_events(&cone(), &opposed, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("opposed cylinder axis must retain one closed section")
        };
        assert!(curve.is_closed().unwrap());
        for index in 0..=64 {
            let angle = *curve.domain().start()
                + (*curve.domain().end() - *curve.domain().start()) * (index as Real / 64.0);
            assert_on_walls(curve.evaluate(angle).unwrap(), 0.0, 4.0);
        }

        let negative_cone = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let negative_cylinder =
            NurbsSurface::try_cylinder(frame().with_origin(point(0.5, 0.0, -4.0)), 1.5, 0.0, 4.0)
                .unwrap();
        let events = surface_surface_intersection_events(
            &negative_cone,
            &negative_cylinder,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("negative-height cone must retain one closed section")
        };
        assert!(curve.is_closed().unwrap());
        for index in 0..=64 {
            let angle = *curve.domain().start()
                + (*curve.domain().end() - *curve.domain().start()) * (index as Real / 64.0);
            let location = curve.evaluate(angle).unwrap();
            assert!((location.z() + 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
            assert!(((location.x() - 0.5).hypot(location.y()) - 1.5).abs() < 5e-9);
        }
    }

    #[test]
    fn parallel_offset_cone_cylinder_handles_rotated_distant_frames() {
        let cone_frame = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder_frame = cone_frame.with_origin(cone_frame.point_at([0.5, 0.0, 0.0]).unwrap());
        let cone = NurbsSurface::try_cone(cone_frame, 3.0, 4.0).unwrap();
        let cylinder = NurbsSurface::try_cylinder(cylinder_frame, 1.5, 0.0, 4.0).unwrap();
        for (first, second) in [(&cone, &cylinder), (&cylinder, &cone)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("rotated distant section must be a closed curve")
            };
            for index in 0..=64 {
                let angle = *curve.domain().start()
                    + (*curve.domain().end() - *curve.domain().start()) * (index as Real / 64.0);
                let location = curve.evaluate(angle).unwrap();
                let local = cone_frame.coordinates_of(location).unwrap();
                let cylinder_local = cylinder_frame.coordinates_of(location).unwrap();
                assert!((local[2] - 4.0 * local[0].hypot(local[1]) / 3.0).abs() < 4e-7);
                assert!((cylinder_local[0].hypot(cylinder_local[1]) - 1.5).abs() < 4e-7);
            }
        }
    }

    #[test]
    fn parallel_offset_cone_cylinder_clips_at_cone_base_and_retains_tangent_point() {
        let crossing =
            NurbsSurface::try_cylinder(frame().with_origin(point(2.5, 0.0, 0.0)), 1.0, 0.0, 5.0)
                .unwrap();
        let events =
            surface_surface_intersection_events(&cone(), &crossing, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(arc)] = events.as_slice() else {
            panic!("cone base must trim the offset section to one arc, got {events:#?}")
        };
        assert!(!arc.is_closed().unwrap());
        let domain = arc.domain();
        for parameter in [*domain.start(), *domain.end()] {
            assert!((arc.evaluate(parameter).unwrap().z() - 4.0).abs() < 2e-8);
        }
        for index in 0..=64 {
            let angle =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            let location = arc.evaluate(angle).unwrap();
            let rho = location.x().hypot(location.y());
            assert!((location.z() - 4.0 * rho / 3.0).abs() < 5e-9);
            assert!(((location.x() - 2.5).hypot(location.y()) - 1.0).abs() < 5e-9);
            assert!((-1e-9..=4.0 + 5e-9).contains(&location.z()));
        }

        let tangent =
            NurbsSurface::try_cylinder(frame().with_origin(point(4.0, 0.0, 0.0)), 1.0, 0.0, 5.0)
                .unwrap();
        let events =
            surface_surface_intersection_events(&cone(), &tangent, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("cone base must touch the cylinder at one point, got {events:#?}")
        };
        assert!(contact.distance_to(point(3.0, 0.0, 4.0)).unwrap() < 2e-9);
    }
}
