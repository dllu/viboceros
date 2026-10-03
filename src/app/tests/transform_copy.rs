//! Replay independently prescribed command input against unmodified native snapshots.
use super::*;
use serde_json::{Value, json};
use viboceros_document::ObjectColorSource;
use viboceros_geometry::{
    Brep, Circle3, CircularArc3, LineSegment, NurbsSurface, Polyline3, Vector3,
};

#[path = "../../../crates/viboceros-oracle/src/test_json.rs"]
mod test_json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}

#[test]
fn rejected_targets_and_escape_keep_accepted_edits_and_last_successful_copy_choice() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    app.document.clear_history().unwrap();
    enter(&mut app, "Scale _Copy=_Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "2");
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.active_command.is_some());
    let accepted = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Copy=No");
    enter(&mut app, "0");
    enter(&mut app, "NaN");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        accepted
    );
    assert!(app.active_command.is_some());
    assert_eq!(app.commands.copy_default("Scale"), Some(true));
    enter(&mut app, "Copy=Maybe");
    assert!(app.active_command.is_some());
    enter(&mut app, "Copy Yes");
    enter(&mut app, "3");
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Copy=No");
    app.cancel_current_prompt_or_selection(); // The real GUI Escape route.
    assert!(app.active_command.is_none());
    assert!(app.transform_session.is_none());
    assert_eq!(app.commands.copy_default("Scale"), Some(true));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn empty_command_input_accepts_scalar_defaults_and_shows_the_pending_value() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    enter(&mut app, "Scale 0,0,0 3");
    enter(&mut app, "Undo");
    enter(&mut app, "Scale");
    enter(&mut app, "w0,0,0");
    assert_eq!(
        app.transform_default_hint().as_deref(),
        Some("Enter accepts the default: 3")
    );
    enter(&mut app, ""); // The command field submitted by the real Enter key.
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), Some("Scale"));
    let Geometry::Point(point) = app.document.objects().next().unwrap().geometry() else {
        panic!("point")
    };
    assert_eq!(point.to_array(), [6., 9., 12.]);
}

fn snapshot(app: &VibocerosApp, sources: &[ObjectId]) -> Value {
    // Mirror Object's independently constructed plane target is checked
    // separately; these snapshots retain the affine point witnesses.
    let objects = app
        .document
        .objects()
        .filter(|object| matches!(object.geometry(), Geometry::Point(_)))
        .collect::<Vec<_>>();
    let groups = app.document.groups().collect::<Vec<_>>();
    json!({
        "objects": objects.iter().map(|object| {
            let Geometry::Point(point) = object.geometry() else { panic!("unexpected geometry") };
            let attributes = object.attributes();
            let color = attributes.object_color();
            json!({
                "source": sources.iter().position(|id| *id == object.id()),
                "selected": app.document.is_selected(object.id()),
                "point": point.to_array(),
                "name": attributes.name().unwrap_or(""),
                "color": [color.red, color.green, color.blue],
                "color_source": match attributes.color_source() {
                    ObjectColorSource::Object => "ColorFromObject",
                    ObjectColorSource::Layer => "ColorFromLayer",
                    ObjectColorSource::Material => "ColorFromMaterial",
                    ObjectColorSource::Parent => "ColorFromParent",
                },
                "current_layer": attributes.layer_id() == app.document.current_layer_id(),
                "groups": object.group_ids().iter().map(|id| groups.iter().position(|group| group.id() == *id).unwrap()).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "groups": groups.iter().map(|group| json!({
            "members": objects.iter().enumerate().filter_map(|(i, obj)| obj.group_ids().contains(&group.id()).then_some(i)).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    })
}

#[test]
fn repeated_transform_input_geometry_selection_groups_and_history_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_copy.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_copy.json"),
        49,
        Invocation::Prompt,
    );
}

#[test]
fn scale1d_typed_projection_and_rejected_zero_targets_match_six_native_captures() {
    for invocation in [Invocation::Prompt, Invocation::Registry] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/scale1d_projection.json"),
            include_str!("../../../tools/rhino_oracle/observations/scale1d_projection.json"),
            6,
            invocation,
        );
    }
}

#[test]
fn move_copy_placement_selection_order_options_and_history_match_native() {
    for invocation in [Invocation::Prompt, Invocation::TranslationRegistry] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/translation.json"),
            include_str!("../../../tools/rhino_oracle/observations/translation.json"),
            110,
            invocation,
        );
    }
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/translation_edges.json"),
        include_str!("../../../tools/rhino_oracle/observations/translation_edges.json"),
        36,
        Invocation::Prompt,
    );
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/translation_mouse.json"),
        include_str!("../../../tools/rhino_oracle/observations/translation_mouse.json"),
        18,
        Invocation::Prompt,
    );
}

