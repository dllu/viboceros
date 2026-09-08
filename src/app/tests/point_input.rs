use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}

#[test]
fn interpolation_close_actions_preserve_degree_and_knot_spacing() {
    for (action, closure) in [("_Close", "Smooth"), ("sHaRp", "Sharp")] {
        for options in ["Degree=1 Knots=Uniform", "Degree=3 Knots=SqrtChrd"] {
            let mut app = test_app();
            enter(&mut app, &format!("InterpCrv {options}"));
            for input in ["0", "3,0,0", "4,2,1", "0,4,0"] {
                enter(&mut app, input);
            }
            let last = app.last_point;
            enter(&mut app, action);
            assert!(app.active_command.is_none());
            assert!(app.command_input.is_empty());
            assert_eq!(app.last_point, last);
            let geometry = app.document.objects().next().unwrap().geometry().clone();
            let mut reference = test_app();
            enter(
                &mut reference,
                &format!("InterpCrv 0,0,0 3,0,0 4,2,1 0,4,0 {options} Close={closure}"),
            );
            assert_eq!(
                &geometry,
                reference.document.objects().next().unwrap().geometry()
            );
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().len(), 0);
            assert!(!app.document.can_undo());
            enter(&mut app, "Redo");
            assert_eq!(&geometry, app.document.objects().next().unwrap().geometry());
        }
    }
}

#[test]
fn rejected_interpolation_closure_preserves_tangents_points_and_redo() {
    for action in ["Close", "Sharp"] {
        let mut app = test_app();
        for input in [
            "Point 9,9,9",
            "Undo",
            "InterpCrv Knots=Uniform StartTangent=1,2,0 EndTangent=0,1,0",
            "0",
            "2,3,0",
            "4,0,0",
        ] {
            enter(&mut app, input);
        }
        let active = app.active_command;
        let points = app.curve_points.clone();
        let plane = app.drafting_plane;
        let last = app.last_point;
        let document = format!("{:?}", app.document);
        let preview = app.curve_draft_preview().unwrap();
        enter(&mut app, action);
        assert_eq!(app.active_command, active);
        assert_eq!(app.curve_points, points);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.last_point, last);
        assert_eq!(app.command_input, action);
        assert_eq!(format!("{:?}", app.document), document);
        enter(&mut app, "");
        assert!(app.active_command.is_none());
        assert_eq!(
            app.document.objects().next().unwrap().geometry(),
            &Geometry::NurbsCurve((*preview).clone())
        );
    }
}

#[test]
fn interpolated_draft_options_match_one_line_completion_and_preview() {
    for options in [
        "Degree=1 Knots=Uniform",
        "Degree=3 Knots=SqrtChrd",
        "Degree=3 Knots=Uniform Close=Smooth",
        "Degree=3 Knots=Chord Close=Sharp",
        "Degree=3 Knots=Uniform StartTangent=1,2,0 EndTangent=-1,1,0",
    ] {
        let mut app = test_app();
        enter(&mut app, &format!("InterpCurve {options}"));
        assert!(
            matches!(
                app.active_command,
                Some(InteractiveCommand::InterpCrv { .. })
            ),
            "{options}"
        );
        for input in ["0", "3,0,0", "4,2,1", "0,4,0"] {
            enter(&mut app, input);
        }
        let preview = app.curve_draft_preview().unwrap();
        assert_eq!(app.document.objects().len(), 0);
        enter(&mut app, "");
        assert!(app.active_command.is_none(), "{options}");
        let geometry = app.document.objects().next().unwrap().geometry().clone();
        assert_eq!(geometry, Geometry::NurbsCurve((*preview).clone()));
        let mut reference = test_app();
        enter(
            &mut reference,
            &format!("InterpCrv 0,0,0 3,0,0 4,2,1 0,4,0 {options}"),
        );
        assert_eq!(
            &geometry,
            reference.document.objects().next().unwrap().geometry(),
            "{options}"
        );
        assert_eq!(app.document.undo_label(), Some("InterpCrv"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 0);
        assert!(!app.document.can_undo());
        enter(&mut app, "Redo");
        assert_eq!(&geometry, app.document.objects().next().unwrap().geometry());
    }
}

#[test]
fn failed_closed_interpolation_retains_startup_options_for_retry() {
    let mut app = test_app();
    for input in ["InterpCrv Knots=Uniform Close=Sharp", "0"] {
        enter(&mut app, input);
    }
    let active = app.active_command;
    let points = app.curve_points.clone();
    enter(&mut app, "");
    assert_eq!(app.active_command, active);
    assert_eq!(app.curve_points, points);
    assert_eq!(app.document.objects().len(), 0);
    assert!(!app.document.can_undo());
    enter(&mut app, "1,0,0");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
        panic!("curve");
    };
    assert!(curve.is_closed().unwrap());
}

