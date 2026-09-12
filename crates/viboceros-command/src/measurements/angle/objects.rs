//! Unoriented line/plane measurements, distinct from directed four-point angles.
use super::*;
use viboceros_geometry::Vector3;

pub(super) fn measure(document: &Document) -> Result<Real, CommandError> {
    let mut selected = document.selected_objects();
    let first = selected.next().ok_or(CommandError::NoObjectsSelected)?;
    let second = selected.next().ok_or(CommandError::Usage(
        "Angle TwoObjects requires exactly two selected objects",
    ))?;
    if selected.next().is_some() {
        return Err(CommandError::Usage(
            "Angle TwoObjects requires exactly two selected objects",
        ));
    }
    let (a, a_plane) = direction(first.geometry(), document.tolerance())?;
    let (b, b_plane) = direction(second.geometry(), document.tolerance())?;
    let a = a.normalized_nonzero()?.as_vector();
    let b = b.normalized_nonzero()?.as_vector();
    let sine = a.cross(b)?.length()?;
    let cosine = a.dot(b)?.abs();
    // Avoid subtracting nearly equal angles near parallel/opposite directions.
    Ok(if a_plane != b_plane {
        cosine.atan2(sine)
    } else {
        sine.atan2(cosine)
    }
    .to_degrees())
}

fn direction(geometry: &Geometry, tolerance: Tolerance) -> Result<(Vector3, bool), CommandError> {
    let unsupported = || {
        CommandError::Usage(
            "Angle TwoObjects accepts straight curves and planar single-face surfaces",
        )
    };
    let surface = match geometry {
        Geometry::NurbsSurface(surface) => Some(surface),
        Geometry::Brep(brep) if brep.faces().len() == 1 => Some(brep.faces()[0].surface()),
        _ => None,
    };
    if let Some(surface) = surface {
        let plane = surface.plane(tolerance)?.ok_or_else(unsupported)?;
        return Ok((plane.normal().as_vector(), true));
    }
    if let Geometry::Line(line) = geometry {
        return Ok((line.start().direction_to(line.end())?.as_vector(), false));
    }
    if let Some(curve) = geometry.nurbs_curve_representation()?
        && curve.is_linear(tolerance)?
    {
        return Ok((
            curve
                .evaluate(*curve.domain().start())?
                .direction_to(curve.evaluate(*curve.domain().end())?)?
                .as_vector(),
            false,
        ));
    }
    Err(unsupported())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_document::SelectionMode;
    use viboceros_geometry::{Frame3, LineSegment, NurbsSurface, Point3};

    fn point(p: [f64; 3]) -> Point3 {
        Point3::try_from(p).unwrap()
    }
    fn line(v: [f64; 3]) -> Geometry {
        Geometry::Line(LineSegment::try_new(point([0.; 3]), point(v), Tolerance::DEFAULT).unwrap())
    }
    fn plane(normal: [f64; 3]) -> Geometry {
        let frame = Frame3::try_from_normal(
            point([4., 5., 6.]),
            Vector3::try_from(normal).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        Geometry::NurbsSurface(
            NurbsSurface::try_bilinear(
                [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
                    .map(|p| frame.point_at(p).unwrap()),
            )
            .unwrap(),
        )
    }

    #[test]
    fn selected_object_angles_match_rhino_orientation_and_mixed_plane_conventions() {
        let registry = CommandRegistry::with_builtins();
        for (first, second, expected) in [
            (line([1., 0., 0.]), line([-1., 1., 0.]), 45.),
            (line([1., 0., 0.]), line([1., -1., 0.]), 45.),
            (plane([0., 0., 1.]), plane([1., 0., -1.]), 45.),
            (plane([0., 0., 1.]), plane([-1., 0., 1.]), 45.),
            (line([1., 0., 2.]), plane([0., 0., 1.]), 63.435),
            (line([1., 0., 0.]), plane([0., 0., 1.]), 0.),
            (line([0., 0., 1.]), plane([0., 0., 1.]), 90.),
        ] {
            for reverse_order in [false, true] {
                let mut document = Document::default();
                let a = document.add_geometry(first.clone()).unwrap();
                let b = document.add_geometry(second.clone()).unwrap();
                document
                    .select_objects(
                        if reverse_order { [b, a] } else { [a, b] },
                        SelectionMode::Replace,
                    )
                    .unwrap();
                let before = format!("{document:?}");
                for input in ["Angle", "Angle _TwoObjects"] {
                    let report = registry.execute(&mut document, input).unwrap();
                    let value: f64 = report.split_whitespace().nth(2).unwrap().parse().unwrap();
                    assert!((value - expected).abs() <= 0.0005, "{report}");
                    assert_eq!(format!("{document:?}"), before);
                }
            }
        }
    }

    #[test]
    fn object_angle_supports_native_linear_nurbs_and_single_face_breps() {
        let mut document = Document::default();
        let curve = line([1., 0., 1.])
            .nurbs_curve_representation()
            .unwrap()
            .unwrap();
        let Geometry::NurbsSurface(surface) = plane([0., 0., 1.]) else {
            unreachable!()
        };
        let brep = viboceros_geometry::Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
        let a = document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
        let b = document.add_geometry(Geometry::Brep(brep)).unwrap();
        document
            .select_objects([a, b], SelectionMode::Replace)
            .unwrap();
        assert!((measure(&document).unwrap() - 45.).abs() < 1e-12);
        let warped = NurbsSurface::try_bilinear(
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 1.], [0., 1., 0.]].map(point),
        )
        .unwrap();
        let c = document
            .add_geometry(Geometry::NurbsSurface(warped))
            .unwrap();
        document
            .select_objects([a, c], SelectionMode::Replace)
            .unwrap();
        let before = format!("{document:?}");
        assert!(measure(&document).is_err());
        assert_eq!(format!("{document:?}"), before);
    }

    #[test]
    fn object_angle_rejects_invalid_selection_and_retains_small_acute_angles() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let a = document.add_geometry(line([1., 0., 0.])).unwrap();
        let b = document.add_geometry(line([-1., 1e-12, 0.])).unwrap();
        document
            .select_objects([a, b], SelectionMode::Replace)
            .unwrap();
        assert!((measure(&document).unwrap() / 1e-12_f64.atan().to_degrees() - 1.).abs() < 1e-14);
        let c = document
            .add_geometry(Geometry::Point(point([0.; 3])))
            .unwrap();
        for ids in [vec![], vec![a], vec![a, b, c], vec![a, c]] {
            document
                .select_objects(ids, SelectionMode::Replace)
                .unwrap();
            let before = format!("{document:?}");
            assert!(registry.execute(&mut document, "Angle TwoObjects").is_err());
            assert_eq!(format!("{document:?}"), before);
        }
    }
}
