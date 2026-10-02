use super::*;

fn tube(document: &mut Document) -> ObjectId {
    let brep = Brep::try_tube(
        CommandContext::default().construction_plane,
        [2., 5.],
        8.,
        document.tolerance(),
    )
    .unwrap();
    document
        .add_geometry_with_attributes(
            Geometry::Brep(brep),
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_name("Source")
                .with_object_color(ColorRgb::new(10, 30, 50)),
        )
        .unwrap()
}

#[test]
fn retained_wall_has_defaults_and_source_metadata_and_order_roundtrip_in_one_undo() {
    let mut document = Document::default();
    let id = tube(&mut document);
    let peer = document
        .add_geometry(Geometry::Point(Point3::try_new(20., 30., 40.).unwrap()))
        .unwrap();
    let group = document.add_group(Some("Source".into()), [id]).unwrap();
    let layer = document
        .add_layer("Trim objects", ColorRgb::new(70, 80, 90))
        .unwrap();
    document.set_current_layer(layer).unwrap();
    document.clear_history().unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let snapshot = format!("{document:?}");
    let pick = UntrimHolesSelection::prepare(
        &document,
        id,
        UntrimHolesComponent::Edge(3),
        UntrimHolesOptions {
            keep_trim_objects: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(format!("{document:?}"), snapshot);
    let result = pick.commit(&mut document).unwrap();
    assert!(result.changed);
    assert_eq!(result.removed_faces, 1);
    assert_eq!(result.removed_openings, 2);
    assert_eq!(result.retained.len(), 1);
    assert_eq!(
        document.object(id).unwrap().attributes(),
        before[0].attributes()
    );
    assert_eq!(document.object(id).unwrap().group_ids(), &[group]);
    let retained = document.object(result.retained[0]).unwrap();
    assert_eq!(retained.attributes().layer_id(), layer);
    assert_eq!(retained.attributes().name(), None);
    assert_eq!(
        retained.attributes().color_source(),
        viboceros_document::ObjectColorSource::Layer
    );
    assert!(retained.group_ids().is_empty());
    assert!(!document.is_selected(retained.id()));
    assert!(matches!(retained.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1));
    let after = document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(
        after.iter().map(|object| object.id()).collect::<Vec<_>>(),
        [peer, result.retained[0], id]
    );
    assert_eq!(document.undo_label(), Some("UntrimHoles"));
    document.undo().unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!document.can_undo());
    document.redo().unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn invalid_options_components_and_filtered_no_ops_preserve_document_redo_and_preferences() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let id = tube(&mut document);
    registry.execute(&mut document, "Point 7,8,9").unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    for tail in [
        "",
        "x",
        "3 unknown=Yes",
        "3 All=Maybe",
        "3 All=Yes All=No",
        "3 MaximumEdgeLength=-1",
        "3 MaximumEdgeLength=NaN",
        "3 MaximumEdgeLength=inf",
        "3 KeepTrimObjects=Yes extra",
        "20",
        "-1",
    ] {
        assert!(
            registry
                .execute(&mut document, &format!("UntrimHoles {id} {tail}"))
                .is_err(),
            "{tail}"
        );
        assert_eq!(format!("{document:?}"), before);
    }
    registry
        .execute(
            &mut document,
            &format!("UntrimHoles {id} 3 MaximumEdgeLength=1 KeepTrimObjects=No"),
        )
        .unwrap();
    assert_eq!(format!("{document:?}"), before);
    registry
        .execute(
            &mut document,
            &format!("UntrimHoles {id} 0 MaximumEdgeLength=0"),
        )
        .unwrap();
    assert_eq!(format!("{document:?}"), before);
    // Accepted prompt choices persist without changing the model or Undo.
    registry
        .accept_object_selection_input("UntrimHoles All=Yes KeepTrimObjects=Yes")
        .unwrap();
    assert_eq!(format!("{document:?}"), before);
    registry
        .execute(&mut document, &format!("UntrimHoles {id} 2"))
        .unwrap();
    assert_eq!(document.objects().len(), 2);
    document.undo().unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut document, &format!("UntrimHoles {id} 2"))
        .unwrap();
    // A fresh registry still interprets component 2 as an outer edge.
    assert_eq!(document.objects().len(), 1);
    assert!(document.can_redo());
}

#[test]
fn stale_geometry_tolerance_and_restricted_sources_cannot_commit_a_prepared_pick() {
    for change in 0..4 {
        let mut document = Document::default();
        let id = tube(&mut document);
        let selection = UntrimHolesSelection::prepare(
            &document,
            id,
            UntrimHolesComponent::Face(2),
            UntrimHolesOptions {
                all: true,
                keep_trim_objects: true,
                ..Default::default()
            },
        )
        .unwrap();
        match change {
            0 => {
                CommandRegistry::with_builtins()
                    .execute(&mut document, "Tolerance Absolute=0.01")
                    .unwrap();
            }
            1 => {
                document
                    .replace_object_geometries([(
                        id,
                        selection
                            .source
                            .clone()
                            .transformed(
                                AffineTransform3::from_translation(
                                    Vector3::try_new(1., 2., 3.).unwrap(),
                                ),
                                document.tolerance(),
                            )
                            .unwrap(),
                    )])
                    .unwrap();
            }
            2 => {
                document.set_objects_visibility([id], false).unwrap();
            }
            _ => {
                document.delete_objects([id]).unwrap();
            }
        }
        let before = format!("{document:?}");
        assert!(matches!(
            selection.commit(&mut document),
            Err(CommandError::UntrimHolesStale)
        ));
        assert_eq!(format!("{document:?}"), before);
    }
}
