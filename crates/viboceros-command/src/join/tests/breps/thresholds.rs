use super::*;

fn sheet(y: [f64; 2], tolerance: Tolerance) -> Geometry {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance,
    )
    .unwrap();
    Geometry::Brep(
        Brep::try_box(frame, [[0., 2.], y, [0., 5.]], tolerance)
            .unwrap()
            .sub_brep(&[4], tolerance)
            .unwrap(),
    )
}

#[test]
fn selection_mode_changes_gap_acceptance_without_changing_copy_or_history_policy() {
    for absolute in [0.0001, 0.001, 0.01] {
        for origin in [0., 3.] {
            // Strictly interior/exterior probes avoid fitting Rhino's observed
            // one-endpoint rounding behavior at the exact 1.8/2.1 cutoffs.
            for ratio in [1.75, 1.85, 2.05, 2.15] {
                for copy in [false, true] {
                    for post in [false, true] {
                        check(absolute, origin, ratio, copy, post);
                    }
                }
            }
        }
    }
}

#[test]
fn overflowing_selection_radius_is_rejected_without_geometry_or_history_edits() {
    for copy in [false, true] {
        for post in [false, true] {
            let mut document = Document::default();
            let ids = [
                document.add_geometry(faces(&[0], 5.)).unwrap(),
                document.add_geometry(faces(&[2], 5.)).unwrap(),
            ];
            document.set_tolerance(Tolerance::try_new(f64::MAX, 1e-12, 1e-10).unwrap());
            let registry = CommandRegistry::with_builtins();
            document
                .select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
            let before = document.objects().cloned().collect::<Vec<_>>();
            let undo = document.undo_label().map(str::to_owned);
            let error = execute(&registry, &mut document, copy, post).unwrap_err();
            assert!(
                matches!(
                    error,
                    CommandError::Geometry(GeometryError::InvalidBrepTopology {
                        context: "B-rep join distance"
                    })
                ),
                "{error:?}"
            );
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(document.undo_label(), undo.as_deref());
        }
    }
}

fn check(absolute: f64, origin: f64, ratio: f64, copy: bool, post: bool) {
    let mut document = Document::default();
    let tolerance = Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap();
    document.set_tolerance(tolerance);
    let gap = absolute * ratio;
    let originals = [
        sheet([origin - 3., origin], tolerance),
        sheet([origin + gap, origin + gap + 3.], tolerance),
    ];
    let ids = originals
        .clone()
        .map(|geometry| document.add_geometry(geometry).unwrap());
    document
        .set_object_names([
            (ids[0], Some("first".into())),
            (ids[1], Some("second".into())),
        ])
        .unwrap();
    let attributes = ids.map(|id| document.object(id).unwrap().attributes().clone());
    let peer = document
        .add_geometry(Geometry::Point(Point3::try_new(20., 0., 0.).unwrap()))
        .unwrap();
    let group = document.add_group(None, [ids[0], ids[1], peer]).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Point 30,0,0").unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    let redo = document.redo_label().map(str::to_owned);
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let before_members = document.group(group).unwrap().members().collect::<Vec<_>>();
    // Native stitching also admits the sum of the stored entity tolerances.
    // This fixture's exact edges inherit one model tolerance each.
    let cutoff = if post {
        2.1
    } else if cfg!(feature = "native-smlib") {
        2.
    } else {
        1.8
    };
    let joined = ratio < cutoff;
    let result = execute(&registry, &mut document, copy, post);
    if post && !joined {
        assert!(matches!(result, Err(CommandError::NothingJoined)));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.redo_label(), redo.as_deref());
        assert_eq!(
            document.group(group).unwrap().members().collect::<Vec<_>>(),
            before_members
        );
        assert!(document.is_selected(ids[0]));
        assert!(!document.is_selected(ids[1]));
        assert!(!document.is_selected(peer));
        return;
    }
    result.unwrap_or_else(|error| {
        panic!(
            "absolute={absolute} origin={origin} ratio={ratio} copy={copy} post={post}: {error:?}"
        )
    });
    let outputs = document
        .objects()
        .filter(|o| !ids.contains(&o.id()) && o.id() != peer)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), if joined { 1 } else { 2 });
    for (index, output) in outputs.iter().enumerate() {
        assert_eq!(output.attributes(), &attributes[index]);
        assert_eq!(output.group_ids(), [group]);
        assert_eq!(document.is_selected(output.id()), !post);
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("B-rep output")
        };
        assert!(brep.is_manifold());
        assert_eq!(brep.faces().len(), if joined { 2 } else { 1 });
        if joined {
            for (face, original) in brep.faces().iter().zip(&originals) {
                let Geometry::Brep(original) = original else {
                    unreachable!()
                };
                assert_eq!(face.surface(), original.faces()[0].surface());
            }
            let seam = origin + gap * 0.5;
            #[cfg(not(feature = "native-smlib"))]
            assert_eq!(
                brep.vertices()
                    .iter()
                    .filter(|v| (v.point().y() - seam).abs() < 1e-12)
                    .count(),
                2
            );
            #[cfg(feature = "native-smlib")]
            {
                // SMLib retains a candidate vertex rather than averaging the
                // two boundaries. Both seam vertices stay inside the gap.
                let vertices = brep
                    .vertices()
                    .iter()
                    .filter(|v| (v.point().y() - seam).abs() <= gap * 0.5 + 1e-12)
                    .collect::<Vec<_>>();
                assert_eq!(vertices.len(), 2);
                assert!(vertices.iter().all(|v| v.tolerance() >= gap * 0.5));
            }
        } else {
            #[cfg(not(feature = "native-smlib"))]
            assert_eq!(output.geometry(), &originals[index]);
            #[cfg(feature = "native-smlib")]
            {
                // Native import/export gives untouched outputs model-sized
                // uncertainty and edge-aligned UV parameter domains. Their
                // spatial definitions must still be exactly unchanged.
                let Geometry::Brep(original) = &originals[index] else {
                    unreachable!()
                };
                assert_eq!(brep.vertices().len(), original.vertices().len());
                for (vertex, before) in brep.vertices().iter().zip(original.vertices()) {
                    assert_eq!(vertex.point(), before.point());
                    assert!(vertex.tolerance() >= before.tolerance());
                }
                assert_eq!(brep.edges().len(), original.edges().len());
                for (edge, before) in brep.edges().iter().zip(original.edges()) {
                    assert_eq!(edge.vertices(), before.vertices());
                    assert_eq!(edge.curve(), before.curve());
                    assert!(edge.tolerance() >= before.tolerance());
                }
                assert_eq!(brep.faces()[0].surface(), original.faces()[0].surface());
            }
        }
    }
    for (index, id) in ids.into_iter().enumerate() {
        assert_eq!(document.object(id).is_some(), copy);
        assert_eq!(document.is_selected(id), copy);
        if copy {
            assert_eq!(document.object(id).unwrap().geometry(), &originals[index]);
            assert_eq!(
                document.object(id).unwrap().attributes(),
                &attributes[index]
            );
        }
    }
    assert!(!document.is_selected(peer));
    let after = document.objects().cloned().collect::<Vec<_>>();
    let after_members = document.group(group).unwrap().members().collect::<Vec<_>>();
    for _ in 0..2 {
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            document.group(group).unwrap().members().collect::<Vec<_>>(),
            before_members
        );
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
        assert_eq!(
            document.group(group).unwrap().members().collect::<Vec<_>>(),
            after_members
        );
    }
}
