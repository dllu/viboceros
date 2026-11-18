use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use serde::Deserialize;
use viboceros_document::{ColorRgb, LayerId, ObjectAttributes};
use viboceros_geometry::TriangleMesh;

#[derive(Deserialize)]
struct Case {
    name: String,
    view: String,
    preselect: bool,
    sources: Vec<MeshObject>,
    inputs: Vec<String>,
    result: Vec<MeshObject>,
}

#[derive(Deserialize)]
struct MeshObject {
    mesh: Mesh,
    name: String,
    layer: String,
}

#[derive(Deserialize)]
struct Mesh {
    vertices: Vec<[f64; 3]>,
    faces: Vec<Vec<u32>>,
}

fn top_app() -> VibocerosApp {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.smart_track = false;
    app
}

fn add_quad(app: &mut VibocerosApp, vertices: &[[f64; 3]], layer: LayerId, name: &str) -> ObjectId {
    let mesh = TriangleMesh::try_new_faces(
        vertices
            .iter()
            .copied()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![MeshFace::Quad([0, 1, 2, 3])],
        app.document.tolerance(),
    )
    .unwrap();
    app.document
        .add_geometry_with_attributes(
            Geometry::Mesh(mesh),
            ObjectAttributes::on_layer(layer).with_name(name),
        )
        .unwrap()
}

fn compare_meshes(app: &VibocerosApp, expected: &[MeshObject], name: &str) {
    let actual = app.document.objects().collect::<Vec<_>>();
    assert_eq!(
        actual.len(),
        expected.len(),
        "{name}: {:?}",
        app.command_log
    );
    for (object, row) in actual.iter().zip(expected) {
        let Geometry::Mesh(mesh) = object.geometry() else {
            panic!("{name}: expected mesh")
        };
        assert_eq!(object.attributes().name().unwrap_or(""), row.name, "{name}");
        assert_eq!(
            app.document
                .layer(object.attributes().layer_id())
                .unwrap()
                .name(),
            row.layer,
            "{name}"
        );
        assert_eq!(mesh.vertices().len(), row.mesh.vertices.len(), "{name}");
        assert_eq!(mesh.faces().len(), row.mesh.faces.len(), "{name}");
        for (actual, expected) in mesh.vertices().iter().zip(&row.mesh.vertices) {
            for (a, b) in actual.to_array().into_iter().zip(expected) {
                assert!((a - b).abs() <= 1e-7, "{name}: {a} differs from {b}");
            }
        }
        for (actual, expected) in mesh.faces().iter().zip(&row.mesh.faces) {
            assert_eq!(actual.indices(), expected.as_slice(), "{name}");
        }
    }
    assert_eq!(app.document.selected_object_count(), 0, "{name}");
}

