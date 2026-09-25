//! Exact finite generators shared by two cones with different axis directions.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Real, Tolerance, Vector3};

pub(super) fn intersect(
    (first_frame, first_radius, first_signed_height): (Frame3, Real, Real),
    (second_frame, second_radius, second_signed_height): (Frame3, Real, Real),
    tolerance: Tolerance,
    spatial_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first_axis = first_frame
        .z_axis()
        .as_vector()
        .scaled(first_signed_height.signum())?;
    let second_axis = second_frame
        .z_axis()
        .as_vector()
        .scaled(second_signed_height.signum())?;
    let first_height = first_signed_height.abs();
    let second_height = second_signed_height.abs();
    let first_slope = first_radius / first_height;
    let second_slope = second_radius / second_height;
    let first_cosine = 1.0 / first_slope.hypot(1.0);
    let second_cosine = 1.0 / second_slope.hypot(1.0);
    let axis_cosine = first_axis.dot(second_axis)?;
    let transverse_axis = first_axis.cross(second_axis)?;
    let axis_sine = transverse_axis.length()?;
    if axis_sine <= tolerance.angular() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "shared-apex cone axes are ill-conditioned",
        });
    }
    let sideways = transverse_axis.scaled(1.0 / axis_sine)?;
    let in_plane = sideways.cross(first_axis)?;
    // A unit generator v must satisfy v·axis_i = cos(half_angle_i).
    // Solve in the orthonormal basis (first_axis, in_plane, sideways).
    let in_plane_component = (-axis_cosine).mul_add(first_cosine, second_cosine) / axis_sine;
    let radial_component_squared =
        (first_slope * first_cosine).powi(2) - in_plane_component * in_plane_component;
    let numerical_roundoff = 64.0
        * Real::EPSILON
        * (1.0 + first_cosine.abs() + second_cosine.abs() + in_plane_component.abs());
    if radial_component_squared < -numerical_roundoff {
        return Ok(Vec::new());
    }
    let radial_component = radial_component_squared.max(0.0).sqrt();
    let maximum_length = first_height
        .hypot(first_radius)
        .min(second_height.hypot(second_radius));
    if maximum_length <= spatial_tolerance {
        return Ok(Vec::new());
    }
    let signs: &[Real] = if radial_component_squared.abs() <= numerical_roundoff {
        &[0.0]
    } else {
        &[1.0, -1.0]
    };
    let x = first_axis.to_array();
    let y = in_plane.to_array();
    let z = sideways.to_array();
    let mut events = Vec::with_capacity(signs.len());
    for sign in signs {
        let direction = Vector3::try_from(std::array::from_fn(|index| {
            first_cosine.mul_add(
                x[index],
                in_plane_component.mul_add(y[index], sign * radial_component * z[index]),
            )
        }))?
        .normalized_nonzero()?
        .as_vector();
        let endpoint = first_frame
            .origin()
            .translated(direction.scaled(maximum_length)?)?;
        let length = first_frame.origin().distance_to(endpoint)?;
        if length <= spatial_tolerance {
            continue;
        }
        let line = NurbsCurve::try_new(
            1,
            vec![first_frame.origin(), endpoint],
            vec![0.0, 0.0, length, length],
        )?;
        events.push(SurfaceSurfaceIntersectionEvent::Curve(line));
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, Point3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame(origin: Point3, axis: [Real; 3]) -> Frame3 {
        Frame3::try_from_normal(origin, Vector3::try_from(axis).unwrap(), Tolerance::DEFAULT)
            .unwrap()
    }

    fn cone(frame: Frame3, radius: Real, height: Real) -> NurbsSurface {
        NurbsSurface::try_cone(frame, radius, height).unwrap()
    }

    #[test]
    fn crossing_cones_with_shared_apex_have_two_exact_finite_generators() {
        let first = cone(frame(point(0.0, 0.0, 0.0), [0.0, 0.0, 1.0]), 8.0, 4.0);
        let second = cone(frame(point(0.0, 0.0, 0.0), [1.0, 0.0, 0.0]), 6.0, 3.0);
        for (left, right) in [(&first, &second), (&second, &first)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            let mut signs = Vec::new();
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                    panic!("shared-apex cones should produce generator lines")
                };
                assert_eq!(line.degree(), 1);
                let start = line.evaluate(*line.domain().start()).unwrap();
                let end = line.evaluate(*line.domain().end()).unwrap();
                assert!(start.distance_to(point(0.0, 0.0, 0.0)).unwrap() < 5e-9);
                assert!((end.x() - 3.0).abs() < 5e-9);
                assert!((end.z() - 3.0).abs() < 5e-9);
                assert!((end.y().abs() - 3.0 * 3.0_f64.sqrt()).abs() < 5e-9);
                signs.push(end.y().signum());
                for index in 0..=16 {
                    let parameter = *line.domain().start()
                        + (*line.domain().end() - *line.domain().start()) * (index as Real / 16.0);
                    let location = line.evaluate(parameter).unwrap();
                    assert!((location.x().hypot(location.y()) - 2.0 * location.z()).abs() < 5e-9);
                    assert!((location.y().hypot(location.z()) - 2.0 * location.x()).abs() < 5e-9);
                }
            }
            assert_eq!(signs.iter().sum::<Real>(), 0.0);
        }
    }

    #[test]
    fn shared_apex_cones_keep_one_tangent_generator_and_omit_apex_only_contact() {
        let origin = point(0.0, 0.0, 0.0);
        let first = cone(frame(origin, [0.0, 0.0, 1.0]), 4.0, 4.0);
        let tangent = cone(frame(origin, [1.0, 0.0, 0.0]), 3.0, 3.0);
        let events =
            surface_surface_intersection_events(&first, &tangent, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(line)] = events.as_slice() else {
            panic!("tangent cones should share one generator, got {events:#?}")
        };
        assert_eq!(line.degree(), 1);
        assert!(
            line.evaluate(*line.domain().end())
                .unwrap()
                .distance_to(point(3.0, 0.0, 3.0))
                .unwrap()
                < 5e-9
        );

        let narrow_first = cone(frame(origin, [0.0, 0.0, 1.0]), 2.0, 4.0);
        let narrow_second = cone(frame(origin, [1.0, 0.0, 0.0]), 1.5, 3.0);
        assert!(
            surface_surface_intersection_events(&narrow_first, &narrow_second, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn shared_apex_generators_support_negative_heights_and_distant_rotated_frames() {
        let origin = point(0.0, 0.0, 0.0);
        let negative_first = cone(frame(origin, [0.0, 0.0, 1.0]), 8.0, -4.0);
        let negative_second = cone(frame(origin, [1.0, 0.0, 0.0]), 6.0, -3.0);
        let events = surface_surface_intersection_events(
            &negative_first,
            &negative_second,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                unreachable!()
            };
            let end = line.evaluate(*line.domain().end()).unwrap();
            assert!((end.x() + 3.0).abs() < 5e-9);
            assert!((end.z() + 3.0).abs() < 5e-9);
        }

        let rotated = frame(point(1.0e8, -1.0e8, 1.0e8), [1.0, 2.0, 3.0]);
        let crossed = frame(rotated.origin(), rotated.x_axis().as_vector().to_array());
        let first = cone(rotated, 8.0, 4.0);
        let second = cone(crossed, 6.0, 3.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                unreachable!()
            };
            let end = line.evaluate(*line.domain().end()).unwrap();
            let first_local = rotated.coordinates_of(end).unwrap();
            let second_local = crossed.coordinates_of(end).unwrap();
            assert!((first_local[0].hypot(first_local[1]) - 2.0 * first_local[2]).abs() < 4e-7);
            assert!((second_local[0].hypot(second_local[1]) - 2.0 * second_local[2]).abs() < 4e-7);
        }
    }

    #[test]
    fn oblique_shared_apex_generators_stay_on_both_finite_cones() {
        let origin = point(0.0, 0.0, 0.0);
        let first_frame = frame(origin, [0.0, 0.0, 1.0]);
        for second_axis in [[1.0, 0.0, 1.0], [1.0, 2.0, 3.0], [-1.0, 1.0, 0.5]] {
            let second_frame = frame(origin, second_axis);
            for (first_slope, second_slope) in
                [(0.5, 0.5), (1.0, 1.0), (2.0, 2.0), (1.5, 0.75), (0.75, 1.5)]
            {
                let first = cone(first_frame, 4.0 * first_slope, 4.0);
                let second = cone(second_frame, 3.0 * second_slope, 3.0);
                let events =
                    surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT)
                        .unwrap();
                assert!(events.len() <= 2);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                        panic!("shared apex can only produce generator curves")
                    };
                    assert_eq!(line.degree(), 1);
                    for index in 0..=16 {
                        let domain = line.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                        let location = line.evaluate(parameter).unwrap();
                        let first_local = first_frame.coordinates_of(location).unwrap();
                        let second_local = second_frame.coordinates_of(location).unwrap();
                        assert!(
                            (first_local[0].hypot(first_local[1]) - first_slope * first_local[2])
                                .abs()
                                < 5e-8
                        );
                        assert!(
                            (second_local[0].hypot(second_local[1])
                                - second_slope * second_local[2])
                                .abs()
                                < 5e-8
                        );
                        assert!((-5e-8..=4.0 + 5e-8).contains(&first_local[2]));
                        assert!((-5e-8..=3.0 + 5e-8).contains(&second_local[2]));
                    }
                }
            }
        }
    }
}
