//! A perpendicular cylinder axis through the cone apex.
//!
//! With X along the cylinder axis and Y across it, the two wall equations
//! eliminate Z to give X² + (1+k²)Y² = k²R². This ellipse has a smooth,
//! strictly positive cone height Z = R sqrt(1-m sin²u) on either cone sheet.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const PI: Real = std::f64::consts::PI;
const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
struct Section {
    frame: Frame3,
    direction: [Real; 2],
    axial_sign: Real,
    radius: Real,
    axial_amplitude: Real,
    cross_amplitude: Real,
    modulus: Real,
    quadratic: Real,
}

pub(super) fn intersect(
    (cone_frame, cone_radius, signed_cone_height): (Frame3, Real, Real),
    (cylinder_radius, cylinder_height): (Real, Real),
    (origin_x, origin_y, direction_x, direction_y): (Real, Real, Real, Real),
    tolerance: Tolerance,
    coordinate_roundoff: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let direction_length = direction_x.hypot(direction_y);
    let direction = [
        direction_x / direction_length,
        direction_y / direction_length,
    ];
    let slope = cone_radius / signed_cone_height.abs();
    let quadratic = slope.mul_add(slope, 1.0);
    let section = Section {
        frame: cone_frame,
        direction,
        axial_sign: signed_cone_height.signum(),
        radius: cylinder_radius,
        axial_amplitude: slope * cylinder_radius,
        cross_amplitude: slope * cylinder_radius / quadratic.sqrt(),
        modulus: slope * slope / quadratic,
        quadratic,
    };
    let axial_start = origin_x.mul_add(direction[0], origin_y * direction[1]);
    let axial_end = cylinder_height.mul_add(direction_length, axial_start);
    let height = signed_cone_height.abs();
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(height).max(cylinder_radius))
        .max(coordinate_roundoff);
    if !fit_tolerance.is_finite() || !section.axial_amplitude.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "perpendicular cone/cylinder fit is ill-conditioned",
        });
    }
    let minimum_height = cylinder_radius / quadratic.sqrt();
    if height < minimum_height - fit_tolerance
        || axial_start > section.axial_amplitude + fit_tolerance
        || axial_end < -section.axial_amplitude - fit_tolerance
    {
        return Ok(Vec::new());
    }

    let mut cuts = vec![0.0, 0.5 * PI, PI, 1.5 * PI, TURN];
    for axial in [axial_start, axial_end] {
        if axial > -section.axial_amplitude && axial < section.axial_amplitude {
            let angle = (axial / section.axial_amplitude).acos();
            cuts.extend([angle, TURN - angle]);
        }
    }
    if height > minimum_height && height < cylinder_radius {
        let sine = ((1.0 - (height / cylinder_radius).powi(2)) / section.modulus)
            .clamp(0.0, 1.0)
            .sqrt();
        let angle = sine.asin();
        cuts.extend([angle, PI - angle, PI + angle, TURN - angle]);
    }
    cuts.sort_by(Real::total_cmp);
    cuts.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);

    let inside = |angle: Real| {
        let axial = section.axial_amplitude * angle.cos();
        axial >= axial_start && axial <= axial_end && section.height(angle) <= height
    };
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in cuts.windows(2) {
        if inside(0.5 * (pair[0] + pair[1])) {
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

    let mut events = Vec::new();
    for angle in cuts {
        let axial = section.axial_amplitude * angle.cos();
        let wall_height = section.height(angle);
        let on_rim = (axial - axial_start).abs() <= fit_tolerance
            || (axial - axial_end).abs() <= fit_tolerance
            || (wall_height - height).abs() <= fit_tolerance;
        let on_curve = [-TURN, 0.0, TURN].into_iter().any(|shift| {
            intervals.iter().any(|(start, end)| {
                angle + shift >= *start - 32.0 * Real::EPSILON
                    && angle + shift <= *end + 32.0 * Real::EPSILON
            })
        });
        if !on_rim
            || on_curve
            || axial < axial_start - fit_tolerance
            || axial > axial_end + fit_tolerance
            || wall_height > height + fit_tolerance
        {
            continue;
        }
        let point = section.sample(angle)?.0;
        if !events.iter().any(|event| {
            matches!(event, SurfaceSurfaceIntersectionEvent::Point(existing)
                if existing.distance_to(point).is_ok_and(|distance| distance <= fit_tolerance))
        }) {
            events.push(SurfaceSurfaceIntersectionEvent::Point(point));
        }
    }
    let derivative_bound = section.fourth_derivative_bound();
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "perpendicular cone/cylinder derivative bound is not finite",
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

impl Section {
    fn height(self, angle: Real) -> Real {
        let sine = angle.sin();
        self.radius * (1.0 - self.modulus * sine * sine).sqrt()
    }

    fn sample(self, angle: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = angle.sin_cos();
        let axial = self.axial_amplitude * cosine;
        let cross = self.cross_amplitude * sine;
        let height = self.height(angle);
        let height_derivative =
            -self.radius * self.modulus * sine * cosine / (1.0 - self.modulus * sine * sine).sqrt();
        let [ux, uy] = self.direction;
        let local = [
            ux.mul_add(axial, -uy * cross),
            uy.mul_add(axial, ux * cross),
            self.axial_sign * height,
        ];
        let axial_derivative = -self.axial_amplitude * sine;
        let cross_derivative = self.cross_amplitude * cosine;
        let derivative = [
            ux.mul_add(axial_derivative, -uy * cross_derivative),
            uy.mul_add(axial_derivative, ux * cross_derivative),
            self.axial_sign * height_derivative,
        ];
        let x = self.frame.x_axis().as_vector().to_array();
        let y = self.frame.y_axis().as_vector().to_array();
        let z = self.frame.z_axis().as_vector().to_array();
        let tangent = Vector3::try_from(std::array::from_fn(|i| {
            derivative[0].mul_add(x[i], derivative[1].mul_add(y[i], derivative[2] * z[i]))
        }))?;
        Ok((self.frame.point_at(local)?, tangent))
    }

    fn fourth_derivative_bound(self) -> Real {
        let lower = 1.0 / self.quadratic;
        let root = lower.sqrt();
        let lower3 = lower * root;
        let lower5 = lower * lower3;
        let lower7 = lower * lower5;
        let q1 = self.modulus;
        let q2 = 2.0 * q1;
        let q3 = 4.0 * q1;
        let q4 = 8.0 * q1;
        let height_fourth = self.radius
            * (q4 / (2.0 * root)
                + (4.0 * q1 * q3 + 3.0 * q2 * q2) / (4.0 * lower3)
                + 9.0 * q1 * q1 * q2 / (4.0 * lower5)
                + 15.0 * q1.powi(4) / (16.0 * lower7));
        self.axial_amplitude
            .hypot(self.cross_amplitude)
            .hypot(height_fourth)
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
    let required = ((end - start) / max_span).ceil();
    if !required.is_finite() || required > MAX_SEGMENTS as Real || max_span <= 0.0 {
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

    fn cone_frame() -> Frame3 {
        Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn cylinder(start: Real, end: Real) -> NurbsSurface {
        let frame = Frame3::try_from_normal(
            point(start, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        NurbsSurface::try_cylinder(frame, 2.0, 0.0, end - start).unwrap()
    }

    fn assert_on_walls(location: Point3, cone_height: Real, start: Real, end: Real) {
        let radial = location.x().hypot(location.y());
        assert!((radial - 0.75 * location.z().abs()).abs() < 5e-9);
        assert!((location.y().hypot(location.z()) - 2.0).abs() < 5e-9);
        assert!(location.z() * cone_height.signum() >= -5e-9);
        assert!(location.z().abs() <= cone_height.abs() + 5e-9);
        assert!((start - 5e-9..=end + 5e-9).contains(&location.x()));
    }

    #[test]
    fn perpendicular_cylinder_through_apex_has_one_closed_section() {
        let cone = NurbsSurface::try_cone(cone_frame(), 3.0, 4.0).unwrap();
        let cylinder = cylinder(-4.0, 4.0);
        for (first, second) in [(&cone, &cylinder), (&cylinder, &cone)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("expected one closed section, got {events:#?}")
            };
            assert_eq!(curve.degree(), 3);
            assert!(curve.is_closed().unwrap());
            for index in 0..=128 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 128.0);
                assert_on_walls(curve.evaluate(parameter).unwrap(), 4.0, -4.0, 4.0);
            }
        }
    }

    #[test]
    fn perpendicular_section_clips_to_both_rims_and_keeps_isolated_contacts() {
        let cone = NurbsSurface::try_cone(cone_frame(), 1.35, 1.8).unwrap();
        let finite_cylinder = cylinder(0.2, 1.2);
        let events =
            surface_surface_intersection_events(&cone, &finite_cylinder, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite rims should retain arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for index in 0..=64 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                assert_on_walls(curve.evaluate(parameter).unwrap(), 1.8, 0.2, 1.2);
            }
        }

        let tangent_cone = NurbsSurface::try_cone(cone_frame(), 1.2, 1.6).unwrap();
        let contacts = surface_surface_intersection_events(
            &tangent_cone,
            &cylinder(-4.0, 4.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(contacts.len(), 2);
        for event in contacts {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("cone base should touch the section at one point on each side")
            };
            assert_on_walls(location, 1.6, -4.0, 4.0);
            assert!(location.x().abs() < 5e-9);
        }
        let rim = surface_surface_intersection_events(
            &NurbsSurface::try_cone(cone_frame(), 3.0, 4.0).unwrap(),
            &cylinder(1.5, 3.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(location)] = rim.as_slice() else {
            panic!("cylinder end should touch one point, got {rim:#?}")
        };
        assert_on_walls(*location, 4.0, 1.5, 3.0);
        let disjoint_cone = NurbsSurface::try_cone(cone_frame(), 1.125, 1.5).unwrap();
        assert!(
            surface_surface_intersection_events(
                &disjoint_cone,
                &cylinder(-4.0, 4.0),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn perpendicular_section_handles_negative_and_distant_rotated_frames() {
        let negative = NurbsSurface::try_cone(cone_frame(), 3.0, -4.0).unwrap();
        let events = surface_surface_intersection_events(
            &negative,
            &cylinder(-4.0, 4.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("negative cone should retain a closed section")
        };
        assert!(curve.is_closed().unwrap());
        for index in 0..=32 {
            let domain = curve.domain();
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
            assert_on_walls(curve.evaluate(parameter).unwrap(), -4.0, -4.0, 4.0);
        }

        let frame = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder_frame = Frame3::try_from_normal(
            frame.point_at([-4.0, 0.0, 0.0]).unwrap(),
            frame.x_axis().as_vector(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let center = frame.origin();
        let cone = NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap();
        let cylinder = NurbsSurface::try_cylinder(cylinder_frame, 2.0, 0.0, 8.0).unwrap();
        let events =
            surface_surface_intersection_events(&cone, &cylinder, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("rotated cone and cylinder should retain one section")
        };
        assert!(curve.is_closed().unwrap());
        for index in 0..=64 {
            let domain = curve.domain();
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            let location = curve.evaluate(parameter).unwrap();
            let local = frame.coordinates_of(location).unwrap();
            assert!((local[0].hypot(local[1]) - 0.75 * local[2]).abs() < 4e-7);
            let relative = cylinder_frame.coordinates_of(location).unwrap();
            assert!((relative[0].hypot(relative[1]) - 2.0).abs() < 4e-7);
            assert!(location.distance_to(center).unwrap() < 4.0);
        }
    }
}
