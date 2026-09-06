use super::*;
use viboceros_geometry::MeshFace;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn polyline() -> Geometry {
    Geometry::Polyline(
        Polyline3::try_with_parameters(
            vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 5., 1.)],
            vec![-7., 3., 13.],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}
fn mesh(disjoint: bool) -> Geometry {
    let mut vertices = vec![p(0., 0., 0.), p(4., 0., 0.), p(0., 3., 0.)];
    let mut faces = vec![MeshFace::Triangle([0, 1, 2])];
    if disjoint {
        vertices.extend([p(10., 0., 0.), p(14., 0., 0.), p(10., 3., 0.)]);
        faces.push(MeshFace::Triangle([3, 4, 5]));
    }
    Geometry::Mesh(TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap())
}
fn selected(geometry: Geometry) -> (Document, ObjectId) {
    let mut d = Document::default();
    let id = d.add_geometry(geometry).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    (d, id)
}

#[test]
fn curves_keep_native_polyline_parameters_and_source_attributes_for_both_choices() {
    for geometry in [
        polyline(),
        Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(4., 2., 0.), Tolerance::DEFAULT).unwrap(),
        ),
        Geometry::Arc(
            CircularArc3::try_from_three_points(
                p(3., 0., 0.),
                p(0., 3., 0.),
                p(-3., 0., 0.),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
    ] {
        for delete in [false, true] {
            let expected = geometry.converted_to_nurbs_curve().unwrap().unwrap();
            let (mut d, id) = selected(geometry.clone());
            let g = d.add_group(Some("Source group".into()), [id]).unwrap();
            let layer = d
                .add_layer("Different current layer", ColorRgb::BLACK)
                .unwrap();
            d.set_current_layer(layer).unwrap();
            let source = d.object(id).unwrap().clone();
            let r = CommandRegistry::with_builtins();
            r.execute(
                &mut d,
                &format!(
                    "ToNURBS DeleteInputObjects={}",
                    if delete { "Yes" } else { "No" }
                ),
            )
            .unwrap();
            assert_eq!(d.objects().len(), if delete { 1 } else { 2 });
            assert!(d.is_selected(id));
            let output = if delete {
                d.object(id).unwrap()
            } else {
                d.objects().find(|o| o.id() != id).unwrap()
            };
            assert_eq!(output.geometry(), &Geometry::NurbsCurve(expected));
            assert_eq!(output.attributes(), source.attributes());
            assert_eq!(output.group_ids(), source.group_ids());
            assert_eq!(d.is_selected(output.id()), delete);
            assert_eq!(
                d.group(g).unwrap().members().len(),
                if delete { 1 } else { 2 }
            );
            let after = d.objects().cloned().collect::<Vec<_>>();
            r.execute(&mut d, "Undo").unwrap();
            assert_eq!(d.objects().cloned().collect::<Vec<_>>(), [source]);
            r.execute(&mut d, "Redo").unwrap();
            assert_eq!(d.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}

#[test]
fn meshes_keep_disconnected_components_together_and_preserve_replacement_identity() {
    for trim in [false, true] {
        for delete in [false, true] {
            let geometry = mesh(true);
            let Geometry::Mesh(m) = &geometry else {
                panic!()
            };
            let expected = Brep::try_from_mesh(m, trim, Tolerance::DEFAULT).unwrap();
            let (mut d, id) = selected(geometry);
            let group = d.add_group(Some("mesh".into()), [id]).unwrap();
            let layer = d.add_layer("Other", ColorRgb::BLACK).unwrap();
            d.set_current_layer(layer).unwrap();
            let original = d.object(id).unwrap().clone();
            let r = CommandRegistry::with_builtins();
            r.execute(
                &mut d,
                &format!(
                    "ToNURBS MeshOptions TrimTriangularFaces={} DeleteInputObjects={}",
                    if trim { "Yes" } else { "No" },
                    if delete { "Yes" } else { "No" }
                ),
            )
            .unwrap();
            assert_eq!(d.objects().len(), if delete { 1 } else { 2 });
            let output = if delete {
                d.object(id).unwrap()
            } else {
                d.objects().find(|o| o.id() != id).unwrap()
            };
            assert_eq!(output.geometry(), &Geometry::Brep(expected));
            assert_eq!(output.attributes(), original.attributes());
            assert_eq!(output.group_ids(), [group]);
            assert_eq!(d.is_selected(output.id()), delete);
            r.execute(&mut d, "Undo").unwrap();
            assert_eq!(d.objects().cloned().collect::<Vec<_>>(), [original]);
            r.execute(&mut d, "Redo").unwrap();
            assert_eq!(d.objects().len(), if delete { 1 } else { 2 });
        }
    }
}

#[test]
fn nurbs_noop_does_not_accept_choices_or_change_history_or_redo() {
    let (mut d, id) = selected(polyline());
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, "ToNURBS DeleteInputObjects=Yes").unwrap();
    let nurbs = d.object(id).unwrap().geometry().clone();
    r.execute(&mut d, "Undo").unwrap();
    let noop = d.add_geometry(nurbs).unwrap();
    r.execute(&mut d, "Move 0,0,0 1,0,0").unwrap();
    r.execute(&mut d, "Undo").unwrap();
    d.select_objects_direct([noop], SelectionMode::Replace)
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let history = d.undo_label().map(str::to_owned);
    assert!(d.can_redo());
    r.execute(&mut d, "ToNURBS DeleteInputObjects=No").unwrap();
    assert!(d.can_redo());
    assert_eq!(d.undo_label(), history.as_deref());
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    let (mut next, id) = selected(polyline());
    r.execute(&mut next, "ToNURBS").unwrap();
    assert_eq!(next.objects().len(), 1);
    assert!(matches!(
        next.object(id).unwrap().geometry(),
        Geometry::NurbsCurve(_)
    ));
}

#[test]
fn invalid_options_and_ineligible_selection_do_not_accept_preferences() {
    let r = CommandRegistry::with_builtins();
    let (mut seed, _) = selected(polyline());
    r.execute(&mut seed, "ToNURBS DeleteInputObjects=Yes")
        .unwrap();
    let (mut d, id) = selected(Geometry::Point(p(0., 0., 0.)));
    let history = d.undo_label().map(str::to_owned);
    for command in [
        "ToNURBS DeleteInputObjects=No",
        "ToNURBS DeleteInputObjects=No extra",
        "ToNURBS DeleteInputObjects=No DeleteInput=Yes",
        "ToNURBS Other=Yes",
        "ToNURBS MeshOptions TrimTriangularFaces=No",
    ] {
        assert!(r.execute(&mut d, command).is_err());
        assert_eq!(d.objects().len(), 1);
        assert!(d.is_selected(id));
        assert_eq!(d.undo_label(), history.as_deref());
    }
    let (mut next, _) = selected(polyline());
    r.execute(&mut next, "ToNURBS").unwrap();
    assert_eq!(next.objects().len(), 1);
    let (mut independent, _) = selected(polyline());
    CommandRegistry::with_builtins()
        .execute(&mut independent, "ToNURBS")
        .unwrap();
    assert_eq!(independent.objects().len(), 2);
}

#[test]
fn conversion_budget_is_aggregate_and_saturating() {
    let mut total = 0;
    charge(&mut total, MAX_OUTPUT_CONTROLS - 1).unwrap();
    charge(&mut total, 1).unwrap();
    assert!(charge(&mut total, 1).is_err());
    assert!(charge(&mut total, usize::MAX).is_err());
}

#[test]
fn replacement_renews_document_order_but_not_selection_action_order_or_identity() {
    let (mut d, curve) = selected(polyline());
    let point = d.add_geometry(Geometry::Point(p(20., 0., 0.))).unwrap();
    let nurbs = d
        .add_geometry(Geometry::NurbsCurve(
            polyline().nurbs_curve_representation().unwrap().unwrap(),
        ))
        .unwrap();
    let mesh = d.add_geometry(mesh(false)).unwrap();
    let tail = d.add_geometry(Geometry::Point(p(30., 0., 0.))).unwrap();
    d.add_group(Some("all".into()), [curve, point, nurbs, mesh, tail])
        .unwrap();
    let selection = [mesh, tail, nurbs, point, curve];
    d.select_objects_direct([], SelectionMode::Replace).unwrap();
    for id in selection {
        d.select_objects_direct([id], SelectionMode::Add).unwrap();
    }
    let before = d.objects().cloned().collect::<Vec<_>>();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, "ToNURBS DeleteInputObjects=Yes").unwrap();
    assert_eq!(
        d.objects().map(|o| o.id()).collect::<Vec<_>>(),
        [point, nurbs, tail, curve, mesh]
    );
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), selection);
    let after = d.objects().cloned().collect::<Vec<_>>();
    r.execute(&mut d, "Undo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    r.execute(&mut d, "Redo").unwrap();
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn later_resource_failure_leaves_sources_groups_selection_history_and_choices_unchanged() {
    let r = CommandRegistry::with_builtins();
    let (mut seed, _) = selected(polyline());
    r.execute(&mut seed, "ToNURBS DeleteInputObjects=Yes")
        .unwrap();
    let (mut d, first) = selected(polyline());
    let huge = TriangleMesh::try_new_faces(
        vec![p(0., 0., 0.), p(4., 0., 0.), p(0., 3., 0.)],
        vec![MeshFace::Triangle([0, 1, 2]); MAX_OUTPUT_CONTROLS / 4 + 1],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let second = d.add_geometry(Geometry::Mesh(huge)).unwrap();
    d.select_objects_direct([second], SelectionMode::Add)
        .unwrap();
    let group = d
        .add_group(Some("sources".into()), [first, second])
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let history = d.undo_label().map(str::to_owned);
    assert!(matches!(
        r.execute(&mut d, "ToNURBS DeleteInputObjects=No"),
        Err(CommandError::TooManyNurbsConversionControls { .. })
    ));
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        d.group(group).unwrap().members().collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), [first, second]);
    assert_eq!(d.undo_label(), history.as_deref());
    let (mut next, _) = selected(polyline());
    r.execute(&mut next, "ToNURBS").unwrap();
    assert_eq!(next.objects().len(), 1);
}
