use super::*;
use std::io::Read;
use viboceros_document::{ObjectId, SelectionMode};
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn fixture() -> (VibocerosApp, ObjectId) {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(
        include_bytes!(
            "../../../tools/rhino_oracle/observations/subd_nurbs_packs_discovery.json.gz"
        )
        .as_slice(),
    )
    .read_to_end(&mut raw)
    .unwrap();
    let data: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let row = &data["results"][0]["value"]["before"][0];
    let bytes = row["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.3dm");
    std::fs::write(&path, bytes).unwrap();
    let model = viboceros_io::read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    let viboceros_io::ThreeDmGeometry::SubD(s) = model.objects[0].geometry.clone() else {
        panic!()
    };
    let mut app = test_app();
    let id = app.document.add_geometry(Geometry::SubD(s)).unwrap();
    app.document.clear_history().unwrap();
    (app, id)
}
fn colors(app: &mut VibocerosApp, id: ObjectId) -> Option<std::sync::Arc<[[u8; 3]]>> {
    app.face_pack_preview
        .view(
            &app.document,
            app.object_prompt.as_ref().map(|p| &p.description),
        )
        .unwrap()
        .and_then(|v| v.colors(app.document.object(id).unwrap()))
}
fn groups(colors: &[[u8; 3]]) -> usize {
    colors
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}
#[test]
fn nurbs_pack_actions_scope_hidden_controls_and_accept_one_conversion() {
    for pre in [false, true] {
        let (mut app, id) = fixture();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        if pre {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "ToNURBS");
        if !pre {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        }
        enter(&mut app, "SubDOptions");
        enter(&mut app, "ShowFacePacks");
        assert_eq!(groups(&colors(&mut app, id).unwrap()), 2);
        enter(&mut app, "ForceUpdateFacePacks");
        assert_eq!(groups(&colors(&mut app, id).unwrap()), 6);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!app.document.can_undo());
        enter(&mut app, "ForceUpdateFacePacks");
        assert_eq!(groups(&colors(&mut app, id).unwrap()), 2);
        enter(&mut app, "ForceUpdateFacePacks=Yes");
        enter(&mut app, "Faces=Unpacked");
        assert!(colors(&mut app, id).is_none());
        let prompt = app.object_prompt.as_ref().unwrap().description.clone();
        assert!(!prompt.display_command_line().contains("ShowFacePacks"));
        enter(&mut app, "ForceUpdateFacePacks");
        assert_eq!(app.object_prompt.as_ref().unwrap().description, prompt);
        enter(&mut app, "");
        enter(&mut app, "");
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.undo_label(), Some("ToNURBS"));
        let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(
            s.face_packs()
                .unwrap()
                .iter()
                .map(|p| p.unwrap().id)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            6
        );
        let output = app
            .document
            .objects()
            .find_map(|o| {
                if let Geometry::Brep(b) = o.geometry() {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(output.faces().len(), 24);
        let after = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "ToNURBS");
        enter(&mut app, "SubDOptions");
        enter(&mut app, "Faces=Packed");
        assert!(
            app.object_prompt.as_ref().unwrap().description.menus[0]
                .options
                .iter()
                .all(|o| !o.value)
        );
        app.cancel_interactive_command(true);
    }
}
#[test]
fn nurbs_pack_preview_cancels_without_consuming_redo() {
    let (mut app, id) = fixture();
    app.document
        .add_geometry(Geometry::Point(point(30., 0., 0.)))
        .unwrap();
    app.document.undo().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ToNURBS");
    enter(&mut app, "SubDOptions");
    enter(&mut app, "ShowFacePacks=Yes ForceUpdateFacePacks=Yes");
    assert_eq!(groups(&colors(&mut app, id).unwrap()), 6);
    app.cancel_interactive_command(true);
    assert!(
        app.face_pack_preview
            .view(&app.document, None)
            .unwrap()
            .is_none()
    );
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