#[test]
fn move_normal_references_signed_distances_selection_and_history_match_native() {
    for invocation in [Invocation::Prompt, Invocation::NormalRegistry] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/move_normal.json"),
            include_str!("../../../tools/rhino_oracle/observations/move_normal.json"),
            102,
            invocation,
        );
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/move_normal_edges.json"),
            include_str!("../../../tools/rhino_oracle/observations/move_normal_edges.json"),
            20,
            invocation,
        );
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/move_normal_trims.json"),
            include_str!("../../../tools/rhino_oracle/observations/move_normal_trims.json"),
            8,
            invocation,
        );
    }
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/move_normal_defaults.json"),
        include_str!("../../../tools/rhino_oracle/observations/move_normal_defaults.json"),
        8,
        Invocation::Prompt,
    );
}

#[test]
fn normal_reference_options_survive_escape_and_undefined_direction_keeps_sources() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    let source = app.document.objects().next().unwrap().id();
    let reference = app
        .document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                point(0., 0., 0.),
                point(6., 2., 0.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Move Normal");
    assert!(app.selecting_move_normal_reference());
    assert!(!app.accept_move_normal_reference(source, None)); // Point is not a normal reference.
    enter(&mut app, "IgnoreTrims=Yes");
    app.cancel_current_prompt_or_selection();
    enter(&mut app, "Move Normal");
    assert!(
        app.translation_session
            .as_ref()
            .unwrap()
            .normal
            .as_ref()
            .unwrap()
            .ignore_trims
    );
    assert!(app.accept_move_normal_reference(reference, None));
    assert!(!app.document.is_selected(reference));
    enter(&mut app, "w3,1,0");
    assert!(app.translation_session.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.is_selected(source));
    assert!(!app.document.can_undo());
}

#[test]
fn move_copy_rejected_geometry_preserves_accepted_copies_and_prompt_phase() {
    let mut app = test_app();
    enter(&mut app, "Point 1e308,0,0");
    enter(&mut app, "SelAll");
    app.document.clear_history().unwrap();
    enter(&mut app, "Copy");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,0");
    let accepted = app.document.objects().cloned().collect::<Vec<_>>();
    let placement = app.translation_session.as_ref().unwrap().placement.unwrap();
    enter(&mut app, "w1e308,0,0"); // Source + offset overflows.
    enter(&mut app, "UseLastDirection=Maybe");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        accepted
    );
    assert_eq!(
        app.translation_session.as_ref().unwrap().placement,
        Some(placement)
    );
    assert!(app.active_command.is_some());
    app.cancel_current_prompt_or_selection();
    assert!(app.translation_session.is_none());
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    assert_eq!(app.document.selected_object_count(), 0);

    enter(&mut app, "SelAll");
    enter(&mut app, "Move");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Move {
            start: Some(point(0., 0., 0.))
        })
    );
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    assert!(app.translation_session.is_none());
}

#[test]
fn command_first_transform_selection_order_cancellation_and_history_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_sources.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_sources.json"),
        61,
        Invocation::Prompt,
    );
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_sources_identity.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_sources_identity.json"),
        12,
        Invocation::Prompt,
    );
}

#[test]
fn transform_source_mouse_picks_expand_groups_while_selid_is_direct_and_escape_obeys_phase() {
    let mut app = test_app();
    let ids = [0., 1., 2.].map(|x| {
        app.document
            .add_geometry(Geometry::Point(point(x, 0., 0.)))
            .unwrap()
    });
    app.document.add_group(None, [ids[0], ids[2]]).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Mirror Copy=No 3Point");
    assert!(app.object_prompt.is_some());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Any)
    );
    enter(&mut app, "Copy=No");
    assert_eq!(app.commands.copy_default("Mirror"), Some(true));
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[2]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0], ids[2]]
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Remove,
    });
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, &format!("SelID {}", ids[2]));
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[2]]
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());

    enter(&mut app, "Mirror");
    enter(&mut app, &format!("SelID {}", ids[2]));
    enter(&mut app, &format!("SelID {}", ids[1]));
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Mirror { start: None })
    );
    app.cancel_current_prompt_or_selection();
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[2], ids[1]]
    );
    assert!(!app.document.can_undo());
}

