use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> crate::viewport::ComponentPick {
    let b = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 10.]; 3],
        app.document.tolerance(),
    )
    .unwrap();
    crate::viewport::ComponentPick {
        object: app.document.add_geometry(Geometry::Brep(b)).unwrap(),
        kind: viboceros_command::ComponentSelectionKind::BrepEdge,
        index: 0,
    }
}
fn pick(app: &mut VibocerosApp, pick: crate::viewport::ComponentPick, preselection: bool) {
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![pick],
        preselection,
        modifiers: Default::default(),
    });
}
#[test]
fn chamfer_getter_units_preselection_options_and_cancel_preserve_source() {
    let mut app = test_app();
    let p = source(&mut app);
    pick(&mut app, p, true);
    let original = app.document.object(p.object).unwrap().clone();
    enter(&mut app, "ChamferEdge");
    assert_eq!(app.edge_fillet_prompt.unwrap().name(), "ChamferEdge");
    assert!(!app.command_line_idle());
    enter(&mut app, "Distance");
    enter(&mut app, "0.1cm");
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap(),
        vec![p]
    );
    enter(&mut app, "RailType=RollingBall Distance=0");
    assert!(app.edge_fillet_prompt.is_some());
    enter(&mut app, "Cancel");
    assert!(app.edge_fillet_prompt.is_none());
    assert_eq!(app.document.object(p.object), Some(&original));
}
#[cfg(not(feature = "native-smlib"))]
#[test]
fn chamfer_portable_failure_keeps_getter_and_picks() {
    let mut app = test_app();
    let p = source(&mut app);
    enter(&mut app, "ChamferEdge 1");
    pick(&mut app, p, false);
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.edge_fillet_prompt.is_some());
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap(),
        vec![p]
    );
    assert_eq!(format!("{:?}", app.document), before);
}
#[cfg(feature = "native-smlib")]
#[test]
fn chamfer_failure_can_be_repaired_and_committed_as_one_history_step() {
    let mut app = test_app();
    let p = source(&mut app);
    let original = app.document.object(p.object).unwrap().clone();
    enter(&mut app, "ChamferEdge 100");
    pick(&mut app, p, false);
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert!(app.edge_fillet_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Distance=1 RailType=RollingBall");
    enter(&mut app, "");
    assert!(app.edge_fillet_prompt.is_none());
    let Geometry::Brep(b) = app.document.object(p.object).unwrap().geometry() else {
        panic!()
    };
    assert!((b.signed_volume(app.document.tolerance()).unwrap() - 995.).abs() < 1e-6);
    assert_eq!(app.document.undo_label(), Some("ChamferEdge"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(p.object), Some(&original));
    enter(&mut app, "Redo");
    assert_ne!(app.document.object(p.object), Some(&original));
}
