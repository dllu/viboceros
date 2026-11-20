use super::super::circle_preview::viewport_frame;
use super::super::curve_fillet::enter;
use super::super::curve_match::click_world;
use super::*;
use viboceros_geometry::{LineSegment, MaelstromPointMorph, PointMorph, Vector3};

fn p(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

#[test]
fn maelstrom_fit_points_viewport_selection_previews_and_keeps_targets_separate() {
    let mut app = test_app();
    enter(&mut app, "Line 3,1,0 3,1,1");
    let target = app.document.objects().next().unwrap().clone();
    let picks = [point(2., 0., 0.), point(0., 2., 0.), point(-2., 0., 0.)];
    let ids = picks.map(|p| app.document.add_geometry(Geometry::Point(p)).unwrap());
    app.document
        .select_object(target.id(), SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "FitPoints");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().label(),
        "Maelstrom FitPoints"
    );
    click_world(&mut app, &context, picks[0]);
    click_world(&mut app, &context, picks[1]);
    assert!(app.maelstrom_fit_preview().is_none());
    click_world(&mut app, &context, picks[2]);
    let fit = app.maelstrom_fit_preview().unwrap();
    assert!(fit.center().distance_to(point(0., 0., 0.)).unwrap() < 1e-10);
    assert!((fit.radius() - 2.).abs() < 1e-10);
    assert!(!viewport_frame(&mut app, &context, vec![]).is_empty());
    assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), ids);
    assert_eq!(app.document.objects().len(), 4);
    assert!(!app.document.can_undo());
    enter(&mut app, "_Enter");
    assert!(app.object_prompt.is_none());
    assert!(app.maelstrom_preview().unwrap().initial.is_some());
    enter(&mut app, "5");
    assert_eq!(
        app.maelstrom_preview().unwrap().target,
        Some(MaelstromRadius::Number(5.))
    );
    enter(&mut app, "90");
    assert!(app.active_command.is_none());
    assert_ne!(app.document.object(target.id()), Some(&target));
    assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), ids);
    assert_eq!(app.document.undo_label(), Some("Maelstrom"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(target.id()), Some(&target));
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_ne!(app.document.object(target.id()), Some(&target));
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn maelstrom_fit_points_viewport_grips_are_reference_geometry() {
    let mut app = test_app();
    enter(&mut app, "Line 3,1,0 3,1,1");
    let target = app.document.objects().next().unwrap().id();
    let grips = [point(2., 0., 0.), point(0., 2., 0.), point(-2., 0., 0.)];
    let curve = NurbsCurve::try_clamped_uniform(2, grips.to_vec()).unwrap();
    let parent = app
        .document
        .add_geometry(Geometry::NurbsCurve(curve))
        .unwrap();
    app.document
        .select_object(parent, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "PointsOn");
    app.document
        .select_object(target, SelectionMode::Replace)
        .unwrap();
    let reference = app.document.object(parent).unwrap().clone();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "FitPoints");
    for point in grips {
        click_world(&mut app, &context, point);
    }
    assert_eq!(app.document.selected_control_points().count(), 3);
    assert!((app.maelstrom_fit_preview().unwrap().radius() - 2.).abs() < 1e-10);
    enter(&mut app, "");
    enter(&mut app, "5");
    enter(&mut app, "90");
    assert_eq!(app.document.object(parent), Some(&reference));
    assert_eq!(app.document.selected_control_points().count(), 3);
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_control_points().count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_control_points().count(), 0);
}

#[test]
fn maelstrom_fit_points_short_or_cancelled_selection_leaves_the_model_unchanged() {
    let mut app = test_app();
    enter(&mut app, "Line 3,1,0 3,1,1");
    let target = app.document.objects().next().unwrap().clone();
    let bad = [point(0., 0., 0.), point(1., 0., 0.), point(2., 0., 0.)]
        .map(|p| app.document.add_geometry(Geometry::Point(p)).unwrap());
    app.document
        .select_object(target.id(), SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "FitPoints");
    for id in bad.into_iter().take(2) {
        enter(&mut app, &format!("SelID={id}"));
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.active_command.is_none());
    assert!(!app.document.can_undo());
    assert_eq!(app.document.selected_object_count(), 0);
    app.document
        .select_object(target.id(), SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "FitPoints");
    enter(&mut app, "_-SelID");
    enter(&mut app, &bad[0].to_string());
    assert!(app.document.is_selected(bad[0]));
    app.cancel_current_prompt_or_selection();
    assert!(app.object_prompt.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.object(target.id()), Some(&target));
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());
}

#[test]
fn maelstrom_fit_points_zero_radius_copy_matches_native_identity_placement() {
    let mut app = test_app();
    enter(&mut app, "Line 3,1,0 3,1,1");
    let target = app.document.objects().next().unwrap().clone();
    let points = [point(2., 5., 7.), point(3., 5., 7.), point(4., 5., 7.)]
        .map(|p| app.document.add_geometry(Geometry::Point(p)).unwrap());
    app.document
        .select_object(target.id(), SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "FitPoints");
    for id in points {
        enter(&mut app, &format!("SelID {id}"));
    }
    assert!(app.maelstrom_fit_preview().is_none());
    enter(&mut app, "");
    let preview = app.maelstrom_preview().unwrap();
    assert_eq!(preview.initial, Some(MaelstromRadius::Number(0.)));
    assert_eq!(preview.center, point(3., 5., 7.));
    assert!(!app.document.can_undo());
    enter(&mut app, "Copy=Yes");
    enter(&mut app, "5");
    enter(&mut app, "90");
    enter(&mut app, "-30");
    enter(&mut app, "");
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().len(), 6);
    for copied in app.document.objects().skip(4) {
        let curve = copied.geometry().curve_ref().unwrap();
        assert_eq!(curve.start_point().unwrap(), point(3., 1., 0.));
        assert_eq!(curve.end_point().unwrap(), point(3., 1., 1.));
    }
    assert_eq!(app.document.object(target.id()), Some(&target));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 4);
}