#[test]
fn failed_postselected_transform_preserves_picks_and_accepted_copies() {
    let mut app = test_app();
    let ids = [0., 1., 2.].map(|x| {
        app.document
            .add_geometry(Geometry::Point(point(x, 1., 0.)))
            .unwrap()
    });
    app.document.clear_history().unwrap();
    enter(&mut app, "Scale");
    for id in [ids[2], ids[0]] {
        enter(&mut app, &format!("SelID {id}"));
    }
    enter(&mut app, "Enter");
    assert!(app.command_input.is_empty());
    enter(&mut app, "w0,0,0");
    enter(&mut app, "Copy=Yes");
    enter(&mut app, "2");
    let accepted = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Copy=No");
    enter(&mut app, "NaN");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        accepted
    );
    assert_eq!(app.document.selected_object_count(), 2);
    assert!(app.document.is_selected(ids[2]) && app.document.is_selected(ids[0]));
    assert!(app.active_command.is_some());
    enter(&mut app, "3");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "SelLast");
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn postselected_transform_edits_restricted_group_peers_and_rolls_back_geometry_failure() {
    for copy in [false, true] {
        let mut app = test_app();
        let ids = [2., 3.].map(|x| {
            app.document
                .add_geometry(Geometry::Point(point(x, 1., 0.)))
                .unwrap()
        });
        app.document.add_group(None, ids).unwrap();
        app.document.set_objects_locked([ids[1]], true).unwrap();
        app.document.clear_history().unwrap();
        enter(&mut app, "Scale");
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[0]),
            mode: SelectionMode::Replace,
        });
        assert_eq!(app.document.selected_object_count(), 2);
        enter(&mut app, "Enter");
        enter(&mut app, "w0,0,0");
        enter(&mut app, if copy { "Copy=Yes" } else { "Copy=No" });
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "1e308");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(app.document.selected_object_count(), 2);
        assert!(app.active_command.is_some());
        assert!(!app.document.can_undo());
        enter(&mut app, "2");
        if copy {
            enter(&mut app, "Enter");
        }
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert_eq!(
            app.document.selected_object_count(),
            if copy { 2 } else { 0 }
        );
        assert_eq!(app.document.objects().len(), if copy { 4 } else { 2 });
        assert_eq!(
            app.document.objects().last().unwrap().geometry(),
            &Geometry::Point(point(6., 2., 0.))
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(app.document.selected_object_count(), 0);
    }
}

#[test]
fn complete_transform_invocations_geometry_selection_groups_and_history_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_script.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_copy_script.json"),
        56,
        Invocation::Registry,
    );
}

#[test]
fn transform_prompts_with_partial_groups_and_untouched_peers_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_script.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_copy_script.json"),
        56,
        Invocation::Prompt,
    );
}

#[test]
fn automatic_scale_centers_with_rotated_and_tilted_planes_match_native() {
    replay_native(
        include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_center.json"),
        include_str!("../../../tools/rhino_oracle/observations/transform_copy_center.json"),
        18,
        Invocation::Prompt,
    );
}

#[test]
fn exact_and_near_identity_transform_prompts_and_invocations_match_native() {
    for invocation in [Invocation::Prompt, Invocation::Registry] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_identity.json"),
            include_str!("../../../tools/rhino_oracle/observations/transform_copy_identity.json"),
            64,
            invocation,
        );
    }
}

#[test]
fn transform_scalar_defaults_and_cancellation_match_self_seeded_native_sessions() {
    for invocation in [Invocation::Prompt, Invocation::RegistrySeeds] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/transform_copy_default.json"),
            include_str!("../../../tools/rhino_oracle/observations/transform_copy_default.json"),
            80,
            invocation,
        );
    }
}

#[test]
fn mirror_plane_options_geometry_groups_and_history_match_native() {
    for invocation in [
        Invocation::Prompt,
        Invocation::RegistrySeeds,
        Invocation::InlineMirrorOptions,
    ] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/mirror_planes.json"),
            include_str!("../../../tools/rhino_oracle/observations/mirror_planes.json"),
            64,
            invocation,
        );
    }
}

