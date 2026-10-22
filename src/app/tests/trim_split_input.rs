use super::*;

#[test]
fn interactive_curve_seam_uses_one_location_pick_on_the_selected_closed_curve() {
    let mut app = test_app();
    app.execute_command("Circle 0,0 5");
    let source = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("CrvSeam"));
    assert_eq!(app.active_command, Some(InteractiveCommand::CrvSeam));
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("selected closed curve")
    );
    app.accept_drafting_point(point(0.0, 5.0, 0.0));

    assert_eq!(app.active_command, None);
    let Geometry::Circle(relocated) = app.document.object(source).unwrap().geometry() else {
        panic!("seam relocation must retain the analytic circle")
    };
    assert!(
        relocated
            .evaluate(*relocated.domain().start())
            .unwrap()
            .is_near(point(0.0, 5.0, 0.0), app.document.tolerance(),)
    );
    assert!(app.document.is_selected(source));
    assert_eq!(app.document.undo_label(), Some("CrvSeam"));
    assert!(!app.try_start_interactive_command("CrvSeam Parameter=1"));

    app.document.clear_selection();
    assert!(app.try_start_interactive_command("CrvSeam"));
    assert_eq!(app.active_command, None);
    assert!(app.command_log.back().unwrap().contains("no objects"));
}

#[test]
fn interactive_surface_seam_uses_one_location_pick_and_direction_options() {
    let mut app = test_app();
    app.execute_command("Sphere 0,0,0 5");
    let source_id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source_id, viboceros_document::SelectionMode::Replace)
        .unwrap();
    let Geometry::NurbsSurface(source) = app.document.object(source_id).unwrap().geometry() else {
        panic!("Sphere must create an exact NURBS surface")
    };
    let source = source.clone();
    let u = source.parameter_at_u(0.37).unwrap();
    let v = source.parameter_at_v(0.43).unwrap();
    let pick = source.evaluate(u, v).unwrap();

    assert!(app.try_start_interactive_command("SrfSeam"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SrfSeam { direction: None })
    );
    assert!(app.command_log.back().unwrap().contains("closed surface"));
    app.accept_component_face_hit(source_id, 0, Some(pick));

    assert_eq!(app.active_command, None);
    let Geometry::NurbsSurface(relocated) = app.document.object(source_id).unwrap().geometry()
    else {
        panic!("expected an interactively edited NURBS surface")
    };
    assert!(
        app.document
            .tolerance()
            .approx_eq(*relocated.domain_u().start(), u)
    );
    assert_eq!(relocated.domain_v(), source.domain_v());
    assert!(app.document.is_selected(source_id));
    assert_eq!(app.document.undo_label(), Some("SrfSeam"));

    assert!(app.try_start_interactive_command("SrfSeam Direction=Both"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SrfSeam {
            direction: Some(InteractiveIsocurveDirection::Both),
        })
    );
    app.cancel_interactive_command(false);
    assert!(!app.try_start_interactive_command("SrfSeam Parameter=1 Direction=U"));
    assert!(!app.try_start_interactive_command("SrfSeam Direction=Sideways"));

    app.document.clear_selection();
    assert!(app.try_start_interactive_command("SrfSeam"));
    assert_eq!(app.active_command, None);
    assert!(app.command_log.back().unwrap().contains("no objects"));
}

