//! Cubic sections of a ring torus by a finite axis-parallel offset plane.

use super::{SurfaceSurfaceIntersectionEvent, intersect_curve_with_planar_surface};
use crate::{Frame3, GeometryError, NurbsCurve, NurbsSurface, Plane, Real, Tolerance};

const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
enum Branch {
    InnerSide,
    OuterSide,
    Joined,
    PinchedPositive,
    PinchedNegative,
}

#[derive(Clone, Copy)]
struct Section {
    major: Real,
    minor: Real,
    offset: Real,
    outer_lateral: Real,
    branch: Branch,
}

#[derive(Clone, Copy)]
struct Sample {
    lateral: Real,
    vertical: Real,
    lateral_derivative: Real,
    vertical_derivative: Real,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn intersect(
    torus_frame: Frame3,
    major: Real,
    minor: Real,
    planar_surface: &NurbsSurface,
    plane: Plane,
    signed_distance: Real,
    tolerance: Tolerance,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let offset = signed_distance.abs();
    let outer = major + minor;
    if offset > outer + fit_tolerance {
        return Ok(Vec::new());
    }
    let normal = plane.normal().as_vector();
    let axis = torus_frame.z_axis().as_vector();
    let lateral = axis.cross(normal)?;
    let origin = torus_frame
        .origin()
        .translated(normal.scaled(-signed_distance)?)?;
    let section_frame = Frame3::try_from_directions(origin, lateral, axis, tolerance)?;
    if (offset - outer).abs() <= fit_tolerance {
        let point = section_frame.point_at([0.0, 0.0, 0.0])?;
        let (u, v) = planar_surface.closest_parameters(point, tolerance)?;
        if planar_surface.evaluate(u, v)?.distance_to(point)? <= fit_tolerance {
            return Ok(vec![SurfaceSurfaceIntersectionEvent::Point(point)]);
        }
        return Ok(Vec::new());
    }
    let critical = major - minor;
    let critical_difference = (offset - critical).abs();
    // Near the pinched section, the loop separation grows as sqrt(distance).
    // A plane offset by the ordinary modeling tolerance can therefore have a
    // visibly different topology. Only merge the branches when that separation
    // itself is below the requested spatial tolerance.
    let pinched_tolerance = fit_tolerance * fit_tolerance / (2.0 * outer);
    let outer_lateral = ((outer - offset) * (outer + offset)).sqrt();
    let branches: &[Branch] = if critical_difference <= pinched_tolerance {
        &[Branch::PinchedPositive, Branch::PinchedNegative]
    } else if offset < critical {
        &[Branch::OuterSide, Branch::InnerSide]
    } else {
        &[Branch::Joined]
    };
    let mut events = Vec::new();
    for &branch in branches {
        let section = Section {
            major,
            minor,
            offset,
            outer_lateral,
            branch,
        };
        let curve = fit(section, section_frame, fit_tolerance)?;
        events.extend(intersect_curve_with_planar_surface(
            &curve,
            planar_surface,
            tolerance,
        )?);
    }
    Ok(events)
}

impl Section {
    fn sample(self, angle: Real) -> Sample {
        let (sine, cosine) = angle.sin_cos();
        match self.branch {
            Branch::OuterSide | Branch::InnerSide => {
                let radius = self.major + self.minor * cosine;
                let lateral_absolute = ((radius - self.offset) * (radius + self.offset)).sqrt();
                let sign = if matches!(self.branch, Branch::OuterSide) {
                    1.0
                } else {
                    -1.0
                };
                Sample {
                    lateral: sign * lateral_absolute,
                    vertical: self.minor * sine,
                    lateral_derivative: -sign * self.minor * radius * sine / lateral_absolute,
                    vertical_derivative: self.minor * cosine,
                }
            }
            Branch::Joined => {
                let lateral = self.outer_lateral * cosine;
                let lateral_derivative = -self.outer_lateral * sine;
                let radial = self.offset.hypot(lateral);
                let denominator = self.major + self.minor + radial;
                let numerator = radial - (self.major - self.minor);
                let root = (numerator / denominator).sqrt();
                let radial_derivative = lateral * lateral_derivative / radial;
                let root_derivative =
                    self.major * radial_derivative / (root * denominator * denominator);
                Sample {
                    lateral,
                    vertical: self.outer_lateral * sine * root,
                    lateral_derivative,
                    vertical_derivative: self.outer_lateral
                        * (cosine * root + sine * root_derivative),
                }
            }
            Branch::PinchedPositive | Branch::PinchedNegative => {
                let meridian = angle - std::f64::consts::PI;
                let (meridian_sine, meridian_cosine) = meridian.sin_cos();
                let half_sine = (0.5 * meridian).sin();
                let half_cosine = (0.5 * meridian).cos();
                let critical_offset = self.major - self.minor;
                let radius = self.major + self.minor * meridian_cosine;
                let second_root = (radius + critical_offset).sqrt();
                let scale = (2.0 * self.minor).sqrt();
                let sign = if matches!(self.branch, Branch::PinchedPositive) {
                    1.0
                } else {
                    -1.0
                };
                let radial_derivative = -self.minor * meridian_sine;
                Sample {
                    lateral: sign * scale * half_cosine * second_root,
                    vertical: self.minor * meridian_sine,
                    lateral_derivative: sign
                        * scale
                        * (-0.5 * half_sine * second_root
                            + half_cosine * radial_derivative / (2.0 * second_root)),
                    vertical_derivative: self.minor * meridian_cosine,
                }
            }
        }
    }
}

fn fit(section: Section, frame: Frame3, fit_tolerance: Real) -> Result<NurbsCurve, GeometryError> {
    let mut segments = Vec::new();
    for index in 0..8 {
        fit_interval(
            section,
            TURN * index as Real / 8.0,
            TURN * (index + 1) as Real / 8.0,
            fit_tolerance,
            0,
            &mut segments,
        )?;
    }
    let first = section.sample(0.0);
    let first_point = frame.point_at([first.lateral, first.vertical, 0.0])?;
    let mut controls = Vec::with_capacity(3 * segments.len() + 1);
    let mut knots = Vec::with_capacity(3 * segments.len() + 5);
    controls.push(first_point);
    knots.extend([0.0; 4]);
    let count = segments.len();
    for (index, (end, control)) in segments.into_iter().enumerate() {
        controls.push(frame.point_at([control[1][0], control[1][1], 0.0])?);
        controls.push(frame.point_at([control[2][0], control[2][1], 0.0])?);
        controls.push(if index + 1 == count {
            first_point
        } else {
            frame.point_at([control[3][0], control[3][1], 0.0])?
        });
        knots.extend([end; 3]);
    }
    knots.push(TURN);
    NurbsCurve::try_new(3, controls, knots)
}

fn fit_interval(
    section: Section,
    start: Real,
    end: Real,
    fit_tolerance: Real,
    depth: usize,
    segments: &mut Vec<(Real, [[Real; 2]; 4])>,
) -> Result<(), GeometryError> {
    let first = section.sample(start);
    let last = section.sample(end);
    let handle = (end - start) / 3.0;
    let control = [
        [first.lateral, first.vertical],
        [
            first.lateral + handle * first.lateral_derivative,
            first.vertical + handle * first.vertical_derivative,
        ],
        [
            last.lateral - handle * last.lateral_derivative,
            last.vertical - handle * last.vertical_derivative,
        ],
        [last.lateral, last.vertical],
    ];
    let accurate = [0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875]
        .into_iter()
        .all(|fraction| {
            let expected = section.sample(start + (end - start) * fraction);
            let actual = bezier(control, fraction);
            (actual[0] - expected.lateral).hypot(actual[1] - expected.vertical)
                <= fit_tolerance * 0.25
        });
    if accurate {
        if segments.len() >= MAX_SEGMENTS {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: 3 * MAX_SEGMENTS + 1,
            });
        }
        segments.push((end, control));
        return Ok(());
    }
    if depth >= 32 || segments.len() >= MAX_SEGMENTS || end - start <= 16.0 * Real::EPSILON {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let middle = 0.5 * (start + end);
    fit_interval(section, start, middle, fit_tolerance, depth + 1, segments)?;
    fit_interval(section, middle, end, fit_tolerance, depth + 1, segments)?;
    Ok(())
}

