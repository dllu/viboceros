//! Exact finite planar sections of a canonical rational cone.

use super::*;

pub(super) fn cone_planar_surface_intersection_events(
    cone: &NurbsSurface,
    frame: crate::Frame3,
    radius: Real,
    height: Real,
    planar_surface: &NurbsSurface,
    plane: Plane,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let apex = frame.origin();
    let axis = frame.z_axis().as_vector();
    let normal = plane.normal().as_vector();
    let axial_dot = axis.dot(normal)?;
    let distance = plane.signed_distance_to(apex)?;
    let distance_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * radius.max(height.abs()));

    if axis.cross(normal)?.length()? <= tolerance.angular() {
        let axial_position = -distance / axial_dot;
        if axial_position < height.min(0.0) - distance_tolerance
            || axial_position > height.max(0.0) + distance_tolerance
        {
            return Ok(Vec::new());
        }
        if axial_position.abs() <= distance_tolerance {
            return Ok(Vec::new());
        }
        let circle = Circle3::try_new(
            frame.point_at([0.0, 0.0, axial_position])?,
            radius * (axial_position / height).abs(),
            plane.normal(),
            tolerance,
        )?
        .to_nurbs()?;
        return intersect_curve_with_planar_surface(&circle, planar_surface, tolerance);
    }

    if distance.abs() <= distance_tolerance {
        return apex_plane_sections(
            frame,
            radius,
            height,
            planar_surface,
            normal,
            axial_dot,
            tolerance,
        );
    }

    if axial_dot.abs() <= tolerance.angular() {
        let distance_from_axis = distance.abs();
        if distance_from_axis > radius + distance_tolerance {
            return Ok(Vec::new());
        }
        if (distance_from_axis - radius).abs() <= distance_tolerance {
            let contact = frame
                .point_at([0.0, 0.0, height])?
                .translated(normal.scaled(-distance)?)?;
            let (u, v) = planar_surface.closest_parameters(contact, tolerance)?;
            return Ok(
                if planar_surface.evaluate(u, v)?.distance_to(contact)? <= distance_tolerance {
                    vec![SurfaceSurfaceIntersectionEvent::Point(contact)]
                } else {
                    Vec::new()
                },
            );
        }
        let center = apex.translated(normal.scaled(-distance)?)?;
        let transverse = axis.cross(normal)?.normalized_nonzero()?.as_vector();
        let branch_frame = crate::Frame3::try_from_directions(
            center,
            axis.scaled(height.signum())?,
            transverse,
            tolerance,
        )?;
        let curve = NurbsCurve::try_hyperbola(
            branch_frame,
            distance_from_axis * height.abs() / radius,
            distance_from_axis,
            height.abs(),
        )?;
        let domain = curve.domain();
        let middle = 0.5 * (*domain.start() + *domain.end());
        let mut result = Vec::new();
        for half in [
            curve.try_trimmed(middle..=*domain.end())?,
            curve.try_trimmed(*domain.start()..=middle)?,
        ] {
            result.extend(intersect_curve_with_planar_surface(
                &half,
                planar_surface,
                tolerance,
            )?);
        }
        return Ok(result);
    }

    let base_row = usize::from(height > 0.0);
    let base_controls = &cone.control_points()[base_row * 9..base_row * 9 + 9];
    let denominators = base_controls
        .iter()
        .map(|control| normal.dot(apex.vector_to(control.point())?))
        .collect::<Result<Vec<_>, _>>()?;
    if denominators
        .iter()
        .any(|value| value.abs() <= distance_tolerance)
        || denominators
            .iter()
            .any(|value| value.signum() != denominators[0].signum())
    {
        return pole_crossing_cone_sections(
            frame,
            radius,
            height,
            planar_surface,
            plane,
            distance,
            tolerance,
            distance_tolerance,
        );
    }
    let weight_sign = denominators[0].signum();
    let controls = base_controls
        .iter()
        .zip(denominators)
        .map(|(control, denominator)| {
            WeightedPoint3::try_new(
                apex.translated(
                    apex.vector_to(control.point())?
                        .scaled(-distance / denominator)?,
                )?,
                control.weight() * denominator * weight_sign,
            )
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let conic = NurbsCurve::try_new_rational(2, controls, cone.knots_u().to_vec())?;
    let inside_rims = conic
        .control_points()
        .iter()
        .try_fold(true, |inside, control| {
            let axial = apex.vector_to(control.point())?.dot(axis)?;
            Ok::<bool, GeometryError>(
                inside && axial >= height.min(0.0) && axial <= height.max(0.0),
            )
        })?;
    if inside_rims {
        return intersect_curve_with_planar_surface(&conic, planar_surface, tolerance);
    }
    let mut result = Vec::new();
    for event in curve_surface_intersection_events(&conic, cone, tolerance)? {
        if let CurveSurfaceIntersectionEvent::Overlap(overlap) = event {
            result.extend(intersect_curve_with_planar_surface(
                &conic.try_trimmed(overlap.curve_interval())?,
                planar_surface,
                tolerance,
            )?);
        }
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn pole_crossing_cone_sections(
    frame: crate::Frame3,
    radius: Real,
    height: Real,
    planar_surface: &NurbsSurface,
    plane: Plane,
    apex_distance: Real,
    tolerance: Tolerance,
    distance_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let axis = frame.z_axis().as_vector();
    let normal = plane.normal().as_vector();
    let nx = normal.dot(frame.x_axis().as_vector())?;
    let ny = normal.dot(frame.y_axis().as_vector())?;
    let nz = normal.dot(axis)?;
    let amplitude = radius * nx.hypot(ny);
    let target = -(apex_distance + height * nz) / amplitude;
    if !target.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "cone/plane projective section has non-finite base-rim roots",
        });
    }
    if target.abs() >= 1.0 {
        if apex_distance * (apex_distance + height * nz) >= 0.0 {
            return Ok(Vec::new());
        }
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "cone/plane projective section lacks two base-rim crossings",
        });
    }
    let phi = ny.atan2(nx);
    let alpha = target.acos();
    let mut angles = [
        (phi - alpha).rem_euclid(std::f64::consts::TAU),
        (phi + alpha).rem_euclid(std::f64::consts::TAU),
    ];
    angles.sort_by(Real::total_cmp);
    let base_center = frame.point_at([0.0, 0.0, height])?;
    let base_circle = Circle3::try_from_frame(
        base_center,
        radius,
        frame.x_axis(),
        frame.z_axis(),
        tolerance,
    )?;
    let base_curve = base_circle.to_nurbs()?;
    let domain = base_curve.domain();
    let mut parameters = vec![*domain.start()];
    for angle in angles {
        let point = base_circle.point_at_angle(angle)?;
        parameters.push(base_curve.closest_parameter(point, tolerance)?);
    }
    parameters.push(*domain.end());
    parameters.sort_by(Real::total_cmp);
    let mut result = Vec::new();
    for interval in parameters.windows(2) {
        if interval[1] - interval[0] <= Real::EPSILON * domain.end().abs().max(1.0) * 256.0 {
            continue;
        }
        let middle = 0.5 * (interval[0] + interval[1]);
        let base_point = base_curve.evaluate(middle)?;
        if apex_distance * plane.signed_distance_to(base_point)? >= 0.0 {
            continue;
        }
        let arc = base_curve.try_trimmed(interval[0]..=interval[1])?;
        let mut denominators = Vec::with_capacity(arc.control_points().len());
        for control in arc.control_points() {
            denominators.push(normal.dot(frame.origin().vector_to(control.point())?)?);
        }
        if denominators.iter().any(|value| {
            value.abs() <= distance_tolerance || value.signum() == apex_distance.signum()
        }) {
            return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "finite cone section has a projective control at infinity",
            });
        }
        let controls = arc
            .control_points()
            .iter()
            .zip(denominators)
            .map(|(control, denominator)| {
                WeightedPoint3::try_new(
                    frame.origin().translated(
                        frame
                            .origin()
                            .vector_to(control.point())?
                            .scaled(-apex_distance / denominator)?,
                    )?,
                    control.weight() * denominator * -apex_distance.signum(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let conic = NurbsCurve::try_new_rational(arc.degree(), controls, arc.knots().to_vec())?;
        result.extend(intersect_curve_with_planar_surface(
            &conic,
            planar_surface,
            tolerance,
        )?);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn apex_plane_sections(
    frame: crate::Frame3,
    radius: Real,
    height: Real,
    planar_surface: &NurbsSurface,
    normal: crate::Vector3,
    axial_dot: Real,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let apex = frame.origin();
    let axis = frame.z_axis().as_vector();
    let axis_array = axis.to_array();
    let normal_array = normal.to_array();
    let transverse = crate::Vector3::try_from(std::array::from_fn(|i| {
        axial_dot.mul_add(-axis_array[i], normal_array[i])
    }))?;
    let transverse_length = transverse.length()?;
    if transverse_length <= tolerance.angular() {
        return Ok(Vec::new());
    }
    let cosine = -height * axial_dot / (radius * transverse_length);
    if cosine.abs() > 1.0 + tolerance.angular() {
        return Ok(Vec::new());
    }
    let cosine = cosine.clamp(-1.0, 1.0);
    let sine = (1.0 - cosine * cosine).max(0.0).sqrt();
    let e = transverse.normalized_nonzero()?.as_vector().to_array();
    let t = axis
        .cross(transverse)?
        .normalized_nonzero()?
        .as_vector()
        .to_array();
    let signs: &[Real] = if sine <= tolerance.angular() {
        // Rhino returns both coincident cone branches at a tangent generator.
        &[0.0, 0.0]
    } else {
        &[-1.0, 1.0]
    };
    let base_center = frame.point_at([0.0, 0.0, height])?;
    let mut result = Vec::new();
    for sign in signs {
        let radial = crate::Vector3::try_from(std::array::from_fn(|i| {
            radius * cosine.mul_add(e[i], sign * sine * t[i])
        }))?;
        let endpoint = base_center.translated(radial)?;
        let length = apex.distance_to(endpoint)?;
        let line = NurbsCurve::try_new(1, vec![apex, endpoint], vec![0.0, 0.0, length, length])?;
        result.extend(intersect_curve_with_planar_surface(
            &line,
            planar_surface,
            tolerance,
        )?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn patch(corners: [[Real; 3]; 4]) -> NurbsSurface {
        NurbsSurface::try_bilinear(corners.map(|[x, y, z]| point(x, y, z))).unwrap()
    }

    #[test]
    fn cone_sections_match_rhino_circle_and_generator_lengths() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
        let horizontal = |z| {
            patch([
                [-3.0, -3.0, z],
                [3.0, -3.0, z],
                [3.0, 3.0, z],
                [-3.0, 3.0, z],
            ])
        };
        let circular =
            surface_surface_intersection_events(&cone, &horizontal(1.5), Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = circular.as_slice() else {
            panic!("expected one cone section circle, got {circular:#?}")
        };
        assert!(circle.is_closed().unwrap());
        assert!((circle.length(Tolerance::DEFAULT).unwrap() - std::f64::consts::TAU).abs() < 1e-7);
        assert!(
            surface_surface_intersection_events(&cone, &horizontal(0.0), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );

        let axial = patch([
            [0.0, -3.0, -1.0],
            [0.0, 3.0, -1.0],
            [0.0, 3.0, 4.0],
            [0.0, -3.0, 4.0],
        ]);
        let generators =
            surface_surface_intersection_events(&cone, &axial, Tolerance::DEFAULT).unwrap();
        assert_eq!(generators.len(), 2);
        for event in generators {
            let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                panic!("expected a cone generator line")
            };
            assert_eq!(line.degree(), 1);
            assert!((line.length(Tolerance::DEFAULT).unwrap() - 13.0_f64.sqrt()).abs() < 1e-9);
        }
        let tangent = patch([
            [-3.0, -3.0, -4.5],
            [3.0, -3.0, 4.5],
            [3.0, 3.0, 4.5],
            [-3.0, 3.0, -4.5],
        ]);
        let coincident =
            surface_surface_intersection_events(&cone, &tangent, Tolerance::DEFAULT).unwrap();
        assert_eq!(coincident.len(), 2);
        assert_eq!(coincident[0], coincident[1]);
    }

    #[test]
    fn cone_oblique_section_is_an_exact_finite_conic() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
        let tilted = patch([
            [-3.0, -3.0, 0.5],
            [3.0, -3.0, 2.5],
            [3.0, 3.0, 2.5],
            [-3.0, 3.0, 0.5],
        ]);
        let events =
            surface_surface_intersection_events(&cone, &tilted, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(conic)] = events.as_slice() else {
            panic!("expected one oblique cone section, got {events:#?}")
        };
        assert_eq!(conic.degree(), 2);
        assert!(conic.is_closed().unwrap());
        assert!((conic.length(Tolerance::DEFAULT).unwrap() - 6.708268235).abs() < 1e-5);
        let plane = tilted.plane(Tolerance::DEFAULT).unwrap().unwrap();
        for control in conic.control_points() {
            assert!(plane.signed_distance_to(control.point()).unwrap().abs() < 1e-9);
        }

        let steep = patch([
            [-4.0, -4.0, -2.5],
            [4.0, -4.0, 5.5],
            [4.0, 4.0, 5.5],
            [-4.0, 4.0, -2.5],
        ]);
        let clipped =
            surface_surface_intersection_events(&cone, &steep, Tolerance::DEFAULT).unwrap();
        assert_eq!(clipped.len(), 1);
        for event in clipped {
            let SurfaceSurfaceIntersectionEvent::Curve(arc) = event else {
                panic!("expected an exact clipped conic arc")
            };
            assert!(!arc.is_closed().unwrap());
            let length = arc.length(Tolerance::DEFAULT).unwrap();
            assert!((length - 7.103006259694).abs() < 1e-8);
            // Rhino's fitted cubic section is about 1.1e-5 longer.
            assert!((length - 7.103017334).abs() < 2e-5);
            for parameter in [*arc.domain().start(), *arc.domain().end()] {
                let point = arc.evaluate(parameter).unwrap();
                assert!(point.z() >= -1e-8 && point.z() <= 3.0 + 1e-8);
                assert!((point.x().hypot(point.y()) - point.z() * (2.0 / 3.0)).abs() < 1e-8);
            }
        }

        let through_apex = patch([
            [-3.0, -3.0, -6.0],
            [3.0, -3.0, 6.0],
            [3.0, 3.0, 6.0],
            [-3.0, 3.0, -6.0],
        ]);
        let generators =
            surface_surface_intersection_events(&cone, &through_apex, Tolerance::DEFAULT).unwrap();
        assert_eq!(generators.len(), 2);
        for event in generators {
            let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                panic!("expected an oblique apex generator")
            };
            assert!((line.length(Tolerance::DEFAULT).unwrap() - 13.0_f64.sqrt()).abs() < 1e-9);
        }
    }

    #[test]
    fn cone_sections_handle_rotated_negative_height() {
        let frame = crate::Frame3::try_from_normal(
            point(1.0, 2.0, 3.0),
            crate::Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(frame, 2.0, -3.0).unwrap();
        let planar = NurbsSurface::try_bilinear([
            frame.point_at([-3.0, -3.0, -2.5]).unwrap(),
            frame.point_at([3.0, -3.0, -0.5]).unwrap(),
            frame.point_at([3.0, 3.0, -0.5]).unwrap(),
            frame.point_at([-3.0, 3.0, -2.5]).unwrap(),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&cone, &planar, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(conic)] = events.as_slice() else {
            panic!("expected a rotated cone conic, got {events:#?}")
        };
        assert!(conic.is_closed().unwrap());
        assert_eq!(conic.degree(), 2);
        let plane = planar.plane(Tolerance::DEFAULT).unwrap().unwrap();
        for control in conic.control_points() {
            assert!(plane.signed_distance_to(control.point()).unwrap().abs() < 1e-8);
        }
    }

    #[test]
    fn cone_parallel_axis_section_is_an_exact_hyperbola_branch() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
        let vertical = patch([
            [1.0, -3.0, -1.0],
            [1.0, 3.0, -1.0],
            [1.0, 3.0, 4.0],
            [1.0, -3.0, 4.0],
        ]);
        let events =
            surface_surface_intersection_events(&cone, &vertical, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(hyperbola) = event else {
                panic!("expected exact hyperbola halves")
            };
            assert_eq!(hyperbola.degree(), 2);
            assert!(!hyperbola.is_closed().unwrap());
            let length = hyperbola.length(Tolerance::DEFAULT).unwrap();
            assert!((length - 2.352738701354).abs() < 1e-8);
            // Rhino's fitted cubic half is about 1.3e-5 shorter.
            assert!((length - 2.352726136).abs() < 2e-5);
            for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let domain = hyperbola.domain();
                let point = hyperbola
                    .evaluate(*domain.start() + fraction * (*domain.end() - *domain.start()))
                    .unwrap();
                assert!((point.x() - 1.0).abs() < 1e-10);
                assert!((point.x().hypot(point.y()) - point.z() * 2.0 / 3.0).abs() < 1e-10);
                assert!(point.z() >= 1.5 - 1e-10 && point.z() <= 3.0 + 1e-10);
            }
        }
        let half_patch = patch([
            [1.0, -3.0, -1.0],
            [1.0, 0.0, -1.0],
            [1.0, 0.0, 4.0],
            [1.0, -3.0, 4.0],
        ]);
        let clipped =
            surface_surface_intersection_events(&cone, &half_patch, Tolerance::DEFAULT).unwrap();
        assert!(
            clipped
                .iter()
                .any(|event| matches!(event, SurfaceSurfaceIntersectionEvent::Curve(_)))
        );
        let boundary_tangent = patch([
            [2.0, -3.0, -1.0],
            [2.0, 3.0, -1.0],
            [2.0, 3.0, 4.0],
            [2.0, -3.0, 4.0],
        ]);
        let contact =
            surface_surface_intersection_events(&cone, &boundary_tangent, Tolerance::DEFAULT)
                .unwrap();
        assert!(
            matches!(contact.as_slice(), [SurfaceSurfaceIntersectionEvent::Point(p)] if p.distance_to(point(2.0, 0.0, 3.0)).unwrap() < 1e-10)
        );
    }

    #[test]
    fn pole_crossing_plane_keeps_only_finite_exact_conic_arc() {
        let frame = crate::Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
        for (slope, intercept, expected_middle_x) in
            [(2.0, 1.5, -3.0 / 7.0), (1.5, 1.0, -1.0 / 3.0)]
        {
            let tilted = patch([
                [-4.0, -4.0, intercept - 4.0 * slope],
                [4.0, -4.0, intercept + 4.0 * slope],
                [4.0, 4.0, intercept + 4.0 * slope],
                [-4.0, 4.0, intercept - 4.0 * slope],
            ]);
            let events =
                surface_surface_intersection_events(&cone, &tilted, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(conic)] = events.as_slice() else {
                panic!("expected one finite conic arc, got {events:#?}")
            };
            assert_eq!(conic.degree(), 2);
            assert!(!conic.is_closed().unwrap());
            let domain = conic.domain();
            let middle = conic
                .evaluate(0.5 * (*domain.start() + *domain.end()))
                .unwrap();
            assert!((middle.x() - expected_middle_x).abs() < 1e-8);
            for fraction in [0.0, 0.2, 0.5, 0.8, 1.0] {
                let domain = conic.domain();
                let parameter = *domain.start() + fraction * (*domain.end() - *domain.start());
                let point = conic.evaluate(parameter).unwrap();
                assert!((point.z() - (intercept + slope * point.x())).abs() < 1e-8);
                assert!((point.x().hypot(point.y()) - point.z() * (2.0 / 3.0)).abs() < 1e-8);
                assert!(point.z() >= -1e-8 && point.z() <= 3.0 + 1e-8);
            }
        }
        let remote = patch([
            [-4.0, -4.0, 92.0],
            [4.0, -4.0, 108.0],
            [4.0, 4.0, 108.0],
            [-4.0, 4.0, 92.0],
        ]);
        assert!(
            surface_surface_intersection_events(&cone, &remote, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        for slope in [1.55, 3.0, 10.0, 100.0] {
            let tilted = patch([
                [-4.0, -4.0, 1.5 - 4.0 * slope],
                [4.0, -4.0, 1.5 + 4.0 * slope],
                [4.0, 4.0, 1.5 + 4.0 * slope],
                [-4.0, 4.0, 1.5 - 4.0 * slope],
            ]);
            let events =
                surface_surface_intersection_events(&cone, &tilted, Tolerance::DEFAULT).unwrap();
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event, SurfaceSurfaceIntersectionEvent::Curve(_)))
            );
        }
    }

    #[test]
    fn pole_crossing_section_wraps_circle_seam_on_rotated_negative_cone() {
        let frame = crate::Frame3::try_from_normal(
            point(1.0, 2.0, 3.0),
            crate::Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(frame, 2.0, -3.0).unwrap();
        let tilted = NurbsSurface::try_bilinear([
            frame.point_at([-4.0, -4.0, -9.5]).unwrap(),
            frame.point_at([4.0, -4.0, 6.5]).unwrap(),
            frame.point_at([4.0, 4.0, 6.5]).unwrap(),
            frame.point_at([-4.0, 4.0, -9.5]).unwrap(),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&cone, &tilted, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(arc) = event else {
                panic!("expected a rational conic arc")
            };
            for fraction in [0.0, 0.5, 1.0] {
                let domain = arc.domain();
                let parameter = *domain.start() + fraction * (*domain.end() - *domain.start());
                let local = frame
                    .coordinates_of(arc.evaluate(parameter).unwrap())
                    .unwrap();
                assert!((local[2] - (-1.5 + 2.0 * local[0])).abs() < 1e-8);
                assert!((local[0].hypot(local[1]) + local[2] * (2.0 / 3.0)).abs() < 1e-8);
                assert!(local[2] >= -3.0 - 1e-8 && local[2] <= 1e-8);
            }
        }
    }

    #[test]
    fn pole_crossing_section_is_stable_far_from_origin() {
        let frame = crate::Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            crate::Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
        let tilted = NurbsSurface::try_bilinear([
            frame.point_at([-4.0, -4.0, -6.5]).unwrap(),
            frame.point_at([4.0, -4.0, 9.5]).unwrap(),
            frame.point_at([4.0, 4.0, 9.5]).unwrap(),
            frame.point_at([-4.0, 4.0, -6.5]).unwrap(),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&cone, &tilted, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(conic)] = events.as_slice() else {
            panic!("expected a translated conic arc, got {events:#?}")
        };
        let midpoint = conic
            .evaluate(0.5 * (*conic.domain().start() + *conic.domain().end()))
            .unwrap();
        let local = frame.coordinates_of(midpoint).unwrap();
        assert!((local[0] + 3.0 / 7.0).abs() < 1e-7);
        assert!((local[2] - (1.5 + 2.0 * local[0])).abs() < 1e-7);
    }
}
