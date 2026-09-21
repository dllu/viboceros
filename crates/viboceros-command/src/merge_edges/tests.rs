use super::*;

fn cube() -> Brep {
    Brep::try_box(
        CommandContext::default().construction_plane,
        [[0., 2.], [0., 3.], [0., 5.]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn kink_surface(degrees: f64) -> NurbsSurface {
    let a = degrees.to_radians();
    NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        [0., 3.]
            .into_iter()
            .flat_map(|z| {
                [
                    [0., 0., z],
                    [10., 0., z],
                    [10. + 10. * a.cos(), 10. * a.sin(), z],
                ]
            })
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn an_uncertifiable_face_partition_rolls_back_all_objects_selection_and_redo() {
    let registry = CommandRegistry::with_builtins();
    // Geometrically C0, but stored with a full-order knot. The current exact
    // tensor slicer requires degree multiplicity and deliberately rejects it.
    let surface = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        [0., 3.]
            .into_iter()
            .flat_map(|z| [[0., 0., z], [10., 0., z], [10., 0., z], [20., 1., z]])
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for bare in [false, true] {
        for preselect in [false, true] {
            let mut doc = Document::default();
            let good = source(&mut doc);
            let bad = doc
                .add_geometry(if bare {
                    Geometry::NurbsSurface(surface.clone())
                } else {
                    Geometry::Brep(
                        Brep::try_surface_face(surface.clone(), doc.tolerance()).unwrap(),
                    )
                })
                .unwrap();
            registry.execute(&mut doc, "Point 9,9,9").unwrap();
            registry.execute(&mut doc, "Undo").unwrap();
            doc.select_objects_direct([good, bad], SelectionMode::Replace)
                .unwrap();
            let before = format!("{doc:?}");
            let error = if preselect {
                registry.execute(&mut doc, "MergeAllEdges")
            } else {
                registry.execute_postselected(&mut doc, "MergeAllEdges", Default::default())
            }
            .unwrap_err();
            assert!(matches!(
                error,
                CommandError::Geometry(GeometryError::InvalidBrepTopology {
                    context: "face partition requires a continuous full-degree knot"
                })
            ));
            assert_eq!(format!("{doc:?}"), before);
            assert!(doc.redo_label().is_some());
        }
    }
}

#[test]
fn surface_replacement_splits_after_cleanup_with_independent_angular_limits_and_history() {
    let registry = CommandRegistry::with_builtins();
    for (document_degrees, kink_degrees, faces, edges) in [
        (1e-8_f64, 0.09, 1, 4),
        (1e-8, 0.11, 2, 7),
        (0.5, 0.49, 1, 4),
        (0.5, 0.51, 2, 7),
        (5., 1.5, 1, 5),
        (5., 2.01, 2, 7),
    ] {
        for bare in [false, true] {
            for preselect in [false, true] {
                let tolerance =
                    Tolerance::try_new(1e-9, 1e-12, document_degrees.to_radians()).unwrap();
                let mut doc = Document::new(tolerance);
                let surface = kink_surface(kink_degrees);
                let geometry = if bare {
                    Geometry::NurbsSurface(surface)
                } else {
                    Geometry::Brep(
                        Brep::try_surface_face(surface, tolerance)
                            .unwrap()
                            .try_split_edges_at_parameters(&[(0, vec![1.])], tolerance)
                            .unwrap(),
                    )
                };
                let id = doc
                    .add_geometry_with_attributes(
                        geometry,
                        ObjectAttributes::on_layer(doc.current_layer_id()).with_name("crease"),
                    )
                    .unwrap();
                let peer = doc.add_geometry(Geometry::Brep(cube())).unwrap();
                let group = doc.add_group(Some("keep".into()), [id, peer]).unwrap();
                doc.select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
                let before = doc.objects().cloned().collect::<Vec<_>>();
                let report = if preselect {
                    registry.execute(&mut doc, "MergeAllEdges")
                } else {
                    registry.execute_postselected(&mut doc, "MergeAllEdges", Default::default())
                }
                .unwrap();
                let removed = usize::from(!bare && edges == 4);
                assert_eq!(
                    report,
                    format!("Merged {removed} redundant edge(s) in 1 object(s)")
                );
                let object = doc.object(id).unwrap();
                let Geometry::Brep(brep) = object.geometry() else {
                    panic!()
                };
                assert_eq!(brep.faces().len(), faces);
                assert_eq!(
                    brep.edges().len(),
                    if bare && faces == 1 && edges == 5 {
                        4
                    } else {
                        edges
                    }
                );
                assert_eq!(object.group_ids(), &[group]);
                assert_eq!(
                    object.attributes(),
                    before.iter().find(|o| o.id() == id).unwrap().attributes()
                );
                let after = doc.objects().cloned().collect::<Vec<_>>();
                assert_eq!(doc.object(peer), before.iter().find(|o| o.id() == peer));
                assert_eq!(doc.is_selected(id), preselect);
                assert_eq!(doc.undo_label(), Some("MergeAllEdges"));
                registry.execute(&mut doc, "Undo").unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                assert_eq!(doc.is_selected(id), preselect);
                registry.execute(&mut doc, "Redo").unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
                assert_eq!(doc.is_selected(id), preselect);
            }
        }
    }
}

fn source(doc: &mut Document) -> ObjectId {
    let cube = cube();
    let cuts = [0.125, 0.375, 0.875].map(|t| cube.edges()[0].curve().parameter_at(t).unwrap());
    let brep = cube
        .try_split_edges_at_parameters(&[(0, cuts.to_vec())], doc.tolerance())
        .unwrap();
    doc.add_geometry_with_attributes(
        Geometry::Brep(brep),
        ObjectAttributes::on_layer(doc.current_layer_id())
            .with_name("split cube")
            .with_object_color(ColorRgb::new(11, 22, 33)),
    )
    .unwrap()
}

#[test]
fn cleanup_preserves_id_attributes_groups_and_unselected_objects_in_one_history_step() {
    for preselect in [false, true] {
        let mut doc = Document::default();
        let a = source(&mut doc);
        let b = source(&mut doc);
        let peer = source(&mut doc);
        let group = doc
            .add_group(Some("assembly".into()), [a, b, peer])
            .unwrap();
        doc.select_objects_direct([b, a], SelectionMode::Replace)
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        let result = if preselect {
            registry.execute(&mut doc, "MergeAllEdges")
        } else {
            registry.execute_postselected(&mut doc, "MergeAllEdges", Default::default())
        }
        .unwrap();
        assert_eq!(result, "Merged 6 redundant edge(s) in 2 object(s)");
        let after = doc.objects().cloned().collect::<Vec<_>>();
        for (original, current) in before.iter().zip(&after) {
            assert_eq!(original.id(), current.id());
            assert_eq!(original.attributes(), current.attributes());
            assert_eq!(current.group_ids(), &[group]);
            let Geometry::Brep(brep) = current.geometry() else {
                panic!("B-rep lost")
            };
            assert_eq!(
                brep.edges().len(),
                if current.id() == peer { 15 } else { 12 }
            );
            assert!(brep.is_solid());
            assert_eq!(
                doc.is_selected(current.id()),
                preselect && current.id() != peer
            );
        }
        assert_eq!(doc.object(peer), before.iter().find(|o| o.id() == peer));
        assert_eq!(doc.undo_label(), Some("MergeAllEdges"));
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_count(), if preselect { 2 } else { 0 });
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        assert_eq!(doc.selected_object_count(), if preselect { 2 } else { 0 });
    }
}

#[test]
fn mixed_preselection_is_ignored_but_unsupported_only_selection_is_released() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    assert!(matches!(
        registry.execute(&mut doc, "MergeAllEdges"),
        Err(CommandError::NoObjectsSelected)
    ));
    let id = source(&mut doc);
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    let peer = doc.object(point).unwrap().clone();
    doc.select_objects_direct([id, point], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "MergeAllEdges").unwrap();
    assert_eq!(doc.object(point), Some(&peer));
    assert_eq!(doc.selected_object_count(), 2);
    doc.select_objects_direct([point], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let history = doc.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut doc, "MergeAllEdges"),
        Err(CommandError::UnsupportedMergeAllEdgesGeometry)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.undo_label(), history.as_deref());
    assert_eq!(doc.selected_object_count(), 0);
}

#[test]
fn replacement_without_merges_materializes_surfaces_and_records_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let surface = cube().faces()[0].surface().clone();
    let id = doc
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    assert!(doc.redo_label().is_some());
    registry
        .execute_postselected(&mut doc, "MergeAllEdges", Default::default())
        .unwrap();
    let after = doc.object(id).unwrap().geometry().clone();
    let Geometry::Brep(brep) = &after else {
        panic!("surface was not materialized")
    };
    assert_eq!(brep.faces()[0].surface(), &surface);
    assert_eq!(brep.edges().len(), 4);
    assert_eq!(doc.selected_object_count(), 0);
    assert_eq!(doc.undo_label(), Some("MergeAllEdges"));
    assert_eq!(doc.redo_label(), None);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(
        doc.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
    assert_eq!(doc.selected_object_count(), 0);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id).unwrap().geometry(), &after);
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(doc.selected_object_count(), 0);

    // Already-clean geometry still represents an object replacement, not an
    // empty transaction that leaves a previous redo branch alive.
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "MergeAllEdges").unwrap();
    assert_eq!(doc.object(id).unwrap().geometry(), &after);
    assert_eq!(doc.redo_label(), None);
    assert_eq!(doc.undo_label(), Some("MergeAllEdges"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id).unwrap().geometry(), &after);
    assert!(doc.is_selected(id));
}

