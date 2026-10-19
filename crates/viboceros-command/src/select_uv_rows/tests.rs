use super::*;
use viboceros_document::ControlPointId;
fn source() -> (Document, ObjectId, ObjectId) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/select_uv_rows/surface_U.3dm");
    let (mut doc, _, _, _) = open_3dm_with_views(path.to_str().unwrap()).unwrap();
    let id = doc.objects().find(|o| accepts(o.geometry())).unwrap().id();
    let peer = doc
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::Point(_)))
        .unwrap()
        .id();
    doc.clear_selection();
    doc.clear_history().unwrap();
    let marker = doc
        .add_geometry(Geometry::Point(Point3::try_new(30., 30., 30.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    assert!(doc.object(marker).is_none());
    doc.enable_control_points([id]).unwrap();
    doc.select_control_points(
        [ControlPointId {
            object: id,
            index: 0,
        }],
        SelectionMode::Add,
    )
    .unwrap();
    doc.select_objects_direct([id, peer], SelectionMode::Add)
        .unwrap();
    (doc, id, peer)
}
#[derive(Debug, PartialEq)]
struct State {
    objects: Vec<ObjectId>,
    grips: Vec<(ControlPointId, Point3, bool)>,
    undo: bool,
    redo: bool,
}
fn state(doc: &Document) -> State {
    State {
        objects: doc.selected_objects().map(|o| o.id()).collect(),
        grips: doc.control_points().collect(),
        undo: doc.can_undo(),
        redo: doc.can_redo(),
    }
}
#[test]
fn malformed_batch_is_atomic_and_registry_preferences_preserve_history() {
    let (mut doc, id, peer) = source();
    let registry = CommandRegistry::with_builtins();
    let before = state(&doc);
    let original = doc.object(id).unwrap().clone();
    for command in [
        "SelUV",
        "SelUV Points 1,1,0.25 Direction=Sideways",
        "SelUV Points 1,1,0.25 HideOthers=Maybe",
        "SelUV Points 1,1,0.25 Delete",
        "SelUV Points NaN,0,0",
        "SelUV Points 0,0,0,0",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(state(&doc), before, "{command}");
        assert_eq!(registry.select_uv_options(), Options::default());
        assert_eq!(doc.object(id), Some(&original));
    }
    registry
        .execute(
            &mut doc,
            "_SelUV Points Direction=Both Toggle HideOthers=Yes w1,1,0.25",
        )
        .unwrap();
    assert_eq!(
        doc.selected_control_points()
            .map(|(g, _)| g.index)
            .collect::<Vec<_>>(),
        [1, 3, 4, 5, 7]
    );
    assert!(!doc.is_selected(id));
    assert!(doc.is_selected(peer));
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
    assert_eq!(
        registry.select_uv_options(),
        Options {
            direction: SurfaceKnotDirection::Both,
            hide_others: true
        }
    );
    doc.select_objects_direct([id], SelectionMode::Add).unwrap();
    registry.execute(&mut doc, "SelUV Points").unwrap();
    assert_eq!(doc.selected_control_points().count(), 0);
    assert!(doc.control_point_locations(id).is_some());
    assert_eq!(doc.object(id), Some(&original));
    assert!(doc.can_redo());
}
#[test]
fn unavailable_sources_are_rejected_without_releasing_existing_picks() {
    let (mut doc, id, peer) = source();
    let before = state(&doc);
    assert!(Session::start(&mut doc, peer, false).is_err());
    assert_eq!(state(&doc), before);
    let session = Session::start(&mut doc, id, true).unwrap();
    doc.disable_control_points();
    let before = state(&doc);
    assert!(
        session
            .pick(
                &mut doc,
                Point3::try_new(0., 0., 0.).unwrap(),
                Options::default()
            )
            .is_err()
    );
    assert_eq!(state(&doc), before);
}
