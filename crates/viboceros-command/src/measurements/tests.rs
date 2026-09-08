use super::*;
use crate::CommandRegistry;
use viboceros_document::SelectionMode;
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, TriangleMesh, Vector3};

#[test]
fn area_measures_selected_nurbs_and_polycurves_without_modifying_the_document() {
    use viboceros_geometry::{Circle3, CurveSegment3, LineSegment, PolyCurve3, UnitVector3};
    let mut document = Document::default();
    let circle = Circle3::try_new(
        Point3::try_new(10., 0., 0.).unwrap(),
        2.,
        UnitVector3::try_new(0., 0., 1., document.tolerance()).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let circle_id = document
        .add_geometry(Geometry::NurbsCurve(circle.to_nurbs().unwrap()))
        .unwrap();
    let corners = [[0., 0.], [3., 0.], [3., 4.], [0., 4.], [0., 0.]]
        .map(|[x, y]| Point3::try_new(x, y, 0.).unwrap());
    let polycurve = PolyCurve3::try_new(
        corners
            .windows(2)
            .map(|pair| {
                CurveSegment3::Line(
                    LineSegment::try_new(pair[0], pair[1], document.tolerance()).unwrap(),
                )
            })
            .collect(),
    )
    .unwrap();
    let polygon_id = document
        .add_geometry(Geometry::PolyCurve(polycurve))
        .unwrap();
    document
        .select_objects([circle_id, polygon_id], SelectionMode::Replace)
        .unwrap();
    let original = document.objects().cloned().collect::<Vec<_>>();
    let selection = document.selected_object_ids().collect::<Vec<_>>();
    let undo = document.undo_label().map(str::to_owned);
    let redo = document.redo_label().map(str::to_owned);
    let output = CommandRegistry::with_builtins()
        .execute(&mut document, "Area")
        .unwrap();
    assert!(output.starts_with("Measured 2 object(s): total area "));
    let area = output
        .split_whitespace()
        .last()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((area - (12. + 4. * std::f64::consts::PI)).abs() < 1e-9);
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    assert_eq!(document.undo_label(), undo.as_deref());
    assert_eq!(document.redo_label(), redo.as_deref());
}

#[test]
fn measurement_output_preserves_small_nonzero_geometry() {
    let mut document = Document::new(Tolerance::try_new(1e-20, 1e-12, 1e-10).unwrap());
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut document, "Line 0,0,0 1e-15,0,0")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let output = registry.execute(&mut document, "Length").unwrap();
    assert_eq!(
        output
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        1e-15
    );
    registry.execute(&mut document, "Circle 0,0 1e-8").unwrap();
    let circle = document.objects().last().unwrap();
    let Geometry::Circle(geometry) = circle.geometry() else {
        panic!("circle");
    };
    let expected_area = geometry.area().unwrap();
    document
        .select_object(circle.id(), SelectionMode::Replace)
        .unwrap();
    let output = registry.execute(&mut document, "Area").unwrap();
    assert!(expected_area > 0. && expected_area < 1e-12);
    assert_eq!(
        output
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        expected_area
    );

    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1e-5, 0., 0.).unwrap(),
            Point3::try_new(0., 1e-5, 0.).unwrap(),
            Point3::try_new(0., 0., 1e-5).unwrap(),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let expected_volume = mesh.signed_volume().unwrap();
    let id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    let output = registry.execute(&mut document, "Volume").unwrap();
    assert!(expected_volume > 0. && expected_volume < 1e-12);
    assert_eq!(
        output
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        expected_volume
    );
}

#[test]
fn measurement_format_is_compact_and_roundtrips_finite_values() {
    for value in [
        0.,
        -0.,
        1.,
        -4.,
        1e-6,
        1e12,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::MAX,
        -f64::MAX,
    ] {
        let text = format_measurement(value);
        assert_eq!(text.parse::<f64>().unwrap(), value);
        assert!(text.len() <= 26, "{text}");
    }
    assert_eq!(format_measurement(-0.), "0");
    assert_eq!(format_measurement(4.), "4");
    assert_eq!(format_measurement(1e-15), "1e-15");
    let mut state = 51_u64;
    for _ in 0..10_000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let value = f64::from_bits(state);
        if value.is_finite() {
            let text = format_measurement(value);
            assert_eq!(text.parse::<f64>().unwrap(), value);
            assert!(text.len() <= 26, "{text}");
        }
    }
}

