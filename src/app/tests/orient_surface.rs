use super::*;
use viboceros_geometry::Vector3;
use viboceros_oracle::OrientSurfaceFixture;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn recipe() -> OrientSurfaceFixture {
    let v: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/orient_surface.json"
    ))
    .unwrap();
    serde_json::from_value(v["operations"][0].clone()).unwrap()
}
fn setup(f: &OrientSurfaceFixture) -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    let tol = app.document.tolerance();
    let guide = app.document.add_geometry(f.guide(tol).unwrap()).unwrap();
    let source = app
        .document
        .add_geometry(Geometry::PointCloud(f.source().unwrap()))
        .unwrap();
    if f.group {
        app.document
            .add_group(Some("surface sources".into()), [source])
            .unwrap();
    }
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.viewports[app.active_viewport].set_construction_plane(f.plane(tol).unwrap());
    app.document.clear_history().unwrap();
    (app, source, guide)
}
#[test]
fn orient_surface_interactive_replays_all_native_clouds_selection_and_groups() {
    for (input, reference) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/orient_surface.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/orient_surface_rhino_20261016.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/orient_surface_policy.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/orient_surface_policy_rhino_20261016.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/orient_surface_options.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/orient_surface_options_rhino_20261016.json"
            ),
        ),
    ] {
        let input: serde_json::Value = serde_json::from_str(input).unwrap();
        let reference: serde_json::Value = serde_json::from_str(reference).unwrap();
        assert_eq!(
            input["operations"].as_array().unwrap().len(),
            reference["results"].as_array().unwrap().len()
        );
        for (r, n) in input["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(reference["results"].as_array().unwrap())
        {
            assert_eq!(r["id"], n["id"]);
            let f: OrientSurfaceFixture = serde_json::from_value(r.clone()).unwrap();
            let (mut app, source, guide) = setup(&f);
            if f.postselected {
                app.document.clear_selection();
            }
            if let Some(seed) = &f.setup {
                enter(
                    &mut app,
                    &format!("OrientOnSrf {}", seed.options().arguments()),
                );
                enter(&mut app, "w0,0,0");
                enter(&mut app, "w1,0,0");
                app.accept_orient_surface(guide, None);
                enter(&mut app, "Cancel");
                assert!(app.document.undo_label().is_none());
            }
            let initial = if f.inherit {
                app.commands.orient_surface_options()
            } else {
                f.options.options()
            };
            let args = if f.inherit {
                format!("Copy={}", if f.options.copy { "Yes" } else { "No" })
            } else {
                initial.arguments()
            };
            enter(&mut app, &format!("OrientOnSrf {args}"));
            if f.postselected {
                enter(&mut app, &format!("SelID {source}"));
                enter(&mut app, "");
            }
            if f.options.on_surface {
                assert!(app.picking_orient_surface());
                app.accept_component_face_hit(guide, 0, None);
            }
            for p in [f.base, f.reference] {
                enter(
                    &mut app,
                    &format!("w{}", p.map(|x| x.to_string()).join(",")),
                );
            }
            if !f.options.on_surface {
                assert!(
                    app.picking_orient_surface(),
                    "{} {:?}",
                    r["id"],
                    app.command_log
                );
                app.apply_selection_click(crate::viewport::SelectionClick {
                    object_id: Some(guide),
                    mode: SelectionMode::Replace,
                });
            }
            for (uv, scale, rotation) in
                std::iter::once((f.uv, f.options.scale, f.options.rotation)).chain(
                    f.batch.iter().map(|b| {
                        (
                            b.uv,
                            b.scale.unwrap_or(f.options.scale),
                            b.rotation.unwrap_or(f.options.rotation),
                        )
                    }),
                )
            {
                let target = f.target(uv, app.document.tolerance()).unwrap();
                // Preset previews use only authored source/guide data. Native
                // coordinates are expectations, never placement inputs.
                if !initial.scale_prompt && !initial.rotation_prompt {
                    use viboceros_geometry::PointMorph;
                    let before = format!("{:?}", app.document);
                    let session = app.orient_surface_session.as_ref().unwrap();
                    let normal = Vector3::try_new(0., 0., 1.).unwrap();
                    let affine = app.affine_preview().map(|preview| {
                        preview
                            .definition
                            .transform_at(preview.frame.unwrap(), target, app.document.tolerance())
                            .unwrap()
                    });
                    let morph = session
                        .deformation(normal)
                        .map(|preview| preview.placement.morph_at(target, normal).unwrap());
                    let points = f
                        .source()
                        .unwrap()
                        .points()
                        .iter()
                        .map(|p| {
                            if initial.rigid {
                                affine.as_ref().unwrap().transform_point(*p).unwrap()
                            } else {
                                morph.as_ref().unwrap().morph_point(*p).unwrap()
                            }
                        })
                        .collect::<Vec<_>>();
                    assert!(
                        n["value"]["outputs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|cloud| {
                                points.iter().zip(cloud["points"].as_array().unwrap()).all(
                                    |(p, q)| {
                                        p.to_array()
                                            .iter()
                                            .zip(q.as_array().unwrap())
                                            .all(|(x, y)| (x - y.as_f64().unwrap()).abs() < 1e-6)
                                    },
                                )
                            }),
                        "{} preset preview {:?}",
                        r["id"],
                        points
                    );
                    assert_eq!(format!("{:?}", app.document), before);
                }
                enter(
                    &mut app,
                    &format!("w{}", target.to_array().map(|x| x.to_string()).join(",")),
                );
                if initial.scale_prompt || initial.rotation_prompt {
                    assert!(app.document.undo_label().is_none() || !f.batch.is_empty());
                }
                if initial.scale_prompt {
                    enter(&mut app, &scale.to_string());
                }
                if initial.rotation_prompt {
                    enter(&mut app, &rotation.to_string());
                }
            }
            if app.active_command.is_some() {
                enter(&mut app, "Cancel");
            }
            assert_eq!(
                app.document.object(guide).unwrap().geometry(),
                &f.guide(app.document.tolerance()).unwrap()
            );
            assert!(!app.document.is_selected(guide));
            assert_eq!(
                app.document.groups().len(),
                n["value"]["group_count"].as_u64().unwrap() as usize,
                "{} groups",
                r["id"]
            );
            let actual = app
                .document
                .objects()
                .filter_map(|o| {
                    let Geometry::PointCloud(c) = o.geometry() else {
                        return None;
                    };
                    Some((
                        o.id() == source,
                        app.document.is_selected(o.id()),
                        o.group_ids().len(),
                        c.points(),
                    ))
                })
                .collect::<Vec<_>>();
            let expected = n["value"]["outputs"].as_array().unwrap();
            assert_eq!(
                actual.len(),
                expected.len(),
                "{} {:?}",
                r["id"],
                app.command_log
            );
            let mut used = std::collections::BTreeSet::new();
            for (original, selected, groups, points) in actual {
                let (i, e) = expected
                    .iter()
                    .enumerate()
                    .find(|(i, e)| {
                        !used.contains(i)
                            && e["original"] == original
                            && points
                                .iter()
                                .zip(e["points"].as_array().unwrap())
                                .all(|(p, q)| {
                                    p.to_array()
                                        .iter()
                                        .zip(q.as_array().unwrap())
                                        .all(|(x, y)| (x - y.as_f64().unwrap()).abs() < 1e-6)
                                })
                    })
                    .unwrap_or_else(|| {
                        panic!("{} points {:?}, log {:?}", r["id"], points, app.command_log)
                    });
                used.insert(i);
                assert_eq!(e["selected"], selected, "{} selection", r["id"]);
                assert_eq!(e["group_count"], groups);
            }
        }
    }
}
#[test]
fn orient_surface_copy_batch_getters_previews_and_one_undo() {
    for rigid in [false, true] {
        let mut f = recipe();
        f.options.copy = true;
        f.options.rigid = rigid;
        let (mut app, source, guide) = setup(&f);
        enter(
            &mut app,
            &format!(
                "OrientOnSrf ScalePrompt=Yes RotationPrompt=Yes Rigid={} Copy=Yes",
                if rigid { "Yes" } else { "No" }
            ),
        );
        enter(&mut app, "w0,0,0");
        enter(&mut app, "w1,0,0");
        enter(&mut app, &format!("Surface={guide} Face=0"));
        assert!(app.orient_surface_location().is_some());
        let target = f.target([0.3, 0.4], app.document.tolerance()).unwrap();
        let before = format!("{:?}", app.document);
        let s = app.orient_surface_session.as_ref().unwrap();
        if rigid {
            let map = app
                .affine_preview()
                .unwrap()
                .definition
                .transform_at(
                    app.affine_preview().unwrap().frame.unwrap(),
                    target,
                    app.document.tolerance(),
                )
                .unwrap();
            assert!(
                map.transform_point(point(0., 0., 0.))
                    .unwrap()
                    .is_near(target, app.document.tolerance())
            );
        } else {
            use viboceros_geometry::PointMorph;
            let normal = Vector3::try_new(0., 0., 1.).unwrap();
            let m = s
                .deformation(normal)
                .unwrap()
                .placement
                .morph_at(target, normal)
                .unwrap();
            assert!(
                m.morph_point(point(0., 0., 0.))
                    .unwrap()
                    .is_near(target, app.document.tolerance())
            );
        }
        assert_eq!(format!("{:?}", app.document), before);
        for _ in 0..2 {
            assert!(app.accept_drafting_point(target));
            assert!(app.orient_surface_location().is_none());
            enter(&mut app, "0");
            assert!(app.document.undo_label().is_none() || app.document.objects().count() == 3);
            enter(&mut app, "");
            enter(&mut app, "");
        }
        enter(&mut app, "");
        assert!(app.active_command.is_none());
        assert_eq!(app.document.objects().count(), 4);
        assert!(app.document.is_selected(source));
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnSrf"));
        assert_eq!(app.document.objects().count(), 2);
        assert!(app.document.undo().unwrap().is_none());
        app.document.redo().unwrap();
        assert_eq!(app.document.objects().count(), 4);
    }
}
#[test]
fn orient_surface_stale_guide_repick_and_invalid_source_reference_are_atomic() {
    let f = recipe();
    let (mut app, _, guide) = setup(&f);
    enter(&mut app, "OrientOnSrf");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w0,0,0");
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "w1,0,0");
    app.accept_orient_surface(guide, None);
    assert!(
        app.document
            .set_tolerance(Tolerance::try_new(1e-5, 1e-12, 1e-8).unwrap())
    );
    assert!(!app.validate_orient_surface());
    assert!(app.picking_orient_surface());
    assert!(app.affine_preview().is_none());
    assert!(app.orient_surface_location().is_none());
    app.accept_orient_surface(guide, Some(0));
    assert!(app.affine_preview().is_some());
    enter(&mut app, "Cancel");
}
#[test]
fn orient_surface_scale_and_rotation_points_use_target_tangent_plane() {
    let mut f = recipe();
    f.options.copy = false;
    let (mut app, _, guide) = setup(&f);
    enter(
        &mut app,
        "OrientOnSrf ScalePrompt=Yes RotationPrompt=Yes Copy=No",
    );
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,0");
    app.accept_orient_surface(guide, None);
    let target = f.target(f.uv, app.document.tolerance()).unwrap();
    assert!(app.accept_drafting_point(target));
    let plane = app
        .orient_surface_session
        .as_ref()
        .unwrap()
        .angle_plane()
        .unwrap();
    assert!(
        app.accept_drafting_point(
            target
                .translated(plane.x_axis().as_vector().scaled(2.).unwrap())
                .unwrap()
        )
    );
    assert!(app.accept_drafting_point(target.translated(plane.y_axis().as_vector()).unwrap()));
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), Some("OrientOnSrf"));
}
#[test]
fn orient_surface_freezes_source_and_destination_planes_independently() {
    use viboceros_geometry::PointMorph;
    let mut f = recipe();
    f.options.rigid = false;
    f.options.constrain_normal = true;
    f.options.copy = false;
    let (mut app, source, guide) = setup(&f);
    let source_plane = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        app.document.tolerance(),
    )
    .unwrap();
    app.viewports[app.active_viewport].set_construction_plane(source_plane);
    enter(
        &mut app,
        "OrientOnSrf ScalePrompt=Yes RotationPrompt=Yes Rigid=No ConstrainNormal=Yes Copy=No",
    );
    enter(&mut app, "w0,0,0");
    app.viewports[app.active_viewport]
        .set_construction_plane(viboceros_command::construction_plane::WorldPlane::Top.frame());
    enter(&mut app, "w1,0,0");
    app.accept_orient_surface(guide, None);
    let q = f.target(f.uv, app.document.tolerance()).unwrap();
    app.accept_drafting_point(q);
    let normal = Vector3::try_new(0., 0., 1.).unwrap();
    let expected = app
        .orient_surface_session
        .as_ref()
        .unwrap()
        .deformation(normal)
        .unwrap()
        .placement
        .morph_at(q, normal)
        .unwrap()
        .morph_point(point(0., 1., 0.))
        .unwrap();
    app.viewports[app.active_viewport]
        .set_construction_plane(viboceros_command::construction_plane::WorldPlane::Right.frame());
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    let Geometry::PointCloud(c) = app.document.object(source).unwrap().geometry() else {
        panic!("source cloud")
    };
    assert!(c.points()[2].is_near(expected, app.document.tolerance()));
}
#[test]
fn orient_surface_negative_scale_is_rejected_and_negative_angle_is_preserved() {
    let f = recipe();
    let (mut app, source, guide) = setup(&f);
    enter(
        &mut app,
        "OrientOnSrf ScalePrompt=Yes RotationPrompt=Yes Copy=No",
    );
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,0");
    app.accept_orient_surface(guide, None);
    let q = f.target(f.uv, app.document.tolerance()).unwrap();
    app.accept_drafting_point(q);
    enter(&mut app, "-2");
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "1");
    enter(&mut app, "-90");
    assert!(app.active_command.is_none());
    let Geometry::PointCloud(c) = app.document.object(source).unwrap().geometry() else {
        panic!("source cloud")
    };
    let Geometry::NurbsSurface(s) = app.document.object(guide).unwrap().geometry() else {
        panic!("guide")
    };
    let y = s
        .frame_at(f.uv[0], f.uv[1], app.document.tolerance())
        .unwrap()
        .y_axis()
        .as_vector();
    assert!(c.points()[1].is_near(
        q.translated(y.scaled(-1.).unwrap()).unwrap(),
        app.document.tolerance()
    ));
}
#[test]
fn orient_surface_trimmed_guide_clamps_targets_and_ignore_trims_uses_full_domain() {
    use viboceros_geometry::Brep;
    let f = recipe();
    let (mut app, _, old_guide) = setup(&f);
    let Geometry::NurbsSurface(s) = app.document.object(old_guide).unwrap().geometry().clone()
    else {
        panic!("surface")
    };
    let guide = app
        .document
        .add_geometry(Geometry::Brep(
            Brep::try_rectangular_surface_face(
                s.clone(),
                0.2..=0.7,
                0.2..=0.8,
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    enter(
        &mut app,
        "OrientOnSrf ScalePrompt=Yes RotationPrompt=Yes Copy=Yes",
    );
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,0");
    app.accept_component_face_hit(guide, 0, None);
    let outside = s.evaluate(0.05, 0.5).unwrap();
    let clamped = app.resolve_orient_surface_point(outside).unwrap();
    assert!(outside.distance_to(clamped).unwrap() > 0.1);
    enter(&mut app, "IgnoreTrims=Yes");
    assert!(
        app.resolve_orient_surface_point(outside)
            .unwrap()
            .is_near(outside, app.document.tolerance())
    );
    enter(&mut app, "Cancel");
    assert!(app.document.undo_label().is_some()); // Guide creation only.
}

#[test]
fn orient_surface_option_menus_persist_on_cancel_without_model_edits() {
    let f = recipe();
    let (mut app, _, _) = setup(&f);
    let before = format!("{:?}", app.document);
    enter(&mut app, "OrientOnSurface Copy=No");
    for text in [
        "Scale", "X", "2", "Y=3", "Z=4", "", "Rotation", "Angle", "-45", "",
    ] {
        enter(&mut app, text);
    }
    let options = app.commands.orient_surface_options();
    assert_eq!(options.axes, Some([2., 3., 4.]));
    assert_eq!(options.rotation, -45.);
    assert!(!options.scale_prompt && !options.rotation_prompt);
    let accepted = options;
    enter(&mut app, "Scale");
    for invalid in ["X=NaN", "X=0", "Uniform=0", "X=2,3", "Prompt=Maybe"] {
        enter(&mut app, invalid);
        assert_eq!(app.commands.orient_surface_options(), accepted);
    }
    enter(&mut app, "Cancel");
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "_OrientOnSrf");
    assert_eq!(
        app.commands.orient_surface_options().axes,
        Some([2., 3., 4.])
    );
    // Canceling before placement leaves Copy under its existing policy.
    assert_eq!(app.commands.copy_default("OrientOnSrf"), Some(true));
    enter(&mut app, "Scale");
    enter(&mut app, "Prompt=Yes");
    enter(&mut app, "");
    enter(&mut app, "Rotation");
    enter(&mut app, "Prompt");
    enter(&mut app, "");
    assert!(app.commands.orient_surface_options().scale_prompt);
    assert!(app.commands.orient_surface_options().rotation_prompt);
    enter(&mut app, "Cancel");
    assert!(test_app().commands.orient_surface_options().axes.is_none());
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn orient_surface_fixed_and_conditional_getters_copy_in_one_history_group() {
    for scale_prompt in [false, true] {
        for rotation_prompt in [false, true] {
            let f = recipe();
            let (mut app, _, guide) = setup(&f);
            enter(
                &mut app,
                &format!(
                    "OrientOnSrf Copy=Yes Scale=1.4 Rotation=30 ScalePrompt={} RotationPrompt={}",
                    if scale_prompt { "Yes" } else { "No" },
                    if rotation_prompt { "Yes" } else { "No" }
                ),
            );
            enter(&mut app, "w0,0,0");
            enter(&mut app, "w1,0,0");
            app.accept_orient_surface(guide, None);
            let target = f.target([0.3, 0.4], app.document.tolerance()).unwrap();
            for i in 0..2 {
                assert!(app.accept_drafting_point(target));
                if scale_prompt {
                    assert!(app.command_log.back().unwrap().contains("scale factor"));
                    enter(&mut app, if i == 0 { "1.4" } else { "0.8" });
                }
                if rotation_prompt {
                    assert!(app.command_log.back().unwrap().contains("rotation angle"));
                    enter(&mut app, if i == 0 { "30" } else { "-45" });
                }
                assert_eq!(app.document.objects().count(), 3 + i);
                assert!(app.orient_surface_location().is_some());
            }
            enter(&mut app, "");
            assert!(app.active_command.is_none());
            let preferences = app.commands.orient_surface_options();
            assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnSrf"));
            assert_eq!(app.document.objects().count(), 2);
            assert!(app.document.undo().unwrap().is_none());
            assert_eq!(app.commands.orient_surface_options(), preferences);
            app.document.redo().unwrap();
            assert_eq!(app.document.objects().count(), 4);
        }
    }
}

#[test]
fn orient_surface_normal_defaults_place_immediately_and_dash_requests_values() {
    for name in ["OrientOnSrf Copy=No", "_-OrientOnSrf Copy=No"] {
        let f = recipe();
        let (mut app, _, guide) = setup(&f);
        enter(&mut app, name);
        enter(&mut app, "w0,0,0");
        enter(&mut app, "w1,0,0");
        app.accept_orient_surface(guide, None);
        assert!(app.accept_drafting_point(f.target([0.3, 0.4], app.document.tolerance()).unwrap()));
        if name.starts_with("_-") {
            assert!(app.active_command.is_some());
            assert!(app.document.undo_label().is_none());
            enter(&mut app, "");
            enter(&mut app, "");
        }
        assert!(app.active_command.is_none());
        assert_eq!(app.document.undo_label(), Some("OrientOnSrf"));
    }
}