#[test]
fn interactive_extend_picks_a_source_end_among_selected_boundary_curves() {
    let mut app = test_app();
    app.execute_command("Line 0,0 5,0");
    let source_id = app.document.objects().next().unwrap().id();
    app.execute_command("Line 10,-5 10,5");
    let boundary_id = app
        .document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap()
        .id();
    app.document
        .select_objects(
            [source_id, boundary_id],
            viboceros_document::SelectionMode::Replace,
        )
        .unwrap();

    assert!(app.try_start_interactive_command("Extend Type=Line Join=Merge"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Extend {
            style: InteractiveCurveExtensionStyle::Line,
            join: InteractiveCurveExtensionJoin::Merge,
        })
    );
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("source curve near the end")
    );
    app.accept_drafting_point(point(5.0, 0.0, 0.0));

    assert_eq!(app.active_command, None);
    let Geometry::NurbsCurve(extended) = app.document.object(source_id).unwrap().geometry() else {
        panic!("interactive Extend must retain exact NURBS geometry")
    };
    assert!(
        app.document
            .tolerance()
            .approx_eq(*extended.domain().end(), 10.0)
    );
    assert!(
        extended
            .evaluate(*extended.domain().end())
            .unwrap()
            .is_near(point(10.0, 0.0, 0.0), app.document.tolerance())
    );
    assert_eq!(
        app.document.object(boundary_id).unwrap().geometry(),
        &Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(10.0, -5.0, 0.0),
                point(10.0, 5.0, 0.0),
                app.document.tolerance(),
            )
            .unwrap()
        )
    );
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("Extend"));
    assert!(!app.try_start_interactive_command("Extend Length=2"));
    assert!(!app.try_start_interactive_command("Extend Type=Bezier"));
}

#[test]
fn interactive_extend_surface_uses_a_boundary_pick_and_its_path_parameter() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 10,0,0 20,10,0 0,10,0");
    let source_id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source_id, viboceros_document::SelectionMode::Replace)
        .unwrap();
    let Geometry::NurbsSurface(source) = app.document.object(source_id).unwrap().geometry() else {
        panic!("SrfPt must create an exact NURBS surface")
    };
    let source = source.clone();
    let v = source.parameter_at_v(0.25).unwrap();
    let pick = source.evaluate(*source.domain_u().end(), v).unwrap();
    let expected = source
        .try_shrunk_by_length(
            SurfaceExtensionEdge::East,
            2.0,
            Some(v),
            app.document.tolerance(),
        )
        .unwrap();

    assert!(app.try_start_interactive_command("ExtendSrf Distance=-2 Type=Line Merge=Yes"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtendSrf {
            distance: -2.0,
            smooth: false,
            merge: true,
        })
    );
    assert!(app.command_log.back().unwrap().contains("natural edge"));
    app.accept_component_face_hit(source_id, 0, Some(pick));

    assert_eq!(app.active_command, None);
    assert_eq!(
        app.document.object(source_id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected)
    );
    assert!(app.document.is_selected(source_id));
    assert_eq!(app.document.undo_label(), Some("ExtendSrf"));
    assert!(!app.try_start_interactive_command("ExtendSrf Edge=East Distance=2"));
    assert!(!app.try_start_interactive_command("ExtendSrf Distance=2 Type=Natural"));
    assert!(!app.try_start_interactive_command("ExtendSrf Distance=-2 Merge=No"));

    app.document.clear_selection();
    assert!(app.try_start_interactive_command("ExtendSrf Distance=2"));
    assert_eq!(app.active_command, None);
    assert!(app.command_log.back().unwrap().contains("no objects"));
}

#[test]
fn interactive_subcurve_uses_two_directed_curve_picks_and_copy_option() {
    let mut app = test_app();
    app.execute_command("Line 0,0 10,0");
    let source_id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source_id, viboceros_document::SelectionMode::Replace)
        .unwrap();
    let source = app.document.object(source_id).unwrap().clone();

    assert!(app.try_start_interactive_command("SubCrv Copy=Yes"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SubCrv {
            start: None,
            copy: true,
            delete: false,
        })
    );
    assert!(app.command_log.back().unwrap().contains("subcurve start"));
    app.accept_drafting_point(point(8.0, 0.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SubCrv {
            start: Some(point(8.0, 0.0, 0.0)),
            copy: true,
            delete: false,
        })
    );
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("directed subcurve end")
    );
    app.accept_drafting_point(point(2.0, 0.0, 0.0));

    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().count(), 2);
    assert_eq!(app.document.object(source_id).unwrap(), &source);
    let output = app
        .document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap();
    let Geometry::Line(curve) = output.geometry() else {
        panic!("interactive SubCrv must retain the native line")
    };
    assert!(
        curve
            .evaluate(*curve.domain().start())
            .unwrap()
            .is_near(point(2.0, 0.0, 0.0), app.document.tolerance())
    );
    assert!(
        curve
            .evaluate(*curve.domain().end())
            .unwrap()
            .is_near(point(8.0, 0.0, 0.0), app.document.tolerance())
    );
    assert_eq!(app.document.undo_label(), Some("SubCrv"));

    assert!(!app.try_start_interactive_command("SubCrv Parameter=0.2,0.8"));
    assert!(!app.try_start_interactive_command("SubCrv Copy=Yes Copy=No"));
    app.document.clear_selection();
    assert!(app.try_start_interactive_command("SubCrv"));
    assert_eq!(app.active_command, None);
    assert!(app.subcurve_prompt.as_ref().unwrap().source.is_none());
    assert!(app.command_log.back().unwrap().contains("select one curve"));
}