#[test]
fn volume_cancels_large_closed_meshes_without_intermediate_overflow() {
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1e103, 0., 0.).unwrap(),
            Point3::try_new(0., 1e103, 0.).unwrap(),
            Point3::try_new(0., 0., 1e103).unwrap(),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let expected = mesh.signed_volume().unwrap();
    assert!(expected > f64::MAX * 0.5);
    let ids = [mesh.clone(), mesh.clone(), mesh.reversed()]
        .into_iter()
        .map(|mesh| document.add_geometry(Geometry::Mesh(mesh)).unwrap())
        .collect::<Vec<_>>();
    document
        .select_objects(ids, SelectionMode::Replace)
        .unwrap();
    let output = CommandRegistry::with_builtins()
        .execute(&mut document, "Volume")
        .unwrap();
    assert_eq!(
        output
            .split_whitespace()
            .last()
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        expected
    );
}

#[test]
fn streaming_aggregation_preserves_small_terms_and_rejects_invalid_values() {
    let mut document = Document::default();
    let mut ids = Vec::new();
    for x in [0., 1., 2.] {
        ids.push(
            document
                .add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
                .unwrap(),
        );
    }
    document
        .select_objects(ids, SelectionMode::Replace)
        .unwrap();
    for (sign, values, expected) in [
        (MeasurementSign::Nonnegative, [1e16, 1., 1.], 1e16 + 2.),
        (MeasurementSign::Signed, [1e16, 1., -1e16], 1.),
        (
            MeasurementSign::Signed,
            [f64::MAX, f64::MAX, -f64::MAX],
            f64::MAX,
        ),
    ] {
        let mut values = values.into_iter();
        assert_eq!(
            selected_measurement(&document, sign, |_, _| Ok(values.next().unwrap())).unwrap(),
            (3, expected)
        );
        assert!(values.next().is_none());
    }
    for invalid in [f64::NAN, f64::INFINITY, -1.] {
        assert!(
            selected_measurement(&document, MeasurementSign::Nonnegative, |_, _| Ok(invalid))
                .is_err()
        );
    }
    assert!(selected_measurement(&document, MeasurementSign::Signed, |_, _| Ok(f64::MAX)).is_err());
}

#[test]
fn measurement_failures_preserve_selection_and_both_history_stacks() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0 1").unwrap();
    registry.execute(&mut document, "Point 9,9").unwrap();
    registry.execute(&mut document, "Point 8,8").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let selection = document.selected_object_ids().collect::<Vec<_>>();
    let undo = document.undo_label().map(str::to_owned);
    let redo = document.redo_label().map(str::to_owned);
    assert!(redo.is_some());
    for command in [
        "Length",
        "Area",
        "Volume",
        "Length extra",
        "Area extra",
        "Volume extra",
    ] {
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            selection
        );
        assert_eq!(document.objects().count(), 2);
        assert_eq!(document.undo_label(), undo.as_deref());
        assert_eq!(document.redo_label(), redo.as_deref());
    }
    registry.execute(&mut document, "SelNone").unwrap();
    for command in ["Length", "Area", "Volume"] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::NoObjectsSelected)
        ));
        assert_eq!(document.undo_label(), undo.as_deref());
        assert_eq!(document.redo_label(), redo.as_deref());
    }
}