fn bezier(control: [[Real; 2]; 4], fraction: Real) -> [Real; 2] {
    let complement = 1.0 - fraction;
    let basis = [
        complement.powi(3),
        3.0 * complement * complement * fraction,
        3.0 * complement * fraction * fraction,
        fraction.powi(3),
    ];
    [
        basis
            .iter()
            .zip(control)
            .map(|(b, point)| b * point[0])
            .sum(),
        basis
            .iter()
            .zip(control)
            .map(|(b, point)| b * point[1])
            .sum(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point3, Vector3, surface_surface_intersection_events};

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

    fn plane(offset: Real, low: Real, high: Real) -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(offset, low, -2.0),
            point(offset, high, -2.0),
            point(offset, high, 2.0),
            point(offset, low, 2.0),
        ])
        .unwrap()
    }

    #[test]
    fn parallel_offset_plane_sections_have_two_or_one_smooth_loops() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        for (offset, expected) in [(1.0, 2), (3.5, 1), (-3.5, 1), (5.5, 0)] {
            let patch = plane(offset, -6.0, 6.0);
            for (left, right) in [(&torus, &patch), (&patch, &torus)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), expected);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("parallel offset torus section should be a curve")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    for index in 0..=64 {
                        let domain = curve.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        let location = curve.evaluate(parameter).unwrap();
                        let radial = location.x().hypot(location.y());
                        assert!((location.x() - offset).abs() < 5e-9);
                        assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn parallel_offset_plane_clips_loops_and_keeps_outer_tangent_point() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        let clipped =
            surface_surface_intersection_events(&torus, &plane(1.0, 3.5, 6.0), Tolerance::DEFAULT)
                .unwrap();
        assert!(!clipped.is_empty());
        for event in clipped {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite offset plane should retain arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for index in 0..=16 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.y() >= 3.5 - 5e-9);
            }
        }
        let tangent =
            surface_surface_intersection_events(&torus, &plane(5.0, -6.0, 6.0), Tolerance::DEFAULT)
                .unwrap();
        assert!(matches!(
            tangent.as_slice(),
            [SurfaceSurfaceIntersectionEvent::Point(_)]
        ));
        let pinched =
            surface_surface_intersection_events(&torus, &plane(3.0, -6.0, 6.0), Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(pinched.len(), 2);
        for event in pinched {
            let SurfaceSurfaceIntersectionEvent::Curve(loop_curve) = event else {
                panic!("inner tangent plane should retain both pinched loops")
            };
            assert!(loop_curve.is_closed().unwrap());
            let pinch = loop_curve.evaluate(*loop_curve.domain().start()).unwrap();
            assert!(pinch.distance_to(point(3.0, 0.0, 0.0)).unwrap() < 5e-9);
            for index in 0..=64 {
                let domain = loop_curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                let location = loop_curve.evaluate(parameter).unwrap();
                let radial = location.x().hypot(location.y());
                assert!((location.x() - 3.0).abs() < 5e-9);
                assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
            }
        }
        for (offset, expected) in [(3.0 - 5.0e-10, 2), (3.0 + 5.0e-10, 1)] {
            let events = surface_surface_intersection_events(
                &torus,
                &plane(offset, -6.0, 6.0),
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(events.len(), expected);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("near-pinched section should retain its loop topology")
                };
                assert!(curve.is_closed().unwrap());
                for index in 0..=128 {
                    let parameter = TURN * index as Real / 128.0;
                    let location = curve.evaluate(parameter).unwrap();
                    let radial = location.x().hypot(location.y());
                    assert!((location.x() - offset).abs() < 5e-9);
                    assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                }
            }
        }
    }

    #[test]
    fn parallel_offset_plane_handles_distant_rotated_coordinates() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        for offset in [1.0, 3.0] {
            let corner = |y: Real, z: Real| rotated.point_at([offset, y, z]).unwrap();
            let patch = NurbsSurface::try_bilinear([
                corner(-6.0, -2.0),
                corner(6.0, -2.0),
                corner(6.0, 2.0),
                corner(-6.0, 2.0),
            ])
            .unwrap();
            let events =
                surface_surface_intersection_events(&torus, &patch, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("rotated parallel offset section should be a loop")
                };
                for index in 0..=32 {
                    let domain = curve.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                    let local = rotated
                        .coordinates_of(curve.evaluate(parameter).unwrap())
                        .unwrap();
                    assert!((local[0] - offset).abs() < 4e-7);
                    assert!(((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                }
            }
        }
    }

    #[test]
    fn fitted_sections_respect_dense_meridian_samples() {
        for (offset, branch) in [
            (2.75, Branch::OuterSide),
            (2.75, Branch::InnerSide),
            (3.25, Branch::Joined),
            (4.75, Branch::Joined),
            (3.0, Branch::PinchedPositive),
            (3.0, Branch::PinchedNegative),
            (3.0 - 5.0e-10, Branch::OuterSide),
            (3.0 + 5.0e-10, Branch::Joined),
        ] {
            let section = Section {
                major: 4.0,
                minor: 1.0,
                offset,
                outer_lateral: ((5.0 - offset) * (5.0 + offset)).sqrt(),
                branch,
            };
            let curve = fit(section, frame(), 1.0e-9).unwrap();
            if matches!(branch, Branch::PinchedPositive | Branch::PinchedNegative) {
                let (_, start_tangent) = curve.evaluate_with_derivative(0.0).unwrap();
                let (_, end_tangent) = curve.evaluate_with_derivative(TURN).unwrap();
                assert!(start_tangent.x() * end_tangent.x() < 0.0);
            }
            for index in 0..=1024 {
                let angle = TURN * index as Real / 1024.0;
                let expected = section.sample(angle);
                let actual = curve.evaluate(angle).unwrap();
                assert!(
                    (actual.x() - expected.lateral).hypot(actual.y() - expected.vertical) < 1.0e-9
                );
            }
        }
    }
}