#[test]
fn syntax_errors_leave_geometry_selection_and_history_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = source(&mut doc);
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        registry.execute(&mut doc, "MergeAllEdges Angle=1"),
        Err(CommandError::Usage("MergeAllEdges"))
    ));
    assert_eq!(format!("{doc:?}"), before);
    let prompt = registry
        .object_selection_prompt("MergeAllEdges")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::SurfaceComponents);
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::OptionsDuringSelection
    );
    assert!(prompt.options.is_empty());
    assert!(registry.object_selection_prompt("MergeAllEdges 1").is_err());
}

#[test]
fn command_angle_tracks_document_settings_with_point_one_and_one_degree_limits() {
    for (document_degrees, kink_degrees, merged) in [
        (1e-8_f64, 0.09_f64, true),
        (1e-8, 0.11, false),
        (0.5, 0.49, true),
        (0.5, 0.51, false),
        (5., 0.99, true),
        (5., 1.01, false),
    ] {
        let tolerance = Tolerance::try_new(1e-9, 1e-12, document_degrees.to_radians()).unwrap();
        let mut doc = Document::new(tolerance);
        let a = kink_degrees.to_radians();
        let curve = NurbsCurve::try_new(
            1,
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [1. + a.cos(), a.sin(), 0.],
                [3., 3., 0.],
                [0., 3., 0.],
                [0., 0., 0.],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            vec![0., 0., 1., 2., 3., 4., 5., 5.],
        )
        .unwrap();
        let source = Brep::try_planar_face(&curve, tolerance)
            .unwrap()
            .try_split_edges_at_parameters(&[(0, vec![1.])], tolerance)
            .unwrap();
        let id = doc.add_geometry(Geometry::Brep(source)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        CommandRegistry::with_builtins()
            .execute(&mut doc, "MergeAllEdges")
            .unwrap();
        let Geometry::Brep(brep) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(
            brep.edges().len(),
            if merged { 1 } else { 2 },
            "doc {document_degrees}, kink {kink_degrees}"
        );
        assert_eq!(brep.faces().len(), 1);
    }
}