#[test]
fn interactive_split_collects_curve_locations_until_enter() {
    let mut app = test_app();
    app.execute_command("Line 0,0 10,0");
    let source_id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source_id, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("Split"));
    assert_eq!(app.active_command, Some(InteractiveCommand::SplitCurve));
    assert!(app.command_log.back().unwrap().contains("split locations"));
    app.accept_drafting_point(point(4.0, 0.0, 0.0));
    app.accept_drafting_point(point(7.0, 0.0, 0.0));
    assert_eq!(app.active_command, Some(InteractiveCommand::SplitCurve));
    assert_eq!(
        app.curve_points,
        vec![point(4.0, 0.0, 0.0), point(7.0, 0.0, 0.0)]
    );
    app.finish_interactive_curve();

    assert_eq!(app.active_command, None);
    assert!(app.curve_points.is_empty());
    assert_eq!(app.document.objects().count(), 3);
    assert_eq!(app.document.selected_object_count(), 3);
    for object in app.document.objects() {
        let Geometry::Line(curve) = object.geometry() else {
            panic!("interactive Split must retain native line pieces")
        };
        let first_split = point(4.0, 0.0, 0.0);
        let second_split = point(7.0, 0.0, 0.0);
        let start = curve.evaluate(*curve.domain().start()).unwrap();
        let end = curve.evaluate(*curve.domain().end()).unwrap();
        assert!(
            start.is_near(first_split, app.document.tolerance())
                || start.is_near(second_split, app.document.tolerance())
                || end.is_near(first_split, app.document.tolerance())
                || end.is_near(second_split, app.document.tolerance())
        );
    }
    assert_eq!(app.document.undo_label(), Some("Split"));
    assert!(!app.try_start_interactive_command("Split Parameter=0.5"));

    app.document.clear_selection();
    assert!(app.try_start_interactive_command("Split"));
    assert_eq!(app.active_command, None);
    assert!(app.command_log.back().unwrap().contains("no objects"));
}

#[test]
fn interactive_cutting_object_split_uses_one_source_pick() {
    let mut app = test_app();
    app.execute_command("Line 0,0 10,0");
    app.execute_command("Line 3,-5 3,5");
    app.execute_command("Line 7,-5 7,5");
    app.execute_command("SelAll");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let source_id = ids[0];
    let cutter_ids = BTreeSet::from([ids[1], ids[2]]);

    assert!(app.try_start_interactive_command("Split CuttingObjects"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SplitCurveWithCutters)
    );
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("source curve or rectangular surface")
    );
    app.accept_drafting_point(point(1.0, 0.0, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.object(source_id).is_none());
    assert!(
        cutter_ids
            .iter()
            .all(|id| app.document.object(*id).is_some() && !app.document.is_selected(*id))
    );
    let domains = app
        .document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            geometry => panic!("interactive Split selected unexpected geometry {geometry:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(domains, vec![0.0..=3.0, 3.0..=7.0, 7.0..=10.0]);
    assert_eq!(app.document.undo_label(), Some("Split"));
    assert!(!app.try_start_interactive_command("Split CuttingObjects=1,0,0"));

    let mut no_selection = test_app();
    assert!(no_selection.try_start_interactive_command("Split _CuttingObjects"));
    assert_eq!(no_selection.active_command, None);
    assert!(
        no_selection
            .command_log
            .back()
            .unwrap()
            .contains("no objects")
    );
}

#[test]
fn interactive_cutting_object_split_accepts_a_surface_source_pick() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 10,0,0 10,10,0 0,10,0");
    app.execute_command("SrfPt 4,-5,-5 4,15,-5 4,15,5 4,-5,5");
    app.execute_command("SelAll");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let source_id = ids[0];
    let cutter_id = ids[1];

    assert!(app.try_start_interactive_command("Split CuttingObjects"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SplitCurveWithCutters)
    );
    app.accept_drafting_point(point(2.0, 5.0, 0.0));

    assert_eq!(app.active_command, None);
    assert!(app.document.object(source_id).is_none());
    assert!(app.document.object(cutter_id).is_some());
    assert!(!app.document.is_selected(cutter_id));
    assert_eq!(app.document.selected_object_count(), 2);
    assert!(app.document.selected_objects().all(|object| {
        matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1)
    }));
    assert_eq!(app.document.undo_label(), Some("Split"));
}