#[test]
fn mirror_enter_ends_every_unfinished_point_prompt_without_an_edit() {
    for invocation in [Invocation::Prompt, Invocation::InlineMirrorOptions] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/mirror_enter.json"),
            include_str!("../../../tools/rhino_oracle/observations/mirror_enter.json"),
            5,
            invocation,
        );
    }
}

#[test]
fn mirror_object_targets_geometry_selection_groups_and_history_match_native() {
    for invocation in [
        Invocation::Prompt,
        Invocation::InlineMirrorOptions,
        Invocation::MirrorObjectRegistry,
    ] {
        replay_native(
            include_str!("../../../tools/rhino_oracle/fixtures/mirror_object.json"),
            include_str!("../../../tools/rhino_oracle/observations/mirror_object.json"),
            70,
            invocation,
        );
    }
}

#[test]
fn mirror_object_rejected_targets_preserve_geometry_history_and_pending_source_ids() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "Point 13,-4,8");
    let sources = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let corners = [
        point(20., -5., 1.),
        point(30., -5., 1.),
        point(30., 5., 4.),
        point(20., 5., 1.),
    ];
    let curved = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear(corners).unwrap(),
        ))
        .unwrap();
    let mesh = app
        .document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new_faces(
                corners.to_vec(),
                vec![MeshFace::Quad([0, 1, 2, 3])],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct(sources.iter().copied(), SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Mirror Object Copy=Yes");
    assert!(app.picking_mirror_object());
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.accept_mirror_object(curved, None));
    assert!(!app.accept_mirror_object(mesh, Some(0)));
    assert!(!app.accept_mirror_object(sources[0], None));
    assert!(app.picking_mirror_object());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(app.transform_session.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());
}

#[test]
fn mirror_object_surface_and_face_picks_use_spatial_planes_and_leave_targets_unchanged() {
    for face in [None, Some(0)] {
        let mut app = test_app();
        enter(&mut app, "Point 2,3,4");
        let source = app.document.objects().next().unwrap().id();
        let surface = NurbsSurface::try_bilinear([
            point(0., 0., 1.),
            point(5., 0., 1.),
            point(5., 4., 4.),
            point(0., 4., 4.),
        ])
        .unwrap();
        let geometry = if face.is_some() {
            Geometry::Brep(
                Brep::try_rectangular_surface_face(
                    surface,
                    0.0..=1.0,
                    0.0..=1.0,
                    app.document.tolerance(),
                )
                .unwrap(),
            )
        } else {
            Geometry::NurbsSurface(surface)
        };
        let target = app.document.add_geometry(geometry).unwrap();
        let target_before = app.document.object(target).unwrap().clone();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        enter(&mut app, "_-mIrRoR _Object _Copy=_Yes");
        assert!(app.picking_mirror_object());
        assert_eq!(app.document.selected_object_count(), 0);
        assert!(!app.accept_mirror_object(target, Some(99)));
        if let Some(face) = face {
            app.accept_component_face_hit(target, face, None);
        } else {
            app.apply_selection_click(SelectionClick {
                object_id: Some(target),
                mode: SelectionMode::Replace,
            });
        }
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let copy = app
            .document
            .objects()
            .find(|object| object.id() != source && object.id() != target)
            .unwrap();
        let Geometry::Point(p) = copy.geometry() else {
            panic!("point")
        };
        for (actual, expected) in p.to_array().into_iter().zip([2., 3.72, 3.04]) {
            assert!((actual - expected).abs() < 1e-9);
        }
        let copy_id = copy.id();
        assert_eq!(app.document.object(target), Some(&target_before));
        assert_eq!(app.document.selected_object_count(), 0);
        assert_eq!(app.document.undo_label(), Some("Mirror"));
        enter(&mut app, "SelLast");
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        assert!(!app.document.can_undo());
        enter(&mut app, "Redo");
        assert!(app.document.is_selected(copy_id));
        assert!(!app.document.is_selected(source));
        assert_eq!(app.document.object(target), Some(&target_before));
    }
}

#[derive(Clone, Copy)]
enum Invocation {
    Prompt,
    Registry,
    RegistrySeeds,
    InlineMirrorOptions,
    MirrorObjectRegistry,
    TranslationRegistry,
    NormalRegistry,
}