#[test]
fn maelstrom_fit_points_replays_native_selection_and_compares_fitted_deformation() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/maelstrom_fit_points.json"
    ))
    .unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/maelstrom_fit_points.json"
    ))
    .unwrap();
    let mut matching_normals = 0;
    let mut matching_results = 0;
    for (op, row) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(response["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let label = op["id"].as_str().unwrap();
        let v = &row["value"];
        let rows = v["before"].as_array().unwrap();
        let mut app = test_app();
        let plane = Frame3::try_from_normal(
            point(0., 0., 0.),
            Vector3::try_from(p(&op["normal"]).to_array()).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        app.viewports[app.active_viewport].set_construction_plane(plane);
        let ids = rows
            .iter()
            .map(|r| {
                let geo = if r["end"].is_null() {
                    Geometry::Point(p(&r["point"]))
                } else {
                    Geometry::Line(
                        LineSegment::try_new(
                            p(&r["point"]),
                            p(&r["end"]),
                            app.document.tolerance(),
                        )
                        .unwrap(),
                    )
                };
                app.document.add_geometry(geo).unwrap()
            })
            .collect::<Vec<_>>();
        app.document
            .select_objects_direct(
                ids.iter()
                    .copied()
                    .zip(rows)
                    .filter_map(|(id, r)| (r["selected"] == true).then_some(id)),
                SelectionMode::Replace,
            )
            .unwrap();
        app.document.clear_history().unwrap();
        enter(&mut app, "Maelstrom");
        enter(&mut app, "FitPoints");
        let definitions = ids
            .iter()
            .zip(rows)
            .filter(|(_, r)| !r["definition"].is_null())
            .collect::<Vec<_>>();
        if definitions.is_empty() {
            assert!(
                app.object_prompt.is_none(),
                "{label}: preselected points should fit immediately"
            );
        } else {
            for (id, _) in definitions {
                enter(&mut app, &format!("SelID {id}"));
            }
            enter(&mut app, "");
        }
        assert!(!app.document.can_undo(), "{label}");
        let fit = app.maelstrom_preview().unwrap();
        let native = &v["sdk_circle"];
        let native_normal = Vector3::try_from(p(&native["normal"]).to_array())
            .unwrap()
            .normalized(Tolerance::DEFAULT)
            .unwrap();
        let axis = fit.cplane.z_axis().as_vector();
        let normal_alignment = axis.dot(native_normal.as_vector()).unwrap();
        assert!(
            normal_alignment.abs() > 1. - 1e-12,
            "{label}: incorrect fitting plane"
        );
        let radius = native["radius"].as_f64().unwrap();
        let fitted_radius = fit.initial.unwrap().radius(fit.cplane).unwrap();
        assert!(
            (radius - fitted_radius).abs() < 1e-7,
            "{label}: circle radius"
        );
        assert!(
            fit.center.distance_to(p(&native["origin"])).unwrap() < 1e-7,
            "{label}: circle center"
        );
        let target = op["target"].as_f64().unwrap();
        let degrees = op["degrees"].as_f64().unwrap();
        let morph =
            MaelstromPointMorph::try_new(fit.cplane, fitted_radius, target, degrees.to_radians())
                .unwrap();
        enter(&mut app, &target.to_string());
        enter(&mut app, &degrees.to_string());
        assert!(app.active_command.is_none(), "{label}");
        let mut max_native_error = 0.0_f64;
        for (phase, next) in [("after", "Undo"), ("undo", "Redo"), ("redo", "")] {
            let native_rows = v[phase].as_array().unwrap();
            assert_eq!(app.document.objects().len(), native_rows.len(), "{label}");
            for (id, original) in ids.iter().zip(rows) {
                let native = native_rows
                    .iter()
                    .find(|r| {
                        r["target"] == original["target"]
                            && r["definition"] == original["definition"]
                    })
                    .unwrap();
                assert_eq!(
                    Some(app.document.is_selected(*id)),
                    native["selected"].as_bool(),
                    "{label} {phase}"
                );
                let geo = app.document.object(*id).unwrap().geometry();
                let (a, b) = match geo {
                    Geometry::Point(point) => (*point, None),
                    _ => {
                        let curve = geo.curve_ref().unwrap();
                        (
                            curve.start_point().unwrap(),
                            Some(curve.end_point().unwrap()),
                        )
                    }
                };
                for (actual, key) in std::iter::once((a, "point")).chain(b.map(|b| (b, "end"))) {
                    let expected = if phase != "undo" && !original["target"].is_null() {
                        morph.morph_point(p(&original[key])).unwrap()
                    } else {
                        p(&original[key])
                    };
                    assert!(
                        actual.distance_to(expected).unwrap() < 2e-5,
                        "{label} {phase} {key}: preview/commit disagreement"
                    );
                    max_native_error =
                        max_native_error.max(actual.distance_to(p(&native[key])).unwrap());
                }
            }
            if !next.is_empty() {
                enter(&mut app, next);
            }
        }
        if normal_alignment > 0. {
            matching_normals += 1;
            assert!(max_native_error < 2e-5, "{label}: {max_native_error}");
        }
        matching_results += usize::from(max_native_error < 2e-5);
        eprintln!(
            "{label}: fitted normal dot={normal_alignment}, max native distance={max_native_error}"
        );
    }
    eprintln!(
        "Maelstrom FitPoints: {matching_normals}/17 matching normal orientations, {matching_results}/17 matching sampled deformations"
    );
}
