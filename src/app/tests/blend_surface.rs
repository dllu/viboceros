use super::boolean_intersection::enter;
use super::*;
use crate::viewport::EdgePick;

fn pair() -> (VibocerosApp, [ObjectId; 2]) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 1,0,0 1,2,0 0,2,0");
    enter(&mut app, "SrfPt 3,0,1 4,0,1 4,2,1 3,2,1");
    let ids = app
        .document
        .objects()
        .map(|o| o.id())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    app.document.clear_history().unwrap();
    (app, ids)
}
fn pick(app: &mut VibocerosApp, id: ObjectId, edge: usize) {
    app.accept_blend_edges(vec![EdgePick { object: id, edge }]);
}
#[test]
fn edge_picks_preview_options_cancel_and_accept_use_atomic_history() {
    let (mut app, ids) = pair();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "BlendSrf");
    assert!(!app.command_line_idle());
    assert!(app.viewport_object_filter().is_none());
    pick(&mut app, ids[0], 1);
    pick(&mut app, ids[1], 3);
    assert_eq!(
        app.blend_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        3
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    enter(&mut app, "Continuity1=G2 Continuity2=G2");
    let p = app.blend_surface_prompt.as_ref().unwrap();
    let Geometry::Brep(b) = p
        .scene()
        .unwrap()
        .selected_objects()
        .next()
        .unwrap()
        .geometry()
    else {
        panic!()
    };
    assert_eq!(b.faces()[0].surface().degree_v(), 5);
    app.cancel_current_prompt_or_selection();
    assert!(app.blend_surface_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "BlendSrf");
    pick(&mut app, ids[0], 1);
    pick(&mut app, ids[1], 3);
    enter(&mut app, "Trim1=0.2 Trim2=0.3 Join=Yes");
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.blend_surface_prompt
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        1
    );
    enter(&mut app, "");
    assert!(app.blend_surface_prompt.is_none());
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
#[test]
fn ambiguous_edge_choices_and_stale_sources_are_handled_without_document_edits() {
    let (mut app, ids) = pair();
    enter(&mut app, "BlendSrf");
    app.accept_blend_edges(vec![
        EdgePick {
            object: ids[0],
            edge: 0,
        },
        EdgePick {
            object: ids[0],
            edge: 1,
        },
    ]);
    enter(&mut app, "2");
    pick(&mut app, ids[1], 3);
    assert!(app.blend_surface_prompt.as_ref().unwrap().scene().is_some());
    app.document.delete_object(ids[0]).unwrap();
    app.validate_blend_surface();
    assert!(app.blend_surface_prompt.is_none());
    assert_eq!(app.document.objects().len(), 1);
}