fn normal_target_geometry(target: &Value, tolerance: viboceros_geometry::Tolerance) -> Geometry {
    let points = target["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            point(
                p[0].as_f64().unwrap(),
                p[1].as_f64().unwrap(),
                p[2].as_f64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let flip = target["flip"].as_bool().unwrap();
    match target["kind"].as_str().unwrap() {
        "line" => {
            let line = LineSegment::try_new(points[0], points[1], tolerance).unwrap();
            Geometry::Line(if flip { line.reversed() } else { line })
        }
        "circle" => {
            let circle = Circle3::try_new(
                points[0],
                3.,
                viboceros_command::CommandContext::default()
                    .construction_plane
                    .z_axis(),
                tolerance,
            )
            .unwrap();
            Geometry::Circle(if flip { circle.reversed() } else { circle })
        }
        "arc" => {
            let arc =
                CircularArc3::try_from_three_points(points[0], points[1], points[2], tolerance)
                    .unwrap();
            Geometry::Arc(if flip {
                arc.reversed(tolerance).unwrap()
            } else {
                arc
            })
        }
        "polyline" => {
            let curve = Polyline3::try_new(points, tolerance).unwrap();
            Geometry::Polyline(if flip { curve.reversed() } else { curve })
        }
        kind @ ("surface" | "brep" | "trimmed") => {
            let surface = NurbsSurface::try_bilinear(points.try_into().unwrap()).unwrap();
            if kind == "surface" {
                Geometry::NurbsSurface(if flip {
                    surface.try_reversed_u().unwrap()
                } else {
                    surface
                })
            } else {
                let brep = Brep::try_rectangular_surface_face(
                    surface,
                    0.0..=if kind == "trimmed" { 0.5 } else { 1.0 },
                    0.0..=1.0,
                    tolerance,
                )
                .unwrap();
                Geometry::Brep(if flip { brep.reversed() } else { brep })
            }
        }
        _ => panic!("unknown normal reference"),
    }
}

fn mirror_target_geometry(target: &Value, tolerance: viboceros_geometry::Tolerance) -> Geometry {
    let corners: [Point3; 4] = std::array::from_fn(|index| {
        let p = &target["corners"][index];
        point(
            p[0].as_f64().unwrap(),
            p[1].as_f64().unwrap(),
            p[2].as_f64().unwrap(),
        )
    });
    match target["kind"].as_str().unwrap() {
        "surface" | "curved" => {
            Geometry::NurbsSurface(NurbsSurface::try_bilinear(corners).unwrap())
        }
        "trimmed" => Geometry::Brep(
            Brep::try_rectangular_surface_face(
                NurbsSurface::try_bilinear(corners).unwrap(),
                0.0..=1.0,
                0.0..=1.0,
                tolerance,
            )
            .unwrap(),
        ),
        "box" | "extrusion" => {
            let [x, y, z] = corners[0].to_array();
            let [end_x, end_y, _] = corners[2].to_array();
            let brep = Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[x, end_x], [y, end_y], [z, z + 6.]],
                tolerance,
            )
            .unwrap();
            // Prescribed rectangle walls in perimeter order, then bottom and
            // top. This input construction is independent of the observations.
            let faces = [2, 5, 3, 4, 0, 1]
                .map(|index| brep.faces()[index].clone())
                .to_vec();
            Geometry::Brep(
                Brep::try_new(
                    brep.vertices().to_vec(),
                    brep.edges().to_vec(),
                    faces,
                    tolerance,
                )
                .unwrap(),
            )
        }
        "mesh" => Geometry::Mesh(
            TriangleMesh::try_new_faces(
                corners.to_vec(),
                vec![MeshFace::Quad([0, 1, 2, 3])],
                tolerance,
            )
            .unwrap(),
        ),
        "curve" => Geometry::Polyline(
            Polyline3::try_new(corners.into_iter().chain([corners[0]]).collect(), tolerance)
                .unwrap(),
        ),
        _ => panic!("unknown target"),
    }
}