#[test]
fn interactive_isocurve_split_uses_hit_elevation_on_a_sloped_surface() {
    let mut clicked = test_app();
    clicked.execute_command("SrfPt 0,0,0 4,0,4 4,3,4 0,3,0");
    let source = clicked.document.objects().next().unwrap().id();
    clicked
        .document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();
    assert!(clicked.try_start_interactive_command("Split Isocurve Direction=V"));
    clicked.accept_component_face_hit(source, 0, Some(point(1.5, 2.0, 1.5)));
    assert_eq!(clicked.document.selected_object_count(), 2);
    let click_geometry = clicked
        .document
        .selected_objects()
        .map(|object| object.geometry().clone())
        .collect::<Vec<_>>();

    let mut exact = test_app();
    exact.execute_command("SrfPt 0,0,0 4,0,4 4,3,4 0,3,0");
    let source = exact.document.objects().next().unwrap().id();
    exact
        .document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();
    exact.execute_command("Split Isocurve=1.5,2,1.5 Direction=V");
    assert_eq!(
        click_geometry,
        exact
            .document
            .selected_objects()
            .map(|object| object.geometry().clone())
            .collect::<Vec<_>>()
    );

    exact.execute_command("Undo");
    exact
        .document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();
    exact.execute_command("Split Isocurve=1.5,2,0 Direction=V");
    assert_ne!(
        click_geometry,
        exact
            .document
            .selected_objects()
            .map(|object| object.geometry().clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn interactive_surface_isocurve_split_uses_one_location_pick() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 4,0,0 4,3,0 0,3,0");
    let source_id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source_id, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("Split _Isocurve Direction=_Both Shrink=_Yes"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::SplitSurfaceIsocurve {
            direction: InteractiveIsocurveDirection::Both,
            shrink: true,
        })
    );
    assert!(app.command_log.back().unwrap().contains("selected surface"));
    app.accept_component_face_hit(source_id, 0, Some(point(1.5, 2.0, 0.0)));

    assert_eq!(app.active_command, None);
    assert!(app.document.object(source_id).is_none());
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!(app.document.selected_object_count(), 4);
    assert!(app.document.objects().all(|object| {
        app.document.is_selected(object.id())
            && matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1)
    }));
    assert_eq!(app.document.undo_label(), Some("Split"));
    assert!(!app.try_start_interactive_command("Split Isocurve=1,2,0"));
    assert!(!app.try_start_interactive_command("Split Isocurve Direction=U Direction=V"));

    let mut unshrunk = test_app();
    unshrunk.execute_command("SrfPt 0,0,0 4,0,0 4,3,0 0,3,0");
    let unshrunk_source = unshrunk.document.objects().next().unwrap().id();
    unshrunk
        .document
        .select_object(unshrunk_source, viboceros_document::SelectionMode::Replace)
        .unwrap();
    assert!(unshrunk.try_start_interactive_command("Split Isocurve Shrink=No"));
    unshrunk.accept_drafting_point(point(1.5, 2.0, 0.0));
    assert!(unshrunk.document.object(unshrunk_source).is_none());
    assert_eq!(unshrunk.document.objects().count(), 2);
    assert!(unshrunk.document.objects().all(|object| {
        matches!(object.geometry(), Geometry::Brep(brep) if brep.faces().len() == 1)
    }));

    let face_id = unshrunk.document.objects().next().unwrap().id();
    unshrunk
        .document
        .select_object(face_id, viboceros_document::SelectionMode::Replace)
        .unwrap();
    assert!(unshrunk.try_start_interactive_command("Split Isocurve Direction=V Shrink=No"));
    unshrunk.accept_drafting_point(point(1.0, 1.0, 0.0));
    assert!(unshrunk.document.object(face_id).is_none());
    assert_eq!(unshrunk.document.objects().count(), 3);
}