#[test]
fn invalid_interpolation_startup_options_do_not_create_a_draft_or_edit_history() {
    for options in [
        "Degree=2",
        "Knots=invalid",
        "Degree=1 Degree=3",
        "Close=Smooth StartTangent=1,0,0",
        "Degree=1 EndTangent=1,0,0",
    ] {
        let mut app = test_app();
        for input in ["Point 9,9,9", "Undo"] {
            enter(&mut app, input);
        }
        let document = format!("{:?}", app.document);
        enter(&mut app, &format!("InterpCrv {options}"));
        assert!(app.active_command.is_none());
        assert_eq!(format!("{:?}", app.document), document);
        assert!(app.curve_points.is_empty());
    }
}

#[test]
fn curve_prompt_uses_fixed_coordinate_wise_coincidence_not_model_tolerance() {
    let zero = 2.0_f64.powi(-32);
    for absolute in [1e-12, 1e-9, 0.01] {
        for (offset, accepted) in [
            (point(zero.next_down(), 0., 0.), false),
            (point(zero, 0., 0.), false),
            (point(zero.next_up(), 0., 0.), true),
            (point(-zero, 0., 0.), false),
            (point(zero, zero, 0.), false),
            (point(zero * 0.75, zero * 0.75, 0.), false),
            (point(0., 0., zero.next_up()), true),
            (point(1e-9, 0., 0.), true),
        ] {
            for typed in [true, false] {
                let mut app = test_app();
                app.document
                    .set_tolerance(Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap());
                enter(&mut app, "Curve");
                enter(&mut app, "0");
                let document = format!("{:?}", app.document);
                if typed {
                    enter(&mut app, &format!("w{}", format_model_point(offset)));
                } else {
                    assert_eq!(app.accept_drafting_point(offset), accepted);
                }
                assert_eq!(app.curve_points.len(), if accepted { 2 } else { 1 });
                assert_eq!(
                    app.last_point,
                    Some(if accepted { offset } else { point(0., 0., 0.) })
                );
                assert_eq!(format!("{:?}", app.document), document);
                for input in ["2,3,0", "10,0,0", ""] {
                    enter(&mut app, input);
                }
                let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry()
                else {
                    panic!("curve");
                };
                assert_eq!(curve.degree(), if accepted { 3 } else { 2 });
            }
        }
    }
}

#[test]
fn curve_prompt_skips_adjacent_points_in_measured_rhino_sequences() {
    for inputs in [
        vec!["0", "2,3,0", "10,0,0"],
        vec!["0", "0", "2,3,0", "10,0,0"],
        vec!["0", "2,3,0", "2,3,0", "10,0,0"],
        vec!["0", "0.0000000001,0,0", "2,3,0", "10,0,0"],
    ] {
        let mut app = test_app();
        enter(&mut app, "Curve Degree=3");
        for input in inputs {
            enter(&mut app, input);
        }
        let points = vec![point(0., 0., 0.), point(2., 3., 0.), point(10., 0., 0.)];
        assert_eq!(app.curve_points, points);
        enter(&mut app, "");
        let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
            panic!("curve");
        };
        assert_eq!(curve.degree(), 2);
        assert!(!curve.is_closed().unwrap());
        assert_eq!(
            curve,
            &NurbsCurve::try_control_point_curve_with_closure(
                3,
                points,
                ControlPointCurveClosure::Open
            )
            .unwrap()
        );
    }
}

