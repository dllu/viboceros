//! transforms surface array command regressions.
use super::*;

fn sorted_points(mut points: Vec<Point3>) -> Vec<Point3> {
    points.sort_by(|left, right| {
        left.x()
            .total_cmp(&right.x())
            .then_with(|| left.y().total_cmp(&right.y()))
            .then_with(|| left.z().total_cmp(&right.z()))
    });
    points
}

#[test]
fn surface_array_uv_matches_rhino_selection_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let originals = add_orient_triad(&mut document);
    let target = add_named_surface(
        &mut document,
        NurbsSurface::try_bilinear([
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(12.0, 10.0, 10.0).unwrap(),
            Point3::try_new(0.0, 10.0, 10.0).unwrap(),
        ])
        .unwrap(),
        "Target",
    );
    let message = registry
        .execute(
            &mut document,
            "ArraySrf 3 2 BasePoint=1,2,3 SurfaceName=target Mode=UV",
        )
        .unwrap();
    assert!(message.contains("creating 18 copy object(s)"));
    assert_eq!(document.objects().len(), 22);
    assert_eq!(document.groups().len(), 7);
    assert_eq!(document.undo_label(), Some("ArraySrf"));
    assert!(!document.is_selected(target));
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        originals.into_iter().collect()
    );

    let actual_starts = sorted_points(
        document
            .objects()
            .filter(|object| object.attributes().name() == Some("x") && object.id() != originals[0])
            .map(|object| match object.geometry() {
                Geometry::Line(line) => line.start(),
                _ => panic!("expected a surface-array line"),
            })
            .collect(),
    );
    let expected_starts = sorted_points(
        [
            [0.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 10.0, 10.0],
            [6.0, 10.0, 10.0],
            [12.0, 10.0, 10.0],
        ]
        .map(|point| Point3::try_from(point).unwrap())
        .to_vec(),
    );
    assert_eq!(actual_starts, expected_starts);

    let copy_ids = document
        .objects()
        .map(|object| object.id())
        .filter(|id| !originals.contains(id) && *id != target)
        .collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 4);
    assert_eq!(document.groups().len(), 7);
    registry.execute(&mut document, "Redo").unwrap();
    assert!(copy_ids.iter().all(|id| document.object(*id).is_some()));
    assert_eq!(document.groups().len(), 7);
}

#[test]
fn surface_array_isocurve_evenly_divides_rational_surface_edges() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let layer = document.current_layer_id();
    let source = document
        .add_geometry_with_attributes(
            Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap()),
            ObjectAttributes::on_layer(layer).with_name("Stud"),
        )
        .unwrap();
    let middle_weight = 0.5_f64.sqrt();
    let mut controls = Vec::new();
    for z in [0.0, 10.0] {
        controls.extend([
            WeightedPoint3::try_new(Point3::try_new(10.0, 0.0, z).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(10.0, 10.0, z).unwrap(), middle_weight)
                .unwrap(),
            WeightedPoint3::try_new(Point3::try_new(0.0, 10.0, z).unwrap(), 1.0).unwrap(),
        ]);
    }
    let target = add_named_surface(
        &mut document,
        NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap(),
        "Cylinder",
    );
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "ArraySrf 4 1 BasePoint=1,2,3 SurfaceName=Cylinder Mode=Isocurve",
        )
        .unwrap();
    assert!(!document.is_selected(target));
    let mut copies = document
        .objects()
        .filter(|object| object.attributes().name() == Some("Stud") && object.id() != source)
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected a surface-array point"),
        })
        .collect::<Vec<_>>();
    copies.sort_by(|left, right| left.y().total_cmp(&right.y()));
    for (actual, angle) in copies.into_iter().zip([
        0.0,
        std::f64::consts::FRAC_PI_6,
        std::f64::consts::FRAC_PI_3,
        std::f64::consts::FRAC_PI_2,
    ]) {
        assert!(actual.is_near(
            Point3::try_new(10.0 * angle.cos(), 10.0 * angle.sin(), 0.0).unwrap(),
            document.tolerance()
        ));
    }
}

#[test]
fn surface_array_custom_up_last_target_and_errors_are_atomic() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let originals = add_orient_triad(&mut document);
    let target = add_named_surface(
        &mut document,
        NurbsSurface::try_bilinear([
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(12.0, 10.0, 10.0).unwrap(),
            Point3::try_new(0.0, 10.0, 10.0).unwrap(),
        ])
        .unwrap(),
        "Target",
    );
    registry
        .execute(
            &mut document,
            "ArraySrf 1 1 BasePoint=1,2,3 Up=0,1,0 SurfaceName=Target",
        )
        .unwrap();
    let root_half = 0.5_f64.sqrt();
    for (name, original, expected_end) in [
        (
            "x",
            originals[0],
            Point3::try_new(0.0, root_half, root_half).unwrap(),
        ),
        (
            "y",
            originals[1],
            Point3::try_new(0.0, -root_half, root_half).unwrap(),
        ),
        ("z", originals[2], Point3::try_new(1.0, 0.0, 0.0).unwrap()),
    ] {
        let copy = orient_line(&document, name, &[original]);
        assert_eq!(copy.start(), Point3::try_new(0.0, 0.0, 0.0).unwrap());
        assert!(copy.end().is_near(expected_end, document.tolerance()));
    }
    registry.execute(&mut document, "Undo").unwrap();

    document.select_object(target, SelectionMode::Add).unwrap();
    registry
        .execute(&mut document, "ArraySrf 1 1 BasePoint=1,2,3")
        .unwrap();
    assert!(!document.is_selected(target));
    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_object(originals[0], SelectionMode::Replace)
        .unwrap();

    let object_count = document.objects().len();
    let group_count = document.groups().len();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "ArraySrf 0 1 BasePoint=1,2,3 SurfaceName=Target",
        "ArraySrf 1 1 SurfaceName=Target",
        "ArraySrf 1 1 BasePoint=1,2,3 Up=0,0,0 SurfaceName=Target",
        "ArraySrf 1 1 BasePoint=1,2,3 Mode=Bad SurfaceName=Target",
        "ArraySrf 1 1 BasePoint=1,2,3 SurfaceName=Missing",
        "ArraySrf 1 1 BasePoint=1,2,3 SurfaceName=x",
        "ArraySrf 1000001 1 BasePoint=1,2,3 SurfaceName=Target",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
    }
    assert_eq!(document.objects().len(), object_count);
    assert_eq!(document.groups().len(), group_count);
    assert_eq!(document.undo_label(), history.as_deref());
    assert_eq!(
        document.selected_object_ids().collect::<BTreeSet<_>>(),
        originals.into_iter().collect()
    );
}