#[test]
fn length_and_area_measure_mixed_selected_geometry_without_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0 2").unwrap();
    registry
        .execute(&mut document, "Rectangle 0,0 3,4")
        .unwrap();
    registry
        .execute(&mut document, "Ellipse 0,0 3,0 0,2")
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let history = document.undo_label().map(str::to_owned);

    let length_message = registry.execute(&mut document, "Len").unwrap();
    let length = length_message
        .split_whitespace()
        .next_back()
        .unwrap()
        .parse::<Real>()
        .unwrap();
    let expected_length = 4.0 * std::f64::consts::PI + 14.0 + 15.865_439_589_290_588;
    assert!(
        Tolerance::try_new(1.0e-10, 1.0e-12, 1.0e-12)
            .unwrap()
            .approx_eq(length, expected_length)
    );

    let area_message = registry.execute(&mut document, "Area").unwrap();
    let area = area_message
        .split_whitespace()
        .next_back()
        .unwrap()
        .parse::<Real>()
        .unwrap();
    assert!(
        Tolerance::try_new(1.0e-10, 1.0e-12, 1.0e-12)
            .unwrap()
            .approx_eq(area, 10.0 * std::f64::consts::PI + 12.0)
    );
    assert_eq!(document.undo_label(), history.as_deref());

    registry.execute(&mut document, "Point 9,9").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Length"),
        Err(CommandError::UnsupportedLengthGeometry)
    ));
    assert!(matches!(
        registry.execute(&mut document, "Area"),
        Err(CommandError::UnsupportedAreaGeometry)
    ));
    assert_eq!(document.undo_label(), Some("Point"));
}

#[test]
fn area_measures_exact_nurbs_surfaces_and_breps_without_tessellation() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0e12, -2.0e12, 3.0e12).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let sphere_id = document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_sphere(frame, 2.0).unwrap(),
        ))
        .unwrap();
    let box_id = document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                frame,
                [[0.0, 1.0], [0.0, 2.0], [0.0, 3.0]],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects([sphere_id, box_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);

    let message = registry.execute(&mut document, "Area").unwrap();
    let area = message
        .split_whitespace()
        .next_back()
        .unwrap()
        .parse::<Real>()
        .unwrap();
    let expected = 16.0 * std::f64::consts::PI + 22.0;
    assert!((area - expected).abs() < 1.0e-10);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn volume_measures_meshes_and_exact_breps_with_stable_signed_accumulation() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let vertices = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 3.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 4.0).unwrap(),
    ];
    let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    let outward =
        TriangleMesh::try_new(vertices.clone(), faces.clone(), document.tolerance()).unwrap();
    let reversed = outward.reversed();
    let open = TriangleMesh::try_new(vertices, faces[..3].to_vec(), document.tolerance()).unwrap();
    let outward_id = document.add_geometry(Geometry::Mesh(outward)).unwrap();
    let reversed_id = document.add_geometry(Geometry::Mesh(reversed)).unwrap();
    let open_id = document.add_geometry(Geometry::Mesh(open)).unwrap();
    let frame = Frame3::try_from_normal(
        Point3::try_new(1.0e12, -2.0e12, 3.0e12).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let box_id = document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                frame,
                [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let history = document.undo_label().map(str::to_owned);

    document
        .select_object(outward_id, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Volume").unwrap(),
        "Measured 1 closed object(s): total volume 4"
    );
    document
        .select_object(reversed_id, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Volume").unwrap(),
        "Measured 1 closed object(s): total volume -4"
    );
    document
        .select_object(outward_id, SelectionMode::Add)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Volume").unwrap(),
        "Measured 2 closed object(s): total volume 0"
    );
    document
        .select_object(box_id, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Volume").unwrap(),
        "Measured 1 closed object(s): total volume 24"
    );
    document
        .select_object(reversed_id, SelectionMode::Add)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Volume").unwrap(),
        "Measured 2 closed object(s): total volume 20"
    );
    assert_eq!(document.undo_label(), history.as_deref());

    document
        .select_object(open_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Volume"),
        Err(CommandError::OpenMeshVolume)
    ));
    registry.execute(&mut document, "Point 9,9").unwrap();
    registry.execute(&mut document, "SelNone").unwrap();
    registry.execute(&mut document, "SelPt").unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Volume"),
        Err(CommandError::UnsupportedVolumeGeometry)
    ));
}
