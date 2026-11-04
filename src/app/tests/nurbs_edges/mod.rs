use super::super::object_selection::ObjectPromptPhase;
use super::*;
mod viewport;
use crate::viewport::{ComponentClick, ComponentPick, ComponentWindow};
use viboceros_command::ComponentSelectionKind as Kind;
use viboceros_geometry::SubD;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn fixture(mesh: bool) -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    let points = vec![
        point(0., 0., 0.),
        point(2., 0., 1.),
        point(4., 0., 0.),
        point(0., 3., 0.),
        point(2., 3., 2.),
        point(4., 3., 0.),
    ];
    let geometry = if mesh {
        Geometry::Mesh(
            TriangleMesh::try_new_faces(
                points,
                vec![MeshFace::Quad([0, 1, 4, 3]), MeshFace::Quad([1, 2, 5, 4])],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        )
    } else {
        Geometry::SubD(
            SubD::try_from_polygons(points, vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]]).unwrap(),
        )
    };
    let id = app.document.add_geometry(geometry).unwrap();
    app.document.clear_history().unwrap();
    (app, id)
}
fn component(id: ObjectId, mesh: bool, index: usize) -> ComponentPick {
    ComponentPick {
        object: id,
        index,
        kind: if mesh { Kind::MeshEdge } else { Kind::SubDEdge },
    }
}
fn click(
    app: &mut VibocerosApp,
    pick: ComponentPick,
    preselection: bool,
    modifiers: egui::Modifiers,
) {
    app.accept_component_click(ComponentClick {
        picks: vec![pick],
        preselection,
        modifiers,
    });
}
fn expected(app: &VibocerosApp, id: ObjectId, indices: &[usize], flat: bool) -> Vec<Geometry> {
    let geometry = app.document.object(id).unwrap().geometry();
    if let Geometry::SubD(s) = geometry
        && !flat
    {
        return s
            .edge_nurbs_curves(indices)
            .unwrap()
            .into_iter()
            .map(Geometry::NurbsCurve)
            .collect();
    }
    let edges = match geometry {
        Geometry::Mesh(m) => m.topology_edge_points(),
        Geometry::SubD(s) => s
            .edges()
            .iter()
            .map(|e| e.map(|v| s.control_points()[v as usize]))
            .collect(),
        _ => panic!(),
    };
    indices
        .iter()
        .map(|&i| {
            let [a, b] = edges[i];
            let l = a.distance_to(b).unwrap();
            Geometry::NurbsCurve(NurbsCurve::try_new(1, vec![a, b], vec![0., 0., l, l]).unwrap())
        })
        .collect()
}
fn outputs(app: &VibocerosApp, skip: usize) -> Vec<Geometry> {
    app.document
        .objects()
        .skip(skip)
        .map(|o| o.geometry().clone())
        .collect()
}