fn replay_native(request: &str, observed: &str, count: usize, invocation: Invocation) {
    let request: Value = serde_json::from_str(request).unwrap();
    let observed: Value = serde_json::from_str(observed).unwrap();
    let operations = request["operations"].as_array().unwrap();
    let results = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), count);
    assert_eq!(results.len(), operations.len());
    assert_eq!(observed["engine"], "rhino");
    let mut failures = Vec::new();
    let mut commands = None;
    for (operation, row) in operations.iter().zip(results) {
        let label = operation["id"].as_str().unwrap();
        assert_eq!(row["id"], label);
        let expected = &row["value"];
        let mut app = test_app();
        if let Some(registry) = commands.take() {
            // The native capture cleans up owned sources between cases while
            // retaining its command-instance preferences. Use fresh documents
            // with the same registry; no measured defaults seed this state.
            app.commands = registry;
        }
        let target = operation
            .get("mirror_target")
            .or(operation.get("normal_target"))
            .map(|target| {
                app.document
                    .add_geometry(if operation.get("normal_target").is_some() {
                        normal_target_geometry(target, app.document.tolerance())
                    } else {
                        mirror_target_geometry(target, app.document.tolerance())
                    })
                    .unwrap()
            });
        let target_before = target.map(|id| app.document.object(id).unwrap().clone());
        // Match the native SDK source construction record. Keeping this
        // baseline also exercises SelLast when the tested transform is a no-op.
        app.document
            .begin_transaction("Transform source setup")
            .unwrap();
        app.active_viewport = 1; // Top; the native probe explicitly uses WorldXY.
        if let Some(plane) = operation.get("cplane") {
            let coords = |value: &Value| {
                [
                    value[0].as_f64().unwrap(),
                    value[1].as_f64().unwrap(),
                    value[2].as_f64().unwrap(),
                ]
            };
            app.viewports[app.active_viewport].set_construction_plane(
                viboceros_geometry::Frame3::try_from_directions(
                    Point3::try_from(coords(&plane["origin"])).unwrap(),
                    Vector3::try_from(coords(&plane["x_axis"])).unwrap(),
                    Vector3::try_from(coords(&plane["y_axis"])).unwrap(),
                    app.document.tolerance(),
                )
                .unwrap(),
            );
        }
        let sources = operation["sources"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, coords)| {
                let point = point(
                    coords[0].as_f64().unwrap(),
                    coords[1].as_f64().unwrap(),
                    coords[2].as_f64().unwrap(),
                );
                let attributes =
                    viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                        .with_name(format!("source-{index}"))
                        .with_object_color(ColorRgb::new(10 + index as u8, 30, 50));
                let id = app
                    .document
                    .add_geometry_with_attributes(Geometry::Point(point), attributes)
                    .unwrap();
                id
            })
            .collect::<Vec<_>>();
        if operation["grouped"].as_bool().unwrap() {
            app.document
                .add_group(
                    Some(format!("RepeatSource_{label}")),
                    sources.iter().copied(),
                )
                .unwrap();
        }
        let selected = operation
            .get("selected")
            .map(|values| {
                values
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|index| sources[index.as_u64().unwrap() as usize])
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                if operation.get("source_selection").is_some() {
                    Vec::new()
                } else {
                    sources.clone()
                }
            });
        app.document
            .select_objects_direct(selected, SelectionMode::Replace)
            .unwrap();
        app.document.commit_transaction().unwrap();
        let compare = |actual: Value, phase: &str, failures: &mut Vec<String>| {
            let path = format!("{label}/{phase}");
            // Curve mouse locations are bounded screen-space minimizations.
            // Native circle picks themselves differ from the analytic camera
            // ray reference by >1e-9; retain the raw outputs with a 2e-8 budget.
            // Typed inputs and surface mouse picks retain the tighter budget.
            let epsilon = if operation["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|input| input == "NormalBase")
                && matches!(
                    operation["normal_target"]["kind"].as_str(),
                    Some("circle" | "arc")
                ) {
                2e-8
            } else {
                1e-9
            };
            if std::panic::catch_unwind(|| {
                test_json::close(&actual, &expected[phase], &path, epsilon, 0.)
            })
            .is_err()
            {
                failures.push(path);
            }
        };
        compare(snapshot(&app, &sources), "before", &mut failures);
        let use_registry = match invocation {
            Invocation::Prompt => false,
            Invocation::NormalRegistry => {
                operation.get("source_selection").is_none()
                    && matches!(operation["inputs"].as_array().unwrap().len(), 4 | 5)
                    && !operation["inputs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|input| input == "Enter" || input == "NormalBase")
                    && !matches!(
                        operation["normal_target"]["kind"].as_str(),
                        Some("line" | "polyline")
                    )
            }
            Invocation::TranslationRegistry => {
                operation.get("source_selection").is_none()
                    && operation["finish"] != "Cancel"
                    && !operation["id"]
                        .as_str()
                        .unwrap()
                        .contains("early-options-rejected")
                    && !operation["inputs"].as_array().unwrap().iter().any(|input| {
                        input == "Enter"
                            || input == "Undo"
                            || input.as_str().unwrap().parse::<f64>().is_ok()
                    })
            }
            Invocation::InlineMirrorOptions => false,
            Invocation::MirrorObjectRegistry => {
                operation["inputs"].as_array().unwrap().len() == 3
                    && operation["inputs"][1] == "Object"
                    && operation["inputs"][2] == "Target"
                    && matches!(
                        operation["mirror_target"]["kind"].as_str(),
                        Some("surface" | "trimmed" | "box" | "extrusion")
                    )
            }
            Invocation::Registry => true,
            Invocation::RegistrySeeds => {
                operation["finish"] == "Automatic"
                    && !operation["inputs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|input| input == "Enter")
                    && operation["inputs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|input| input.as_str().unwrap().starts_with("Copy="))
                        .count()
                        <= 1
            }
        };
        match use_registry {
            false => {
                let inputs = operation["inputs"].as_array().unwrap();
                let consumed = if matches!(invocation, Invocation::InlineMirrorOptions) {
                    let mut prefix = vec![
                        "_-mIrRoR".to_owned(),
                        inputs[0].as_str().unwrap().replace("Copy=", "_Copy=_"),
                    ];
                    if viboceros_command::mirror::MirrorPlaneOption::from_token(
                        inputs[1].as_str().unwrap(),
                    )
                    .is_some()
                    {
                        prefix.push(format!("_{}", inputs[1].as_str().unwrap()));
                    }
                    let consumed = prefix.len() - 1;
                    enter(&mut app, &prefix.join(" "));
                    consumed
                } else {
                    enter(&mut app, operation["command"].as_str().unwrap());
                    if let Some(steps) = operation.get("source_selection") {
                        assert!(
                            app.object_prompt.is_some(),
                            "{label}: {:?}",
                            app.command_log
                        );
                        assert!(app.transform_session.is_none());
                        for step in steps.as_array().unwrap() {
                            if let Some(index) = step.as_u64() {
                                enter(&mut app, &format!("SelID {}", sources[index as usize]));
                            } else {
                                enter(&mut app, step.as_str().unwrap());
                            }
                        }
                    } else {
                        assert!(
                            app.transform_session.is_some() || app.translation_session.is_some(),
                            "{label}: {:?}",
                            app.command_log
                        );
                    }
                    0
                };
                for input in inputs.iter().skip(consumed) {
                    if input == "Mouse" {
                        // Public native viewing-line calibration is input,
                        // never a measured destination or command result.
                        let ray = &expected["frame"]["ray"];
                        let p = |value: &Value| {
                            point(
                                value[0].as_f64().unwrap(),
                                value[1].as_f64().unwrap(),
                                value[2].as_f64().unwrap(),
                            )
                        };
                        let a = p(&ray[0]);
                        let b = p(&ray[1]);
                        let constraint = app.translation_constraint().unwrap();
                        let candidate = constraint
                            .project_view_line(
                                constraint.anchor.vector_to(a).unwrap(),
                                a.vector_to(b).unwrap(),
                            )
                            .unwrap();
                        assert!(
                            app.accept_drafting_point(candidate),
                            "{label}: {:?}",
                            app.command_log
                        );
                    } else if input == "NormalTarget" {
                        assert!(
                            app.accept_move_normal_reference(target.unwrap(), None),
                            "{label}: {:?}",
                            app.command_log
                        );
                    } else if input == "NormalBase" {
                        let camera = &expected["base_frame"]["camera"];
                        let view = crate::viewport::clip_tests::captured_view(camera);
                        let size: [i32; 2] =
                            serde_json::from_value(camera["viewport_size"].clone()).unwrap();
                        let rect = egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(size[0] as f32, size[1] as f32),
                        );
                        let xy: [i32; 2] = serde_json::from_value(
                            expected["base_frame"]["frame"]["click_client"].clone(),
                        )
                        .unwrap();
                        let pointer = egui::pos2(xy[0] as f32, xy[1] as f32);
                        if let Some(curve) = app.move_normal_curve() {
                            let parameter = view
                                .pick_edge_parameter(curve, pointer, rect, false)
                                .unwrap();
                            assert!(
                                app.accept_move_normal_curve_parameter(parameter),
                                "{label}: {:?}",
                                app.command_log
                            );
                        } else {
                            let location = view
                                .normal_surface_point(
                                    pointer,
                                    rect,
                                    &app.document,
                                    app.move_normal_surface().unwrap(),
                                )
                                .unwrap();
                            assert!(
                                app.accept_drafting_point(location),
                                "{label}: {:?}",
                                app.command_log
                            );
                        }
                    } else if input == "Target" {
                        let target = target.unwrap();
                        let recipe = &operation["mirror_target"];
                        if recipe["pick"] != "id" {
                            app.accept_component_face_hit(
                                target,
                                recipe["face"].as_u64().unwrap() as usize,
                                None,
                            );
                        } else {
                            enter(&mut app, &target.to_string());
                        }
                    } else {
                        enter(&mut app, input.as_str().unwrap());
                    }
                }
                if operation["finish"] != "Automatic" {
                    enter(&mut app, operation["finish"].as_str().unwrap());
                }
            }
            true => {
                let input = if matches!(invocation, Invocation::NormalRegistry) {
                    let inputs = operation["inputs"].as_array().unwrap();
                    let base = inputs
                        .iter()
                        .position(|input| input == "NormalTarget")
                        .unwrap()
                        + 1;
                    let options = inputs
                        .iter()
                        .filter_map(|input| {
                            input
                                .as_str()
                                .filter(|input| input.starts_with("IgnoreTrims="))
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!(
                        "Move Normal={} {} {} {}",
                        target.unwrap(),
                        inputs[base].as_str().unwrap().trim_start_matches('w'),
                        inputs[base + 1].as_str().unwrap().trim_start_matches('w'),
                        options
                    )
                } else {
                    std::iter::once(operation["command"].as_str().unwrap().to_owned())
                        .chain(operation["inputs"].as_array().unwrap().iter().map(|value| {
                            let value = value.as_str().unwrap();
                            if value == "Target" {
                                let id = target.unwrap();
                                let recipe = &operation["mirror_target"];
                                if recipe["pick"] != "id" {
                                    format!("{id} Face={}", recipe["face"])
                                } else {
                                    id.to_string()
                                }
                            } else {
                                value.trim_start_matches('w').to_owned()
                            }
                        }))
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                let context = viboceros_command::CommandContext {
                    construction_plane: app.viewports[app.active_viewport].construction_plane(),
                };
                let result = app
                    .commands
                    .execute_in_context(&mut app.document, &input, context);
                if let Err(error) = result {
                    assert_eq!(expected["succeeded"], false, "{label}: {error}");
                }
            }
        }
        assert!(
            app.active_command.is_none() && app.object_prompt.is_none(),
            "{label}: {:?}",
            app.command_log
        );
        compare(snapshot(&app, &sources), "after", &mut failures);
        if operation["sel_last"].as_bool().unwrap() {
            enter(&mut app, "SelLast");
            compare(snapshot(&app, &sources), "last", &mut failures);
        }
        if !expected["undo"].is_null() {
            enter(&mut app, "Undo");
            assert!(
                app.document.undo_label() == Some("Transform source setup"),
                "{label}: more than one transform Undo entry"
            );
            compare(snapshot(&app, &sources), "undo", &mut failures);
            enter(&mut app, "Redo");
            compare(snapshot(&app, &sources), "redo", &mut failures);
        } else if operation["undo_redo"] == true {
            assert!(
                app.document.undo_label() == Some("Transform source setup"),
                "{label}: canceled or identity command recorded history"
            );
        }
        if let Some(id) = target {
            assert_eq!(
                app.document.object(id),
                target_before.as_ref(),
                "{label}: plane target was edited"
            );
            assert!(
                !app.document.is_selected(id),
                "{label}: plane target became a transform source"
            );
        }
        commands = Some(app.commands);
    }
    assert!(failures.is_empty(), "native differences: {failures:?}");
}
