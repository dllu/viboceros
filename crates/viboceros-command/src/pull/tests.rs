use super::*;
use viboceros_geometry::{Frame3, MeshFace, Polyline3};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane() -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_bilinear([
            p(0., 0., 0.),
            p(10., 0., 0.),
            p(10., 10., 0.),
            p(0., 10., 0.),
        ])
        .unwrap(),
    )
}
fn line(a: Point3, b: Point3) -> Geometry {
    Geometry::Line(LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap())
}
fn setup(source: Geometry, target: Geometry) -> (Document, CommandRegistry, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let a = doc.add_geometry(source).unwrap();
    let b = doc.add_geometry(target).unwrap();
    doc.select_objects_direct([a, b], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (doc, CommandRegistry::with_builtins(), a, b)
}
#[test]
fn planar_pull_ignores_cplane_direction_preserves_sources_and_replays_one_edit() {
    let (mut doc, r, a, b) = setup(line(p(-2., 5., 3.), p(12., 5., 3.)), plane());
    let source = doc.object(a).unwrap().clone();
    let context = CommandContext {
        construction_plane: Frame3::try_from_normal(
            p(0., 0., 0.),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    };
    r.execute_in_context(&mut doc, "Pull", context).unwrap();
    assert_eq!(doc.selected_object_count(), 1);
    let c = doc
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!((c.length(doc.tolerance()).unwrap() - 10.).abs() < 1e-8);
    for point in c.divide_by_count(32, true, doc.tolerance()).unwrap() {
        assert!(point.z().abs() < 1e-8 && point.x() >= -1e-8 && point.x() <= 10. + 1e-8);
    }
    assert_eq!(doc.object(a), Some(&source));
    assert!(doc.object(b).is_some());
    assert_eq!(doc.undo_label(), Some("Pull"));
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 2);
    doc.redo().unwrap();
    assert_eq!(doc.selected_object_count(), 1);
}
#[test]
fn point_pull_uses_closest_surface_and_mesh_boundaries() {
    let mesh = Geometry::Mesh(
        TriangleMesh::try_new_faces(
            vec![
                p(0., 0., 0.),
                p(10., 0., 0.),
                p(10., 10., 0.),
                p(0., 10., 0.),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    for target in [plane(), mesh] {
        let (mut doc, r, _, _) = setup(Geometry::Point(p(12., 5., 3.)), target);
        r.execute(&mut doc, "Pull").unwrap();
        assert_eq!(
            doc.selected_objects().next().unwrap().geometry(),
            &Geometry::Point(p(10., 5., 0.))
        );
    }
}
#[test]
fn trimmed_planar_pull_retains_hole_intervals() {
    let rectangle = |lo: f64, hi: f64| {
        Polyline3::try_new(
            vec![
                p(lo, lo, 0.),
                p(hi, lo, 0.),
                p(hi, hi, 0.),
                p(lo, hi, 0.),
                p(lo, lo, 0.),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let target = Brep::try_planar_face_with_holes(
        &rectangle(0., 10.),
        &[rectangle(4., 6.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (mut doc, r, _, _) = setup(line(p(-2., 5., 3.), p(12., 5., 3.)), Geometry::Brep(target));
    r.execute(&mut doc, "Pull").unwrap();
    assert_eq!(doc.selected_object_count(), 2);
    for object in doc.selected_objects() {
        let c = object.geometry().curve_ref().unwrap();
        assert!((c.length(doc.tolerance()).unwrap() - 4.).abs() < 1e-8);
    }
}
#[test]
fn output_layers_delete_input_and_temporary_ranges_use_shared_admission() {
    for option in ["Current", "Input", "TargetObject"] {
        let (mut doc, r, a, b) = setup(line(p(1., 5., 3.), p(9., 5., 3.)), plane());
        let input = doc
            .add_layer("Input", viboceros_document::ColorRgb::BLACK)
            .unwrap();
        let target = doc
            .add_layer("Target", viboceros_document::ColorRgb::BLACK)
            .unwrap();
        doc.set_objects_layer([a], input).unwrap();
        doc.set_objects_layer([b], target).unwrap();
        let expected = match option {
            "Input" => input,
            "TargetObject" => target,
            _ => doc.current_layer_id(),
        };
        r.execute(
            &mut doc,
            &format!("Pull Sources={a} Targets={b} OutputLayer={option} DeleteInput=Yes"),
        )
        .unwrap();
        assert!(doc.object(a).is_none());
        assert_eq!(
            doc.selected_objects()
                .next()
                .unwrap()
                .attributes()
                .layer_id(),
            expected
        );
        doc.undo().unwrap();
        assert!(doc.object(a).is_some());
        doc.clear_selection();
        r.execute(
            &mut doc,
            &format!("Pull Targets={b} SubCrv={a},2,6 DeleteInput=Yes"),
        )
        .unwrap();
        assert!(doc.object(a).is_some());
        let c = doc
            .selected_objects()
            .next()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert!((c.length(doc.tolerance()).unwrap() - 4.).abs() < 1e-8);
    }
}
#[test]
fn infinite_planes_use_their_own_normal_and_a_miss_preserves_input() {
    let (mut doc, r, a, _) = setup(Geometry::Point(p(3., 4., 5.)), plane());
    doc.clear_selection();
    r.execute(
        &mut doc,
        &format!("Pull Sources={a} InfinitePlane=WorldYZ DeleteInput=Yes"),
    )
    .unwrap();
    assert_eq!(
        doc.selected_objects().next().unwrap().geometry(),
        &Geometry::Point(p(0., 4., 5.))
    );
    doc.undo().unwrap();
    r.execute(&mut doc, "Line -2,-5,2 12,-5,2").unwrap();
    let source = doc.objects().last().unwrap().id();
    let target = doc
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::NurbsSurface(_)))
        .unwrap()
        .id();
    r.execute(
        &mut doc,
        &format!("Pull Sources={source} Targets={target} DeleteInput=Yes"),
    )
    .unwrap();
    assert!(doc.object(source).is_some());
}
#[test]
fn invalid_options_and_later_unsupported_targets_roll_back() {
    let (mut doc, r, a, b) = setup(line(p(1., 5., 3.), p(9., 5., 3.)), plane());
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for option in [
        "Direction=Custom",
        "Direction=0,0,1",
        "Loose=Maybe",
        "OutputLayer=Other",
        "DeleteInput=Maybe",
    ] {
        assert!(
            r.execute(&mut doc, &format!("Pull Sources={a} Targets={b} {option}"))
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.undo_label().is_none());
    }
    let point = doc.add_geometry(Geometry::Point(p(0., 0., 0.))).unwrap();
    assert!(
        r.execute(
            &mut doc,
            &format!("Pull Sources={a} Targets={b},{point} DeleteInput=Yes")
        )
        .is_err()
    );
    assert!(doc.object(a).is_some());
    assert_eq!(doc.objects().len(), 3);
}
fn folded_mesh(upward: bool) -> Geometry {
    Geometry::Mesh(
        TriangleMesh::try_new(
            vec![
                p(-4., 0., 0.),
                p(0., -4., 0.),
                p(0., 4., 0.),
                p(4., 0., if upward { 4. } else { -4. }),
            ],
            vec![[0, 1, 2], [1, 3, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}
#[test]
fn mesh_curve_pull_follows_fold_and_replays_delete_input_as_one_edit() {
    let (mut doc, r, a, b) = setup(line(p(-1., 0., 2.), p(4., 0., 2.)), folded_mesh(false));
    let layer = doc
        .add_layer("mesh-target", viboceros_document::ColorRgb::BLACK)
        .unwrap();
    doc.set_objects_layer([b], layer).unwrap();
    doc.clear_history().unwrap();
    r.execute(
        &mut doc,
        &format!("Pull Sources={a} Targets={b} DeleteInput=Yes OutputLayer=TargetObject"),
    )
    .unwrap();
    assert!(doc.object(a).is_none());
    let output = doc.selected_objects().next().unwrap();
    assert_eq!(output.attributes().layer_id(), layer);
    let Geometry::Polyline(polyline) = output.geometry() else {
        panic!()
    };
    assert!(polyline.vertices().contains(&p(0., 0., 0.)));
    assert!((polyline.length().unwrap() - (1. + 2_f64.sqrt())).abs() < 1e-8);
    assert_eq!(doc.undo_label(), Some("Pull"));
    doc.undo().unwrap();
    assert!(doc.object(a).is_some());
    assert_eq!(doc.objects().len(), 2);
    doc.redo().unwrap();
    assert!(doc.object(a).is_none());
}
#[test]
fn mesh_branch_failure_rolls_back_already_staged_surface_output() {
    let (mut doc, r, a, b) = setup(line(p(-1., 0., 2.), p(2., 0., 2.)), plane());
    let mesh = doc.add_geometry(folded_mesh(true)).unwrap();
    doc.clear_history().unwrap();
    let original = doc.objects().cloned().collect::<Vec<_>>();
    assert!(
        r.execute(
            &mut doc,
            &format!("Pull Sources={a} Targets={b},{mesh} DeleteInput=Yes")
        )
        .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
    assert!(doc.undo_label().is_none());
}
#[test]
fn mesh_curve_collapsed_image_preserves_whole_input_and_temporary_range() {
    let mesh = Geometry::Mesh(
        TriangleMesh::try_new(
            vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.)],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let (mut doc, r, a, b) = setup(line(p(-2., -2., 1.), p(-3., -3., 2.)), mesh);
    r.execute(
        &mut doc,
        &format!("Pull Sources={a} Targets={b} DeleteInput=Yes"),
    )
    .unwrap();
    assert!(doc.object(a).is_some());
    assert_eq!(doc.objects().len(), 2);
    assert!(doc.undo_label().is_none());
    r.execute(
        &mut doc,
        &format!("Pull Targets={b} SubCrv={a},0.2,0.8 DeleteInput=Yes"),
    )
    .unwrap();
    assert!(doc.object(a).is_some());
    assert_eq!(doc.objects().len(), 2);
}
#[cfg(feature = "native-smlib")]
#[test]
fn native_cylinder_seam_witnesses_admit_one_axial_curve() {
    let surface =
        NurbsSurface::try_cylinder(CommandContext::default().construction_plane, 2., 0., 5.)
            .unwrap();
    let (mut doc, r, _, _) = setup(
        line(p(3., 0., 0.5), p(3., 0., 4.5)),
        Geometry::NurbsSurface(surface),
    );
    r.execute(&mut doc, "Pull").unwrap();
    assert_eq!(doc.selected_object_count(), 1);
    let c = doc
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    for q in c.divide_by_count(32, true, doc.tolerance()).unwrap() {
        assert!((q.x() - 2.).abs() + q.y().abs() < 1e-8);
    }
}
#[cfg(feature = "native-smlib")]
#[test]
fn native_curve_pull_is_clipped_by_the_original_curved_face() {
    let surface =
        NurbsSurface::try_cylinder(CommandContext::default().construction_plane, 2., 0., 5.)
            .unwrap();
    let [_, upper] = Brep::try_split_rectangular_surface_face_v(
        surface.clone(),
        surface.domain_u(),
        surface.domain_v(),
        2.5,
        false,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (mut doc, r, _, _) = setup(line(p(3., 1., 0.5), p(3., 1., 4.5)), Geometry::Brep(upper));
    r.execute(&mut doc, "Pull").unwrap();
    assert_eq!(doc.selected_object_count(), 1);
    let c = doc
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!((c.length(doc.tolerance()).unwrap() - 2.).abs() < 1e-8);
    for q in c.divide_by_count(32, true, doc.tolerance()).unwrap() {
        assert!(q.z() >= 2.5 - 1e-8);
        assert!((q.x().hypot(q.y()) - 2.).abs() < 1e-8);
    }
}