#[test]
fn interpolated_preview_matches_completion_and_reuses_unchanged_geometry() {
    let mut app = test_app();
    for input in ["Point 9,9,9", "Undo", "InterpCrv", "0"] {
        enter(&mut app, input);
    }
    assert!(app.curve_draft_preview().is_none());
    for input in ["3,0,0", "4,2,1", "0,4,0"] {
        enter(&mut app, input);
    }
    let document = format!("{:?}", app.document);
    let preview = app.curve_draft_preview().unwrap();
    assert_eq!(preview.degree(), 3);
    assert!(std::sync::Arc::ptr_eq(
        &preview,
        &app.curve_draft_preview().unwrap()
    ));
    assert_eq!(format!("{:?}", app.document), document);
    enter(&mut app, "");
    assert_eq!(
        app.document.objects().next().unwrap().geometry(),
        &Geometry::NurbsCurve((*preview).clone())
    );
    assert!(app.curve_draft_preview().is_none());
}

#[test]
fn interpolated_preview_invalidates_on_point_undo_and_tolerance_change() {
    let mut app = test_app();
    for input in ["InterpCrv", "0", "1,0,0", "0,1,0"] {
        enter(&mut app, input);
    }
    let original = app.curve_draft_preview().unwrap();
    let tolerance = Tolerance::try_new(0.01, 1e-12, 1e-10).unwrap();
    app.document.set_tolerance(tolerance);
    let updated = app.curve_draft_preview().unwrap();
    assert!(!std::sync::Arc::ptr_eq(&original, &updated));
    enter(&mut app, "Undo");
    let two_points = app.curve_draft_preview().unwrap();
    assert!(!std::sync::Arc::ptr_eq(&updated, &two_points));
    enter(&mut app, "Undo");
    assert!(app.curve_draft_preview().is_none());
    enter(&mut app, "rw2,0,0");
    assert!(app.curve_draft_preview().is_some());
    assert_eq!(app.document.objects().len(), 0);
    assert_eq!(app.document.tolerance(), tolerance);
}

#[test]
fn curve_preview_tracks_settings_and_matches_committed_geometry_without_edits() {
    let mut app = test_app();
    assert!(app.curve_draft_preview().is_none());
    for input in ["Point 9,9,9", "Undo", "Curve", "0"] {
        enter(&mut app, input);
    }
    assert!(app.curve_draft_preview().is_none());
    for input in ["3,0,0", "4,2,1", "0,4,0"] {
        enter(&mut app, input);
    }
    let document = format!("{:?}", app.document);
    let open = app.curve_draft_preview().unwrap();
    assert!(!open.is_closed().unwrap());
    enter(&mut app, "Degree=2");
    let quadratic = app.curve_draft_preview().unwrap();
    assert_eq!(quadratic.degree(), 2);
    assert_ne!(open, quadratic);
    enter(&mut app, "Close=Smooth");
    let closed = app.curve_draft_preview().unwrap();
    assert!(closed.is_periodic());
    assert!(closed.is_closed().unwrap());
    assert_eq!(format!("{:?}", app.document), document);
    enter(&mut app, "");
    assert_eq!(
        app.document.objects().next().unwrap().geometry(),
        &Geometry::NurbsCurve((*closed).clone())
    );
    assert!(app.curve_draft_preview().is_none());
}

#[test]
fn curve_preview_disappears_when_undo_leaves_insufficient_closed_controls() {
    let mut app = test_app();
    for input in ["Curve Close=Sharp", "0", "1,0,0", "0,1,0"] {
        enter(&mut app, input);
    }
    assert!(app.curve_draft_preview().is_some());
    enter(&mut app, "Undo");
    assert!(app.curve_draft_preview().is_none());
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "Close=Open");
    assert!(app.curve_draft_preview().is_some());
    app.cancel_interactive_command(false);
    assert!(app.curve_draft_preview().is_none());
}