#[test]
fn interactive_trim_removes_the_picked_interval_in_one_click() {
    let mut app = test_app();
    app.execute_command("Line 0,0 10,0");
    app.execute_command("Line 3,-5 3,5");
    app.execute_command("Line 7,-5 7,5");
    app.execute_command("SelAll");

    assert!(app.try_start_interactive_command("Trim"));
    assert_eq!(app.active_command, Some(InteractiveCommand::TrimCurve));
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("interval to remove")
    );
    app.accept_drafting_point(point(5.0, 0.0, 0.0));

    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!(app.document.selected_object_count(), 2);
    let mut domains = app
        .document
        .selected_objects()
        .map(|object| match object.geometry() {
            Geometry::Line(curve) => curve.domain(),
            _ => panic!("interactive Trim must retain native lines"),
        })
        .collect::<Vec<_>>();
    domains.sort_by(|left, right| left.start().total_cmp(right.start()));
    assert_eq!(domains, vec![0.0..=3.0, 7.0..=10.0]);
    assert_eq!(app.document.undo_label(), Some("Trim"));
    assert!(!app.try_start_interactive_command("Trim 5,0"));

    app.document.clear_selection();
    assert!(app.try_start_interactive_command("Trim"));
    assert_eq!(app.active_command, None);
    assert!(app.command_log.back().unwrap().contains("no objects"));
}

#[test]
fn interactive_isocurve_face_click_targets_one_selected_surface() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 2,0,0 2,2,0 0,2,0");
    let back = app.document.objects().next().unwrap().id();
    app.execute_command("SrfPt 0,0,10 2,0,10 2,2,10 0,2,10");
    let front = app
        .document
        .objects()
        .find(|object| object.id() != back)
        .unwrap()
        .id();
    app.document
        .select_objects_direct([back, front], viboceros_document::SelectionMode::Replace)
        .unwrap();
    assert!(app.try_start_interactive_command("ExtractIsocurve Direction=Both"));
    app.accept_isocurve_face_click(back, 0, point(0.5, 0.5, 10.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.selected_object_count(), 2);
    for object in app.document.selected_objects() {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("expected isocurve")
        };
        assert!(curve.evaluate(0.5).unwrap().z().abs() < 1e-10);
    }
    assert!(app.document.object(back).is_some());
    assert!(app.document.object(front).is_some());
}

#[test]
fn interactive_extract_isocurve_uses_one_surface_location_pick() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 4,0,0 4,3,0 0,3,0");
    let surface = app.document.objects().next().unwrap().id();
    app.document
        .select_object(surface, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("ExtractIsocurve Direction=Both IgnoreTrims=Yes"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtractIsocurve {
            direction: InteractiveIsocurveDirection::Both,
            ignore_trims: true,
        })
    );
    assert!(app.command_log.back().unwrap().contains("selected surface"));
    app.accept_drafting_point(point(1.5, 2.0, 0.0));

    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().len(), 3);
    assert!(!app.document.is_selected(surface));
    assert!(app.document.objects().skip(1).all(|object| {
        app.document.is_selected(object.id())
            && matches!(object.geometry(), Geometry::NurbsCurve(_))
    }));
    assert_eq!(app.document.undo_label(), Some("ExtractIsocurve"));
    assert!(!app.try_start_interactive_command("ExtractIsocurve Direction=Sideways"));

    app.execute_command("Undo");
    app.document
        .select_object(surface, viboceros_document::SelectionMode::Replace)
        .unwrap();
    app.command_input = "ExtractIsocurve ExtractAll Direction=Both".to_owned();
    app.run_command();
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().len(), 7);
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("Extracted 6 exact U/V isocurve(s) from 1 surface(s)")
    );
}