#[test]
fn staged_mesh_storage_matches_headless_rhino_geometry_and_history() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("offset_mesh/rhino.json")).unwrap();
    for case in cases {
        let mut app = top_app();
        if case.view == "Front" {
            app.viewports[0] = Viewport::new(ViewKind::Front);
        }
        let source_layer = app
            .document
            .add_layer("Plane sources", ColorRgb::BLACK)
            .unwrap();
        let output_layer = app
            .document
            .add_layer("Plane output", ColorRgb::BLACK)
            .unwrap();
        app.document.set_current_layer(output_layer).unwrap();
        let sources = case
            .sources
            .iter()
            .map(|row| {
                add_quad(
                    &mut app,
                    &row.mesh.vertices,
                    if row.layer == "Plane sources" {
                        source_layer
                    } else {
                        output_layer
                    },
                    &row.name,
                )
            })
            .collect::<Vec<_>>();
        if case.preselect {
            app.document
                .select_objects_direct(sources.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        app.document.clear_history().unwrap();
        for input in &case.inputs {
            match input.as_str() {
                "@select" => app.apply_selection_click(SelectionClick {
                    object_id: Some(sources[0]),
                    mode: SelectionMode::Add,
                }),
                "@first" => enter(&mut app, &sources[0].to_string()),
                "Enter" if app.object_prompt.is_some() => enter(&mut app, ""),
                _ => enter(&mut app, input),
            }
        }
        assert!(
            app.command_line_idle(),
            "{}: {:?}",
            case.name,
            app.command_log
        );
        compare_meshes(&app, &case.result, &case.name);
        app.document.undo().unwrap();
        compare_meshes(&app, &case.sources, &case.name);
        assert!(!app.document.can_undo(), "{}", case.name);
        app.document.redo().unwrap();
        compare_meshes(&app, &case.result, &case.name);
    }
}

fn quad(app: &mut VibocerosApp) -> ObjectId {
    let layer = app.document.current_layer_id();
    add_quad(
        app,
        &[[0., 0., 0.], [8., 0., 0.], [8., 6., 0.], [0., 6., 0.]],
        layer,
        "mesh",
    )
}

fn preview(app: &VibocerosApp) -> &TriangleMesh {
    app.offset_mesh_session
        .as_ref()
        .unwrap()
        .preview()
        .unwrap()
        .meshes()
        .next()
        .unwrap()
}

#[test]
fn viewport_picks_and_previews_then_uses_left_or_right_click_distance() {
    for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
        let mut app = top_app();
        app.grid_snap = true;
        app.viewports[0].display_mode = DisplayMode::Shaded;
        let source = quad(&mut app);
        app.document.clear_history().unwrap();
        let context = egui::Context::default();
        viewport_frame(&mut app, &context, Vec::new());
        viewport_frame(&mut app, &context, Vec::new());
        app.viewports[0]
            .zoom_extents(&app.document, Default::default())
            .unwrap();

        enter(&mut app, "OffsetMesh");
        viewport_frame(&mut app, &context, Vec::new());
        let mesh = screen_point(&app, point(4., 1., 0.));
        viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(mesh)]);
        viewport_click(&mut app, &context, mesh);
        assert!(
            app.document.is_selected(source),
            "{mesh:?} {:?}",
            app.command_log
        );
        enter(&mut app, "");

        enter(&mut app, "Flip");
        viewport_frame(&mut app, &context, Vec::new());
        viewport_click(&mut app, &context, mesh);
        enter(&mut app, "Enter");
        assert!(preview(&app).vertices()[0].z() < 0.);

        enter(&mut app, "2");
        enter(&mut app, "SetBasePoint");
        enter(&mut app, "w1,2,0");
        viewport_frame(&mut app, &context, Vec::new());
        let end = screen_point(&app, point(4., 6., 0.));
        viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(end)]);
        let lines = viewport_frame(&mut app, &context, Vec::new());
        assert!(lines.len() >= 4, "mesh preview lines were not painted");
        assert!((preview(&app).vertices()[0].z() + 5.).abs() < 1e-5);
        assert_eq!(app.document.objects().len(), 1);
        assert!(!app.document.can_undo());

        for pressed in [true, false] {
            viewport_frame(
                &mut app,
                &context,
                vec![egui::Event::PointerButton {
                    pos: end,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert!(app.command_line_idle(), "{:?}", app.command_log);
        let Geometry::Mesh(mesh) = app.document.objects().last().unwrap().geometry() else {
            panic!("mesh result")
        };
        let expected = if button == egui::PointerButton::Primary {
            -5.
        } else {
            -2.
        };
        assert!((mesh.vertices()[0].z() - expected).abs() < 1e-5);
        assert!(app.document.object(source).is_some());
    }
}

#[test]
fn cancel_and_stale_sources_do_not_persist_settings_or_add_history() {
    let mut app = top_app();
    let source = quad(&mut app);
    app.document.clear_history().unwrap();
    enter(&mut app, "OffsetMesh");
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Add,
    });
    enter(&mut app, "");
    enter(&mut app, "DynamicPreview=No 6 Solid=Yes");
    enter(&mut app, "Flip");
    enter(&mut app, &source.to_string());
    app.cancel_current_prompt_or_selection();
    assert_eq!(preview(&app).vertices()[0].z(), 6.);
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());

    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "OffsetMesh");
    assert_eq!(preview(&app).vertices()[0].z(), 1.);
    app.document
        .set_object_names([(source, Some("updated".into()))])
        .unwrap();
    enter(&mut app, "Enter");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().len(), 1);
    assert!(
        app.command_log
            .iter()
            .any(|line| line.contains("OffsetMesh inputs or tolerance changed"))
    );
}

#[test]
fn perspective_mouse_uses_the_viewing_ray_to_preview_the_offset_axis() {
    let mut app = top_app();
    app.viewports[0] = Viewport::new(ViewKind::Perspective);
    app.grid_snap = false;
    app.ortho = false;
    app.planar = false;
    let source = quad(&mut app);
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "OffsetMesh SetBasePoint w1,2,0");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(&mut app, &context, Vec::new());
    let target = screen_point(&app, point(1., 2., 4.));
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
    let free_ray = app.viewports[0].pointer_ray().unwrap();
    let ray_point = free_ray
        .closest_point_on_axis(
            point(1., 2., 0.),
            app.viewports[0].construction_plane().z_axis(),
        )
        .unwrap();
    let displayed = preview(&app).vertices()[0].z();
    assert!(
        (ray_point.z() - 4.).abs() < 1e-3,
        "ray hit {}",
        ray_point.z()
    );
    assert!((displayed - 4.).abs() < 1e-3, "{displayed}");
    viewport_click(&mut app, &context, target);
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Mesh(mesh) = app.document.objects().last().unwrap().geometry() else {
        panic!("mesh result")
    };
    assert_eq!(mesh.vertices()[0].z(), displayed);
}
