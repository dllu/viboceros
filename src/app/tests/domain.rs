use super::*;

#[test]
fn domain_postselection_reports_curve_without_model_edits() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 3,4,0");
    let id = app.document.objects().next().unwrap().id();
    app.command_input = "Domain".into();
    app.run_command();
    assert!(app.object_prompt.is_some());
    app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
    let before = format!("{:?}", app.document);
    assert!(app.try_continue_object_prompt(""));
    assert!(app.object_prompt.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.command_log.back().unwrap(), "Curve domain = [0,5]");
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn domain_polysurface_transitions_to_face_pick_for_pre_and_postselection() {
    use viboceros_geometry::{Brep, NurbsSurface};
    for preselected in [false, true] {
        let mut app = test_app();
        let parts = [0., 10.].map(|z| {
            Brep::try_surface_face(
                NurbsSurface::try_bilinear([
                    point(0., 0., z),
                    point(1., 0., z),
                    point(1., 1., z),
                    point(0., 1., z),
                ])
                .unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap()
        });
        let id = app
            .document
            .add_geometry(Geometry::Brep(
                Brep::try_combine(parts.to_vec(), Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        if preselected {
            app.document
                .select_object(id, viboceros_document::SelectionMode::Replace)
                .unwrap();
        }
        app.command_input = "Domain".into();
        app.run_command();
        if !preselected {
            assert!(app.object_prompt.is_some());
            app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
            let selected = format!("{:?}", app.document);
            assert!(app.try_continue_object_prompt(""));
            assert_eq!(format!("{:?}", app.document), selected);
        }
        assert!(app.object_prompt.is_none());
        assert_eq!(app.active_command, Some(InteractiveCommand::DomainFace));
        let before = format!("{:?}", app.document);
        assert!(app.accept_drafting_point(point(0.5, 0.5, 10.)));
        assert!(app.active_command.is_none());
        assert_eq!(
            app.command_log.back().unwrap(),
            "Face 1: U domain = [0,1]; V domain = [0,1]"
        );
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_start_interactive_command("Domain"));
        app.cancel_interactive_command(true);
        assert_eq!(format!("{:?}", app.document), before);
    }
}