#[test]
fn curve_draft_settings_preserve_points_and_use_the_updated_geometry_parameters() {
    for (input_degree, expected_degree) in [("0", 1), ("2", 2), ("999", 11)] {
        let mut app = test_app();
        for input in [
            "Point 9,9,9",
            "Undo",
            "Curve",
            "0",
            "3,0,0",
            "4,2,1",
            "0,4,0",
        ] {
            enter(&mut app, input);
        }
        let points = app.curve_points.clone();
        let last_point = app.last_point;
        let plane = app.drafting_plane;
        let document = format!("{:?}", app.document);
        enter(&mut app, &format!("_dEgReE=_{input_degree}"));
        enter(&mut app, "Close=Sharp");
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Curve {
                degree: expected_degree,
                closure: ControlPointCurveClosure::Sharp,
            })
        );
        assert!(app.command_input.is_empty());
        assert_eq!(app.curve_points, points);
        assert_eq!(app.last_point, last_point);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(format!("{:?}", app.document), document);
        enter(&mut app, "");
        assert!(app.active_command.is_none());
        let mut reference = test_app();
        enter(
            &mut reference,
            &format!("Curve 0,0,0 3,0,0 4,2,1 0,4,0 Degree={expected_degree} Close=Sharp"),
        );
        assert_eq!(
            app.document.objects().next().unwrap().geometry(),
            reference.document.objects().next().unwrap().geometry()
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 0);
    }
}

#[test]
fn invalid_curve_draft_settings_preserve_all_draft_state_and_redo() {
    let mut app = test_app();
    for input in [
        "Point 9,9,9",
        "Undo",
        "Curve Degree=5 Close=Sharp",
        "0",
        "1,0,0",
    ] {
        enter(&mut app, input);
    }
    let active = app.active_command;
    let points = app.curve_points.clone();
    let last_point = app.last_point;
    let plane = app.drafting_plane;
    let document = format!("{:?}", app.document);
    for input in [
        "Degree=",
        "Degree=-1",
        "Degree=1.5",
        "Degree=NaN",
        "Degree=999999999999999999999999999",
        "Degree=2 extra",
        "Close=",
        "Close=invalid",
        "Close=Open extra",
    ] {
        enter(&mut app, input);
        assert_eq!(app.active_command, active, "{input}");
        assert_eq!(app.curve_points, points);
        assert_eq!(app.last_point, last_point);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.command_input, input);
        assert_eq!(format!("{:?}", app.document), document);
    }
    enter(&mut app, "Close=Open");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn curve_close_options_match_one_line_geometry_and_make_one_history_edit() {
    for (option, closure) in [("_Close", "Smooth"), ("sHaRp", "Sharp")] {
        for degree in [1, 2, 3, 5] {
            let mut app = test_app();
            enter(&mut app, &format!("Curve Degree={degree}"));
            for input in ["0", "3,0,0", "4,2,1", "0,4,0"] {
                enter(&mut app, input);
            }
            let last_point = app.last_point;
            enter(&mut app, option);
            assert!(app.active_command.is_none());
            assert!(app.curve_points.is_empty());
            assert!(app.command_input.is_empty());
            assert_eq!(app.last_point, last_point);
            let geometry = app.document.objects().next().unwrap().geometry().clone();
            let Geometry::NurbsCurve(curve) = &geometry else {
                panic!("curve")
            };
            assert!(curve.is_closed().unwrap());
            if degree > 1 {
                assert_eq!(curve.is_periodic(), closure == "Smooth");
            }
            let mut reference = test_app();
            enter(
                &mut reference,
                &format!("Curve 0,0,0 3,0,0 4,2,1 0,4,0 Degree={degree} Close={closure}"),
            );
            assert_eq!(
                &geometry,
                reference.document.objects().next().unwrap().geometry()
            );
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().len(), 0);
            assert!(!app.document.can_undo());
            enter(&mut app, "Redo");
            assert_eq!(&geometry, app.document.objects().next().unwrap().geometry());
        }
    }
}

#[test]
fn failed_curve_close_restores_original_options_and_preserves_redo() {
    for option in ["Close", "Sharp"] {
        for inputs in [vec!["0", "1,0,0"], vec!["0", "1,0,0", "0"]] {
            let mut app = test_app();
            for input in ["Point 9,9,9", "Undo", "Curve Degree=5"] {
                enter(&mut app, input);
            }
            for input in inputs {
                enter(&mut app, input);
            }
            let active = app.active_command;
            let points = app.curve_points.clone();
            let plane = app.drafting_plane;
            let last_point = app.last_point;
            let document = format!("{:?}", app.document);
            enter(&mut app, option);
            assert_eq!(app.active_command, active);
            assert_eq!(app.curve_points, points);
            assert_eq!(app.drafting_plane, plane);
            assert_eq!(app.last_point, last_point);
            assert_eq!(app.command_input, option);
            assert_eq!(format!("{:?}", app.document), document);
            enter(&mut app, "0,1,0");
            enter(&mut app, option);
            assert!(app.active_command.is_none());
            assert_eq!(app.document.objects().len(), 1);
        }
    }
}

