use super::boolean_intersection::enter;
use super::*;
fn pair() -> (VibocerosApp, [ObjectId; 2]) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0 4,0 4,4 0,4");
    enter(&mut app, "SrfPt 2,1 5,1 5,3 2,3");
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    (app, [ids[0], ids[1]])
}
#[test]
fn planar_difference_and_intersection_pick_ordered_surfaces_and_finish_on_second_pick() {
    for (name, area) in [("PlanarDifference", 12.), ("PlanarIntersection", 4.)] {
        let (mut app, ids) = pair();
        enter(&mut app, name);
        assert!(!app.command_line_idle());
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[0]),
            mode: SelectionMode::Replace,
        });
        assert!(!app.document.can_undo());
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[1]),
            mode: SelectionMode::Replace,
        });
        assert!(app.planar_boolean_prompt.is_none());
        let Geometry::Brep(b) = app.document.objects().next().unwrap().geometry() else {
            panic!()
        };
        assert!((b.area(app.document.tolerance()).unwrap() - area).abs() < 1e-9);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), 1);
    }
}
#[test]
fn planar_union_set_getter_and_cancel_leave_sources_intact() {
    let (mut app, ids) = pair();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "PlanarUnion");
    for id in ids {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Add,
        });
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "PlanarDifference");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn planar_preselection_uses_native_command_admission_and_empty_results() {
    for name in ["PlanarUnion", "PlanarDifference", "PlanarIntersection"] {
        let (mut app, ids) = pair();
        app.document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        enter(&mut app, name);
        if name == "PlanarDifference" {
            assert!(app.planar_boolean_prompt.is_some());
            assert_eq!(app.document.objects().len(), 2);
            for id in ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
            }
        } else {
            assert!(app.planar_boolean_prompt.is_none());
        }
        assert_eq!(
            app.document.objects().len(),
            1,
            "{name}: {:?}",
            app.command_log
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(
            app.document.selected_object_count(),
            if name == "PlanarDifference" { 0 } else { 2 }
        );
        enter(&mut app, "Redo");
        assert_eq!(app.document.selected_object_count(), 0);
    }
}

#[test]
fn planar_application_replays_all_native_recipes_and_history() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/planar_boolean_command.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let name = v["command_name"].as_str().unwrap();
        let mut app = test_app();
        for shape in v["shapes"].as_array().unwrap() {
            let points = serde_json::from_value::<[[f64; 3]; 4]>(shape["points"].clone())
                .unwrap()
                .map(|p| viboceros_geometry::Point3::try_from(p).unwrap());
            let b = viboceros_geometry::Brep::try_surface_face(
                viboceros_geometry::NurbsSurface::try_bilinear(points).unwrap(),
                app.document.tolerance(),
            )
            .unwrap();
            app.document
                .add_geometry(Geometry::Brep(if shape["reverse"] == true {
                    b.reversed()
                } else {
                    b
                }))
                .unwrap();
        }
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        if v["pre"] == true {
            app.document
                .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, name);
        if v["pre"] != true || name == "PlanarDifference" {
            for id in &ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(*id),
                    mode: SelectionMode::Add,
                });
            }
            if name == "PlanarUnion" {
                enter(&mut app, "");
            }
        }
        assert!(app.planar_boolean_prompt.is_none());
        assert!(
            app.object_prompt.is_none(),
            "{} {:?}",
            v["case"],
            app.command_log
        );
        let native = v["command"]["after_script"].as_array().unwrap();
        assert_eq!(app.document.objects().len(), native.len());
        for (o, n) in app.document.objects().zip(native) {
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            assert_eq!(b.faces().len(), n["faces"].as_u64().unwrap() as usize);
            assert_eq!(b.edges().len(), n["edges"].as_u64().unwrap() as usize);
            assert!(
                (b.area(app.document.tolerance()).unwrap() - n["area"].as_f64().unwrap()).abs()
                    < 1e-9
            );
        }
        assert!(app.document.can_undo());
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            app.document.selected_object_count(),
            v["undo"]["after_script"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["selected"] == true)
                .count()
        );
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), native.len());
        assert_eq!(app.document.selected_object_count(), 0);
    }
}