#[test]
fn preselected_edges_keep_owners_and_clear_components_through_one_history_edit() {
    for mesh in [false, true] {
        for flat in [false, true] {
            let (mut app, id) = fixture(mesh);
            app.document.set_subd_control_net(flat);
            for i in [3, 0] {
                click(
                    &mut app,
                    component(id, mesh, i),
                    true,
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                );
            }
            let curves = expected(&app, id, &[3, 0], flat);
            let source = app.document.object(id).unwrap().clone();
            enter(&mut app, "ToNURBS");
            let pending = app.object_prompt.as_ref().unwrap();
            assert_eq!(pending.phase, ObjectPromptPhase::Options);
            assert!(!pending.postselected);
            assert!(pending.description.menus.is_empty());
            enter(&mut app, "SubDOptions");
            assert_eq!(app.command_input, "SubDOptions");
            enter(&mut app, "DeleteInputObjects=Yes");
            enter(&mut app, "");
            assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
            assert!(
                app.component_selection
                    .checked_picks(&app.document)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(app.document.object(id), Some(&source));
            assert_eq!(outputs(&app, 1), curves);
            assert_eq!(app.document.selected_object_count(), 0);
            let after = app.document.objects().cloned().collect::<Vec<_>>();
            enter(&mut app, "Undo");
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                [source]
            );
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}

#[test]
fn escape_releases_edge_picks_without_consuming_redo_or_option_memory() {
    for pre in [false, true] {
        for confirmation in [false, true] {
            if pre && !confirmation {
                continue;
            }
            let (mut app, id) = fixture(false);
            enter(&mut app, "Point 30,0,0");
            enter(&mut app, "Undo");
            let before = app.document.objects().cloned().collect::<Vec<_>>();
            if pre {
                click(
                    &mut app,
                    component(id, false, 3),
                    true,
                    egui::Modifiers::default(),
                );
            }
            enter(&mut app, "ToNURBS");
            if !pre {
                click(
                    &mut app,
                    component(id, false, 3),
                    false,
                    egui::Modifiers::default(),
                );
                if confirmation {
                    enter(&mut app, "");
                }
            }
            if confirmation {
                enter(&mut app, "DeleteInputObjects=Yes");
            }
            app.cancel_interactive_command(true);
            assert!(app.object_prompt.is_none());
            assert!(
                app.component_selection
                    .checked_picks(&app.document)
                    .unwrap()
                    .is_empty()
            );
            assert!(app.document.can_redo());
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(app.document.selected_object_count(), 0);
            click(
                &mut app,
                component(id, false, 0),
                true,
                egui::Modifiers::default(),
            );
            enter(&mut app, "ToNURBS");
            assert!(!app.object_prompt.as_ref().unwrap().description.options[0].value);
        }
    }
}

#[test]
fn prompted_mixed_picks_track_removal_clear_and_component_action_order() {
    let (mut app, id) = fixture(false);
    enter(&mut app, "Line 20,0,0 24,1,0");
    let line = app.document.objects().last().unwrap().id();
    app.document.clear_history().unwrap();
    enter(&mut app, "ToNURBS");
    assert!(app.picking_nurbs_edges());
    assert!(app.component_preselection_available());
    for kind in [Kind::SubDFace, Kind::SubDVertex] {
        click(
            &mut app,
            ComponentPick {
                object: id,
                index: 0,
                kind,
            },
            false,
            egui::Modifiers::default(),
        );
    }
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    app.accept_component_window(ComponentWindow {
        picks: vec![component(id, false, 0), component(id, false, 3)],
        preselection: false,
        modifiers: egui::Modifiers::default(),
        crossing: true,
        inverted: false,
    });
    click(
        &mut app,
        component(id, false, 0),
        false,
        egui::Modifiers::CTRL,
    );
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap(),
        [component(id, false, 3)]
    );
    enter(&mut app, "SelNone");
    assert!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .is_empty()
    );
    click(
        &mut app,
        component(id, false, 3),
        true,
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(line),
        mode: SelectionMode::Replace,
    });
    click(
        &mut app,
        component(id, false, 0),
        true,
        egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    );
    let edges = expected(&app, id, &[3, 0], false);
    let curve = app
        .document
        .object(line)
        .unwrap()
        .geometry()
        .converted_to_nurbs_curve()
        .unwrap()
        .unwrap();
    let source = app.document.object(id).unwrap().clone();
    enter(&mut app, "");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().phase,
        ObjectPromptPhase::Options
    );
    enter(&mut app, "DeleteInputObjects=Yes");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(
        outputs(&app, 1),
        [
            edges[0].clone(),
            Geometry::NurbsCurve(curve),
            edges[1].clone()
        ]
    );
    assert_eq!(app.document.object(id), Some(&source));
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn nested_display_toggle_uses_input_appearance_and_survives_history() {
    for flat in [false, true] {
        let (mut app, id) = fixture(false);
        app.document.set_subd_control_net(flat);
        let curves = expected(&app, id, &[3], flat);
        click(
            &mut app,
            component(id, false, 3),
            true,
            egui::Modifiers::default(),
        );
        enter(&mut app, "ToNURBS");
        let pending = app.object_prompt.clone();
        enter(&mut app, "SubDDisplayToggle");
        assert_eq!(app.object_prompt, pending);
        assert_eq!(app.document.subd_control_net(), !flat);
        enter(&mut app, "");
        assert_eq!(outputs(&app, 1), curves);
        enter(&mut app, "Undo");
        assert_eq!(app.document.subd_control_net(), !flat);
        enter(&mut app, "Redo");
        assert_eq!(outputs(&app, 1), curves);
    }
}

#[test]
fn changed_sources_attributes_groups_selection_or_tolerance_reject_confirmation() {
    for change in 0..6 {
        let (mut app, id) = fixture(false);
        click(
            &mut app,
            component(id, false, 3),
            true,
            egui::Modifiers::default(),
        );
        enter(&mut app, "ToNURBS");
        match change {
            0 => {
                app.document
                    .set_object_names([(id, Some("changed".into()))])
                    .unwrap();
            }
            1 => {
                app.document
                    .add_group(Some("changed".into()), [id])
                    .unwrap();
            }
            2 => {
                app.document
                    .set_tolerance(Tolerance::try_new(0.01, 1e-6, 1e-6).unwrap());
            }
            3 => {
                app.document
                    .select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            4 => app.component_selection.clear(),
            _ => {
                app.document
                    .replace_object_geometries(vec![(id, Geometry::Point(point(9., 0., 0.)))])
                    .unwrap();
            }
        }
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let history = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "");
        assert!(app.object_prompt.is_some());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(app.document.undo_label(), history.as_deref());
        assert!(
            app.command_log.back().unwrap().contains("input changed"),
            "{:?}",
            app.command_log
        );
    }
}