#[test]
fn polyline_close_finishes_once_without_duplicating_an_existing_seam() {
    for repeated_start in [false, true] {
        let mut app = test_app();
        for input in ["Polyline", "1,2,3", "4,2,3", "4,5,3"] {
            enter(&mut app, input);
        }
        if repeated_start {
            assert!(app.accept_drafting_point(point(1.0, 2.0, 3.0)));
        }
        let last_point = app.last_point;
        enter(&mut app, "_cLoSe");
        assert!(app.active_command.is_none());
        assert!(app.curve_points.is_empty());
        assert!(app.command_input.is_empty());
        assert_eq!(app.last_point, last_point);
        let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
            panic!("polyline");
        };
        assert!(polyline.is_closed());
        assert_eq!(
            polyline.vertices(),
            &[
                point(1.0, 2.0, 3.0),
                point(4.0, 2.0, 3.0),
                point(4.0, 5.0, 3.0),
                point(1.0, 2.0, 3.0),
            ]
        );
        assert_eq!(app.document.undo_label(), Some("Polyline"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 0);
        assert!(!app.document.can_undo());
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), 1);
    }
}

#[test]
fn invalid_polyline_close_preserves_points_plane_reference_and_redo() {
    for points in [
        vec!["0", "1,0,0"],
        vec!["0", "1,0,0", "0.0000000001,0,0"],
        vec!["-1e308,0,0", "0", "1e308,0,0"],
    ] {
        let mut app = test_app();
        for input in ["Point 9,9,9", "Undo", "Polyline"] {
            enter(&mut app, input);
        }
        for input in points {
            enter(&mut app, input);
        }
        let document = format!("{:?}", app.document);
        let points = app.curve_points.clone();
        let plane = app.drafting_plane;
        let last_point = app.last_point;
        enter(&mut app, "Close");
        assert_eq!(app.active_command, Some(InteractiveCommand::Polyline));
        assert_eq!(app.curve_points, points);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.last_point, last_point);
        assert_eq!(app.command_input, "Close");
        assert_eq!(format!("{:?}", app.document), document);
        // An open completion remains available after a rejected closing edge.
        enter(&mut app, "");
        assert!(app.active_command.is_none());
        assert_eq!(app.document.objects().len(), 1);
    }
}

#[test]
fn curve_prompt_undo_removes_points_without_touching_document_history() {
    for command in ["Polyline", "Curve", "InterpCrv"] {
        let mut app = test_app();
        for input in ["Point 9,9,9", "Undo", command, "1,2,3"] {
            enter(&mut app, input);
        }
        assert!(app.accept_drafting_point(point(4.0, 5.0, 6.0)));
        let document = format!("{:?}", app.document);
        let active = app.active_command;
        let plane = app.drafting_plane;
        enter(&mut app, "_uNdO");
        assert_eq!(app.curve_points, vec![point(1.0, 2.0, 3.0)]);
        assert_eq!(app.last_point, Some(point(1.0, 2.0, 3.0)));
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.active_command, active);
        assert!(app.command_input.is_empty());
        assert_eq!(format!("{:?}", app.document), document);
        enter(&mut app, "rw2,0,0");
        assert_eq!(app.curve_points[1], point(3.0, 2.0, 3.0));
        for _ in 0..3 {
            enter(&mut app, "Undo");
        }
        assert!(app.curve_points.is_empty());
        assert_eq!(app.last_point, None);
        assert_eq!(app.drafting_plane, None);
        assert_eq!(app.active_command, active);
        assert_eq!(format!("{:?}", app.document), document);
        enter(&mut app, "rw1,0,0");
        assert!(app.curve_points.is_empty());
        for input in ["0", "1,0,0", ""] {
            enter(&mut app, input);
        }
        assert!(app.active_command.is_none(), "{command}");
        assert_eq!(app.document.objects().len(), 1);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 0);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), 1);
    }
}

#[test]
fn curve_prompt_undo_can_correct_failed_closed_curve_completion() {
    let mut app = test_app();
    for input in ["Curve Close=Sharp", "0", "1,0,0", "0", ""] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_some());
    for input in ["Undo", "0,1,0", ""] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.undo_label(), Some("Curve"));
}

