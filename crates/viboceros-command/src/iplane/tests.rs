use super::*;
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn crossing() -> (CommandRegistry, Document, ObjectId) {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 10,20,-3 10,20,5").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_objects([id], SelectionMode::Replace).unwrap();
    (registry, doc, id)
}
#[test]
fn aliases_intersect_outside_any_plane_rectangle_and_replay_one_edit() {
    for command in ["IPlane", "InfinitePlane", "IP", "_IPlane"] {
        let (registry, mut doc, id) = crossing();
        let source = doc.object(id).unwrap().clone();
        registry
            .execute(&mut doc, &format!("{command} Plane=WorldXY"))
            .unwrap();
        assert_eq!(doc.objects().len(), 2);
        assert_eq!(doc.object(id), Some(&source));
        assert_eq!(
            doc.selected_objects().next().unwrap().geometry(),
            &Geometry::Point(point(10., 20., 0.))
        );
        assert_eq!(doc.undo_label(), Some("IPlane"));
        doc.undo().unwrap();
        assert_eq!(doc.objects().len(), 1);
        doc.redo().unwrap();
        assert_eq!(doc.objects().len(), 2);
    }
}
#[test]
fn default_plane_uses_command_context_and_inputs_are_deduplicated() {
    let (registry, mut doc, id) = crossing();
    let context = CommandContext {
        construction_plane: CommandContext::default()
            .construction_plane
            .with_origin(point(0., 0., 2.)),
    };
    registry
        .execute_in_context(&mut doc, &format!("IPlane Objects={id},{id}"), context)
        .unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(
        doc.selected_objects().next().unwrap().geometry(),
        &Geometry::Point(point(10., 20., 2.))
    );
}
#[test]
fn misses_create_no_geometry_and_bad_later_inputs_roll_back() {
    let (registry, mut doc, id) = crossing();
    registry.execute(&mut doc, "IPlane Plane=WorldYZ").unwrap();
    assert_eq!(doc.objects().len(), 1);
    registry.execute(&mut doc, "Point 0,0,0").unwrap();
    let unsupported = doc.objects().find(|o| o.id() != id).unwrap().id();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("IPlane Objects={id},{unsupported} Plane=WorldXY")
            )
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), history.as_deref());
    for args in [
        "Plane=0,0,0,0,0,0",
        "Plane=WorldXY Plane=WorldYZ",
        "Unknown=Yes",
        "Objects=invalid",
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("IPlane {args}"))
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
}
#[test]
fn planar_object_reference_is_read_only_and_does_not_bound_the_cut() {
    let (registry, mut doc, id) = crossing();
    registry
        .execute(&mut doc, "SrfPt 0,0,1 1,0,1 1,1,1 0,1,1")
        .unwrap();
    let reference = doc.objects().find(|o| o.id() != id).unwrap().clone();
    registry
        .execute(
            &mut doc,
            &format!("IPlane Objects={id} Plane=Object:{}", reference.id()),
        )
        .unwrap();
    assert_eq!(doc.object(reference.id()), Some(&reference));
    assert_eq!(
        doc.selected_objects().next().unwrap().geometry(),
        &Geometry::Point(point(10., 20., 1.))
    );
}
#[test]
fn crossing_surface_uses_existing_intersection_producer() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "SrfPt 2,0,-1 2,3,-1 2,3,1 2,0,1")
        .unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "IPlane Plane=WorldXY").unwrap();
    assert_eq!(doc.selected_object_count(), 1);
    let curve = doc
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    for t in [0., 0.5, 1.] {
        let domain = curve.domain();
        let p = curve
            .evaluate(*domain.start() + (*domain.end() - *domain.start()) * t)
            .unwrap();
        assert!(p.to_array()[2].abs() < doc.tolerance().absolute());
        assert!((p.to_array()[0] - 2.).abs() < doc.tolerance().absolute());
    }
}
#[test]
fn mesh_and_box_sections_preserve_sources_and_return_complete_curves() {
    let registry = CommandRegistry::with_builtins();
    for geometry in [
        Geometry::Mesh(
            TriangleMesh::try_new_faces(
                vec![
                    point(2., 0., -1.),
                    point(2., 3., -1.),
                    point(2., 3., 1.),
                    point(2., 0., 1.),
                ],
                vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        Geometry::Brep(
            Brep::try_command_box(
                CommandContext::default().construction_plane,
                [[-1., 1.], [-1., 1.], [-1., 1.]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(geometry.clone()).unwrap();
        registry
            .execute(&mut doc, &format!("IPlane Objects={id} Plane=WorldXY"))
            .unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &geometry);
        assert_eq!(doc.selected_object_count(), 1);
        let curve = doc
            .selected_objects()
            .next()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert!(
            (curve.length(doc.tolerance()).unwrap()
                - if matches!(geometry, Geometry::Mesh(_)) {
                    3.
                } else {
                    8.
                })
            .abs()
                < 1e-7
        );
        assert_eq!(
            curve.is_closed().unwrap(),
            matches!(geometry, Geometry::Brep(_))
        );
        for p in curve.divide_by_count(32, true, doc.tolerance()).unwrap() {
            assert!(p.z().abs() <= doc.tolerance().absolute());
        }
    }
}
#[test]
fn circle_crossings_tangent_contacts_and_coplanar_overlap_are_retained() {
    for (plane, points, curves) in [("WorldYZ", 2, 0), ("2,0,0,1,0,0", 1, 0), ("WorldXY", 0, 1)] {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        registry
            .execute(&mut doc, &format!("IPlane Plane={plane}"))
            .unwrap();
        assert_eq!(
            doc.selected_objects()
                .filter(|o| matches!(o.geometry(), Geometry::Point(_)))
                .count(),
            points,
            "{plane}"
        );
        assert_eq!(
            doc.selected_objects()
                .filter(|o| o.geometry().curve_ref().is_some())
                .count(),
            curves,
            "{plane}"
        );
    }
}
#[test]
fn explicit_hidden_inputs_decline_and_output_uses_current_layer() {
    let (registry, mut doc, id) = crossing();
    let layer = doc
        .add_layer("Result", viboceros_document::ColorRgb::new(12, 34, 56))
        .unwrap();
    doc.set_current_layer(layer).unwrap();
    registry.execute(&mut doc, "IPlane Plane=WorldXY").unwrap();
    assert_eq!(
        doc.selected_objects()
            .next()
            .unwrap()
            .attributes()
            .layer_id(),
        layer
    );
    doc.undo().unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "Hide").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(
        registry
            .execute(&mut doc, &format!("IPlane Objects={id} Plane=WorldXY"))
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}
#[test]
fn trimmed_face_sections_keep_hole_intervals_separate() {
    use viboceros_geometry::Polyline3;
    let rectangle = |lo: f64, hi: f64| {
        Polyline3::try_new(
            vec![
                point(lo, lo, 0.),
                point(hi, lo, 0.),
                point(hi, hi, 0.),
                point(lo, hi, 0.),
                point(lo, lo, 0.),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let face = Brep::try_planar_face_with_holes(
        &rectangle(0., 10.),
        &[rectangle(4., 6.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Brep(face)).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, &format!("IPlane Objects={id} Plane=0,5,0,0,1,0"))
        .unwrap();
    assert_eq!(doc.selected_object_count(), 2);
    for object in doc.selected_objects() {
        let curve = object.geometry().curve_ref().unwrap();
        assert!((curve.length(doc.tolerance()).unwrap() - 4.).abs() < 1e-7);
        for p in curve.divide_by_count(16, true, doc.tolerance()).unwrap() {
            assert!((p.y() - 5.).abs() <= doc.tolerance().absolute());
            assert!(p.x() <= 4. + 1e-7 || p.x() >= 6. - 1e-7);
        }
    }
}