#[test]
fn failed_curve_completion_retains_the_draft_for_correction() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    assert!(app.document.can_redo());
    for input in ["Curve Close=Sharp", "0,0,0", "1,0,0", "0,0,0"] {
        enter(&mut app, input);
    }
    let active = app.active_command;
    let points = app.curve_points.clone();
    let plane = app.drafting_plane;
    let previous = app.last_point;
    let document = format!("{:?}", app.document);
    enter(&mut app, "");
    assert_eq!(app.active_command, active);
    assert_eq!(app.curve_points, points);
    assert_eq!(app.drafting_plane, plane);
    assert_eq!(app.last_point, previous);
    assert_eq!(format!("{:?}", app.document), document);
    assert!(app.document.can_redo());
    enter(&mut app, "0,1,0");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(app.curve_points.is_empty());
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.undo_label(), Some("Curve"));
}

#[test]
fn polyline_rejects_overflowing_segments_before_appending_vertices() {
    for typed in [false, true] {
        let mut app = test_app();
        enter(&mut app, "Polyline");
        enter(&mut app, "-1e308,0,0");
        let active = app.active_command;
        let points = app.curve_points.clone();
        if typed {
            enter(&mut app, "1e308,0,0");
            assert_eq!(app.command_input, "1e308,0,0");
        } else {
            assert!(!app.accept_drafting_point(point(1e308, 0.0, 0.0)));
        }
        assert_eq!(app.active_command, active);
        assert_eq!(app.curve_points, points);
        assert_eq!(app.last_point, Some(point(-1e308, 0.0, 0.0)));
        enter(&mut app, "0,0,0");
        enter(&mut app, "");
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.undo_label(), Some("Polyline"));
    }
}

#[test]
fn overflowing_endpoint_distances_preserve_typed_and_picked_drafts() {
    for command in ["Line", "Sphere"] {
        for typed in [false, true] {
            let mut app = test_app();
            enter(&mut app, command);
            enter(&mut app, "-1e308,0,0");
            let active = app.active_command;
            let previous = app.last_point;
            if typed {
                enter(&mut app, "1e308,0,0");
                assert_eq!(app.command_input, "1e308,0,0");
            } else {
                assert!(!app.accept_drafting_point(point(1e308, 0.0, 0.0)));
            }
            assert_eq!(app.active_command, active, "{command}, typed={typed}");
            assert_eq!(app.last_point, previous);
            assert_eq!(app.document.objects().len(), 0);
            assert!(!app.document.can_undo());
            if command == "Line" {
                if typed {
                    enter(&mut app, "0,0,0");
                } else {
                    assert!(app.accept_drafting_point(point(0.0, 0.0, 0.0)));
                }
                assert!(app.active_command.is_none());
                assert_eq!(app.document.objects().len(), 1);
                assert_eq!(app.document.undo_label(), Some("Line"));
                assert_eq!(app.last_point, Some(point(0.0, 0.0, 0.0)));
            }
        }
    }
}

#[test]
fn typed_and_picked_endpoints_share_the_current_absolute_tolerance() {
    for command in ["Line", "Sphere"] {
        for typed in [false, true] {
            let mut app = test_app();
            enter(&mut app, "Tolerance Absolute=0.01");
            enter(&mut app, command);
            enter(&mut app, "0,0,0");
            let active = app.active_command;
            for distance in [0.005, 0.01] {
                if typed {
                    enter(&mut app, &format!("{distance},0,0"));
                } else {
                    assert!(!app.accept_drafting_point(point(distance, 0.0, 0.0)));
                }
                assert_eq!(app.active_command, active);
                assert_eq!(app.last_point, Some(point(0.0, 0.0, 0.0)));
                assert_eq!(app.document.objects().len(), 0);
            }
            if typed {
                enter(&mut app, "1,0,0");
            } else {
                assert!(app.accept_drafting_point(point(1.0, 0.0, 0.0)));
            }
            assert!(app.active_command.is_none());
            assert_eq!(app.document.objects().len(), 1);
            assert_eq!(app.document.undo_label(), Some(command));
        }
    }
}

#[test]
fn typed_points_continue_line_instead_of_cancelling_it() {
    let mut app = test_app();
    for input in ["Line", "1,2,3", "4,6,3"] {
        enter(&mut app, input);
    }
    let Geometry::Line(line) = app
        .document
        .objects()
        .next()
        .expect("created line")
        .geometry()
    else {
        panic!("line");
    };
    assert_eq!(line.start(), point(1.0, 2.0, 3.0));
    assert_eq!(line.end(), point(4.0, 6.0, 3.0));
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), Some("Line"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
}

#[test]
fn typed_points_use_each_active_construction_plane_with_world_override() {
    for (viewport, expected) in [
        (0, [1.0, 2.0, 3.0]),
        (1, [1.0, 2.0, 3.0]),
        (2, [1.0, -3.0, 2.0]),
        (3, [3.0, 1.0, 2.0]),
    ] {
        let mut app = test_app();
        app.active_viewport = viewport;
        for input in ["Line", "1,2,3", "wr2,3,4"] {
            enter(&mut app, input);
        }
        let Geometry::Line(line) = app.document.objects().next().unwrap().geometry() else {
            panic!("line");
        };
        assert_eq!(line.start().to_array(), expected);
        assert_eq!(
            line.end().to_array(),
            [expected[0] + 2.0, expected[1] + 3.0, expected[2] + 4.0]
        );
    }
}

#[test]
fn mouse_and_typed_relative_points_share_a_polyline_without_grid_rounding() {
    let mut app = test_app();
    enter(&mut app, "Polyline");
    assert!(app.accept_drafting_point(point(1.25, 2.5, 0.75)));
    for input in ["r2,3", "@2<90,1", ""] {
        enter(&mut app, input);
    }
    let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
        panic!("polyline");
    };
    assert_eq!(
        polyline.vertices(),
        &[
            point(1.25, 2.5, 0.75),
            point(3.25, 5.5, 0.75),
            point(3.25, 7.5, 1.75)
        ]
    );
    assert!(app.osnap && app.smart_track && app.grid_snap);
    assert_eq!(app.document.undo_label(), Some("Polyline"));
}

#[test]
fn invalid_typed_points_preserve_the_draft_last_point_and_editable_input() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "r1,2");
    assert_eq!(app.last_point, None);
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Line { start: None })
    );
    assert_eq!(app.command_input, "r1,2");
    enter(&mut app, "1,2,3");
    let active = app.active_command;
    for input in [
        "1,,2",
        "NaN,0",
        "1e309,0",
        "wInf,0",
        "5",
        "1, 2",
        "w 1,2",
        "bad,1",
        "@",
        "rw",
        "w5<30<120",
    ] {
        enter(&mut app, input);
        assert_eq!(app.active_command, active);
        assert_eq!(app.last_point, Some(point(1.0, 2.0, 3.0)));
        assert_eq!(app.command_input, input);
        assert!(!app.document.can_undo());
    }
    enter(&mut app, "r1,0");
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.command_input.is_empty());
}

#[test]
fn rejected_geometric_pick_does_not_replace_the_relative_origin() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    enter(&mut app, "1,2,3");
    enter(&mut app, "1,2,3"); // A coincident radius point is rejected.
    assert_eq!(app.last_point, Some(point(1.0, 2.0, 3.0)));
    assert!(app.active_command.is_some());
    enter(&mut app, "r2,0");
    let Geometry::Circle(circle) = app.document.objects().next().unwrap().geometry() else {
        panic!("circle");
    };
    assert_eq!(circle.center(), point(1.0, 2.0, 3.0));
    assert_eq!(circle.radius(), 2.0);
}

#[test]
fn command_names_aliases_and_script_prefixes_can_replace_a_point_prompt() {
    for replacement in ["Line", "L", "-Line", "_Line", "_-Line"] {
        let mut app = test_app();
        for input in ["Line", "1,2,3", replacement] {
            enter(&mut app, input);
        }
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Line { start: None })
        );
        assert_eq!(app.document.objects().len(), 0);
    }
    let mut app = test_app();
    for input in ["Line", "1,2,3", "Point 8,9,10"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn unit_scale_changes_invalidate_old_relative_point_references() {
    let mut app = test_app();
    for input in ["Line", "1000,0,0", "Units Meters Scale=Yes"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none());
    assert!(app.curve_points.is_empty());
    assert_eq!(app.last_point, None);
    enter(&mut app, "Point");
    enter(&mut app, "rw1,0,0");
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "2,0,0");
    assert_eq!(app.last_point, Some(point(2.0, 0.0, 0.0)));
    enter(&mut app, "Undo"); // Point edit, not a unit edit.
    assert_eq!(app.last_point, Some(point(2.0, 0.0, 0.0)));
    enter(&mut app, "Undo"); // Unit edit.
    assert_eq!(app.last_point, None);
    for input in ["Line", "5,0,0", "Redo"] {
        enter(&mut app, input);
    }
    assert_eq!(app.last_point, None);
}

#[test]
fn metadata_only_unit_scale_changes_clear_references_but_custom_renames_do_not() {
    let mut app = test_app();
    for input in ["Polyline", "1000,0,0", "2000,0,0", "Units Meters Scale=No"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none());
    assert!(app.curve_points.is_empty());
    assert_eq!(app.last_point, None);
    assert_eq!(app.document.objects().len(), 0);
    for input in [
        "Line",
        "3,4,5",
        "Units Custom MetersPerUnit=1 Scale=No Name=custom metre",
    ] {
        enter(&mut app, input);
    }
    assert_eq!(app.last_point, Some(point(3.0, 4.0, 5.0)));
    enter(
        &mut app,
        "Units Custom MetersPerUnit=1 Scale=Yes Name=renamed metre",
    );
    assert_eq!(app.last_point, Some(point(3.0, 4.0, 5.0)));
    enter(
        &mut app,
        "Units Custom MetersPerUnit=0.5 Scale=No Name=half metre",
    );
    assert_eq!(app.last_point, None);
}

#[test]
fn unit_queries_noops_and_failures_preserve_relative_point_references() {
    for command in [
        "Units",
        "Units Millimeters Scale=Yes",
        "Units Meters",
        "Units Unset Scale=Yes",
    ] {
        let mut app = test_app();
        for input in ["Line", "1000,0,0", command] {
            enter(&mut app, input);
        }
        assert_eq!(app.last_point, Some(point(1000.0, 0.0, 0.0)), "{command}");
    }
}

#[test]
fn cancellation_discards_geometry_but_remembers_accepted_interactive_points() {
    let mut app = test_app();
    for input in ["Polyline", "1,2,3", "r1,2", "1,,2"] {
        enter(&mut app, input);
    }
    app.cancel_interactive_command(true);
    assert!(app.command_input.is_empty());
    assert!(app.curve_points.is_empty());
    assert_eq!(app.document.objects().len(), 0);
    assert!(!app.document.can_undo());
    for input in ["Point", "rw1,2,3"] {
        enter(&mut app, input);
    }
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point");
    };
    assert_eq!(*p, point(3.0, 6.0, 6.0));
}

#[test]
fn typed_transform_points_use_the_existing_transactional_command() {
    let mut app = test_app();
    for input in ["Point 1,2,3", "SelAll", "Move", "0", "r4,5,6"] {
        enter(&mut app, input);
    }
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point");
    };
    assert_eq!(*p, point(5.0, 7.0, 9.0));
    assert_eq!(app.document.undo_label(), Some("Move"));
    enter(&mut app, "Undo");
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point");
    };
    assert_eq!(*p, point(1.0, 2.0, 3.0));
}

#[test]
fn typed_point_completion_retains_full_float_precision() {
    let mut app = test_app();
    enter(&mut app, "Point");
    enter(
        &mut app,
        "w1.1234567890123457,-2.123456789012345,0.000000123456789",
    );
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point");
    };
    assert_eq!(
        *p,
        point(1.1234567890123457, -2.123456789012345, 0.000000123456789)
    );
}

#[test]
fn a_mouse_pick_can_correct_invalid_typed_input_without_leaving_stale_text() {
    let mut app = test_app();
    for input in ["Line", "0", "1,,2"] {
        enter(&mut app, input);
    }
    app.handle_viewport_action(ViewportOutput {
        picked_point: Some(point(2.0, 3.0, 0.0)),
        ..ViewportOutput::default()
    });
    assert!(app.command_input.is_empty());
    assert!(app.active_command.is_none());
    assert_eq!(app.last_point, Some(point(2.0, 3.0, 0.0)));
    assert_eq!(app.document.objects().len(), 1);
}
