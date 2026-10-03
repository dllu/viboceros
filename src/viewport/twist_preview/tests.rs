use super::super::object_preview::ObjectPreview;
use super::*;
use serde_json::Value;
use std::sync::Arc;
use viboceros_command::CommandRegistry;
use viboceros_document::ColorRgb;
use viboceros_geometry::{
    AffineTransform3, Brep, LineSegment, MeshFace, NurbsSurface, WeightedPoint3,
};

fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| WeightedPoint3::try_new(p(&c["point"]), c["weight"].as_f64().unwrap()).unwrap())
        .collect()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
fn near(a: Point3, b: Point3, epsilon: Real, label: &str) {
    assert!(
        a.distance_to(b).unwrap() < epsilon,
        "{label}: {a:?} != {b:?}"
    );
}
fn captures() -> (Value, Value) {
    (
        serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/twist_preview.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/twist_preview.json"
        ))
        .unwrap(),
    )
}
fn setup(op: &Value, before: &Value) -> (Document, Vec<ObjectId>) {
    let tol = Tolerance::try_new(1e-5, 1e-12, 1e-9).unwrap();
    let mut doc = Document::new(tol);
    let mut ids = Vec::new();
    for row in before.as_array().unwrap() {
        let g = &row["geometry"];
        if row["witness"] == true {
            doc.add_geometry(Geometry::Point(p(&g["points"][0])))
                .unwrap();
            continue;
        }
        let geometry = match op["shape"].as_str().unwrap() {
            "Points" => Geometry::Point(p(&g["points"][0])),
            "Line" => Geometry::Line(
                LineSegment::try_new(p(&g["samples"][0]), p(&g["samples"][64]), tol).unwrap(),
            ),
            "Curve" => Geometry::NurbsCurve(curve(&g["definition"])),
            "Surface" => Geometry::NurbsSurface(surface(&g["surfaces"][0])),
            "Box" => Geometry::Brep(
                Brep::try_box(
                    WorldPlane::Top.frame(),
                    [[1., 3.], [-1., 1.], [0., 10.]],
                    tol,
                )
                .unwrap(),
            ),
            "Mesh" => {
                let mut colors: Vec<[u8; 4]> = serde_json::from_value(g["colors"].clone()).unwrap();
                for c in &mut colors {
                    c[3] = 255 - c[3];
                }
                Geometry::Mesh(
                    TriangleMesh::try_new_faces(
                        g["points"].as_array().unwrap().iter().map(p).collect(),
                        vec![MeshFace::Quad([0, 1, 2, 3])],
                        tol,
                    )
                    .unwrap()
                    .try_with_vertex_colors(Some(colors))
                    .unwrap(),
                )
            }
            _ => unreachable!(),
        };
        let id = doc.add_geometry(geometry).unwrap();
        ids.push(id);
    }
    doc.set_objects_color(ids.iter().copied(), Some(ColorRgb::new(200, 80, 60)))
        .unwrap();
    if op["rigid"].as_bool().unwrap() && op["shape"] == "Points" {
        doc.add_group(None, ids.iter().copied()).unwrap();
    }
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    (doc, ids)
}
fn preview<'a>(
    ids: &'a [ObjectId],
    cache: &'a RefCell<TwistPreviewCache>,
    angle: Real,
    op: &Value,
) -> TwistPreview<'a> {
    TwistPreview {
        sources: ids,
        cache,
        start: point(0., 0., 0.),
        end: point(0., 0., 10.),
        reference: point(5., 0., 0.),
        last_angle: Some(angle),
        options: TwistOptions {
            copy: op["copy"].as_bool().unwrap(),
            rigid: op["rigid"].as_bool().unwrap(),
            infinite: op["infinite"].as_bool().unwrap(),
            preserve_structure: op["preserve"].as_bool().unwrap(),
        },
    }
}

#[test]
fn native_twist_cubic_cages_rigid_placements_and_overlay_styles() {
    let (fixture, observed) = captures();
    let mut checked = 0;
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let label = op["id"].as_str().unwrap();
        let v = &row["value"];
        if op["phase"] == "Reference" {
            continue;
        }
        let (mut doc, ids) = setup(op, &v["before"]);
        if op["phase"] == "Repeat" {
            let g = &v["pending"]["objects"][1]["geometry"];
            doc.add_geometry(Geometry::NurbsCurve(curve(&g["definition"])))
                .unwrap();
        }
        let before = doc.objects().cloned().collect::<Vec<_>>();
        doc.clear_history().unwrap();
        let cache = RefCell::new(TwistPreviewCache::default());
        let display = RefCell::new(DisplayCache::default());
        let angle = if op["phase"] == "Wrap" { 450. } else { 90. };
        let preview = preview(&ids, &cache, angle, op);
        let image = preview.resolve(None, &doc, &display).0.unwrap();
        if op["rigid"].as_bool().unwrap() {
            for (id, record) in ids.iter().zip(v["after"].as_array().unwrap()) {
                let actual = &image.objects[id];
                assert!(Rc::ptr_eq(
                    &actual.geometry,
                    &display
                        .borrow_mut()
                        .get(doc.object(*id).unwrap(), doc.tolerance())
                ));
                let source = doc.object(*id).unwrap().geometry();
                match source {
                    Geometry::Point(pt) => near(
                        actual.transform.unwrap().transform_point(*pt).unwrap(),
                        p(&record["geometry"]["points"][0]),
                        1e-7,
                        label,
                    ),
                    Geometry::Line(line) => {
                        for (n, t) in [0., 0.5, 1.].into_iter().enumerate() {
                            near(
                                actual
                                    .transform
                                    .unwrap()
                                    .transform_point(line.point_at(t).unwrap())
                                    .unwrap(),
                                p(&record["geometry"]["samples"][n * 32]),
                                1e-7,
                                label,
                            );
                        }
                    }
                    _ => unreachable!(),
                }
            }
        } else {
            let actual = &image.objects[&ids[0]].geometry;
            match &*actual.geometry {
                Geometry::NurbsCurve(c) => {
                    let native = &v["sdk_cubic_preview"][0];
                    let expected = curve(&native["definition"]);
                    assert_eq!(c.degree(), expected.degree());
                    assert_eq!(c.control_points().len(), expected.control_points().len());
                    for (a, b) in c.control_points().iter().zip(expected.control_points()) {
                        near(a.point(), b.point(), 1e-11, label);
                        assert!((a.weight() - b.weight()).abs() < 1e-14);
                    }
                    let domain = c.domain();
                    for (i, q) in native["samples"].as_array().unwrap().iter().enumerate() {
                        near(
                            c.evaluate(
                                domain.start() + (domain.end() - domain.start()) * i as Real / 64.,
                            )
                            .unwrap(),
                            p(q),
                            1e-11,
                            label,
                        );
                    }
                }
                Geometry::NurbsSurface(s) => {
                    let expected = surface(&v["sdk_preview"][0]["surfaces"][0]);
                    assert_eq!(
                        [s.degree_u(), s.degree_v()],
                        [expected.degree_u(), expected.degree_v()]
                    );
                    assert_eq!(s.control_points().len(), expected.control_points().len());
                    for (a, b) in s.control_points().iter().zip(expected.control_points()) {
                        near(a.point(), b.point(), 1e-11, label);
                        assert!((a.weight() - b.weight()).abs() < 1e-14);
                    }
                }
                Geometry::Brep(_) => {
                    let prepared = cache.borrow();
                    let Cage::Brep(cage) = prepared.sources[0].cage.as_ref().unwrap() else {
                        panic!()
                    };
                    let morph = TwistPointMorph::try_new(
                        preview.start,
                        preview.end,
                        angle.to_radians(),
                        false,
                        doc.tolerance(),
                    )
                    .unwrap();
                    let wires = cage.morphed_wires(&morph, false).unwrap();
                    assert!(wires.len() >= 12);
                    // Native patches can have different UV orientation. Each box wire
                    // must match a sampled row/column of one independently recorded quick-preview patch.
                    let patches = v["sdk_preview"][0]["surfaces"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(surface)
                        .collect::<Vec<_>>();
                    for c in wires {
                        let d = c.domain();
                        let samples = (0..9)
                            .map(|i| {
                                c.evaluate(d.start() + (d.end() - d.start()) * i as Real / 8.)
                                    .unwrap()
                            })
                            .collect::<Vec<_>>();
                        let matches = patches.iter().any(|patch| {
                            (0..2).any(|axis| {
                                (0..9).any(|station| {
                                    (0..2).any(|reverse| {
                                        samples.iter().enumerate().all(|(i, a)| {
                                            let i = if reverse == 0 { i } else { 8 - i };
                                            let (u, v) = if axis == 0 {
                                                (i, station)
                                            } else {
                                                (station, i)
                                            };
                                            let du = patch.domain_u();
                                            let dv = patch.domain_v();
                                            let b = patch
                                                .evaluate(
                                                    du.start()
                                                        + (du.end() - du.start()) * u as Real / 8.,
                                                    dv.start()
                                                        + (dv.end() - dv.start()) * v as Real / 8.,
                                                )
                                                .unwrap();
                                            a.distance_to(b).unwrap() < 1e-11
                                        })
                                    })
                                })
                            })
                        });
                        assert!(
                            matches,
                            "{label}: B-rep wire differs from native quick-preview patches"
                        );
                    }
                }
                Geometry::Mesh(_) => {
                    let morph = TwistPointMorph::try_new(
                        preview.start,
                        preview.end,
                        angle.to_radians(),
                        false,
                        doc.tolerance(),
                    )
                    .unwrap();
                    let source = display
                        .borrow_mut()
                        .get(doc.object(ids[0]).unwrap(), doc.tolerance());
                    for (a, b) in actual.wires().iter().zip(source.wires()) {
                        near(a[0], morph.morph_point(b[0]).unwrap(), 1e-11, label);
                        near(a[1], morph.morph_point(b[1]).unwrap(), 1e-11, label);
                    }
                }
                _ => unreachable!(),
            }
        }
        let mut view = super::super::clip_tests::captured_view(&v["pending"]["camera"]);
        view.display_mode = match op["display_mode"].as_str().unwrap() {
            "Shaded" => DisplayMode::Shaded,
            "Ghosted" => DisplayMode::Ghosted,
            _ => DisplayMode::Wireframe,
        };
        let rect = view.last_rect.unwrap();
        let original = view.object_scene(rect, &doc);
        let scene = view.object_scene_with_object_preview(
            rect,
            &doc,
            None,
            &[],
            Some(ObjectPreview::Deformed(&image.objects)),
        );
        assert_eq!(
            scene.triangles.len(),
            original.triangles.len(),
            "{label}: preview adds faces"
        );
        for (a, b) in scene.triangles.iter().zip(&original.triangles) {
            assert_eq!(a.color, b.color, "{label}: source face colors changed");
        }
        assert_eq!(scene.overlay_line_start, original.lines.len());
        assert_eq!(scene.overlay_point_start, original.points.len());
        let expected = [200. / 255., 80. / 255., 60. / 255., 1.];
        for c in scene.lines[scene.overlay_line_start..]
            .iter()
            .map(|l| l.color)
            .chain(
                scene.points[scene.overlay_point_start..]
                    .iter()
                    .map(|p| p.color),
            )
        {
            assert!(
                c.iter().zip(expected).all(|(a, b)| (*a - b).abs() < 1e-6),
                "{label}: target color {c:?}"
            );
        }
        assert!(Arc::ptr_eq(
            &scene,
            &view.object_scene_with_object_preview(
                rect,
                &doc,
                None,
                &[],
                Some(ObjectPreview::Deformed(&image.objects))
            )
        ));
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        checked += 1;
    }
    assert_eq!(checked, 30);
}

#[test]
fn twist_cache_shares_across_views_retains_invalid_angles_and_invalidates_source_changes() {
    let (fixture, observed) = captures();
    let op = &fixture["operations"][0];
    let (mut doc, ids) = setup(op, &observed["results"][0]["value"]["before"]);
    let cache = RefCell::new(TwistPreviewCache::default());
    let display = RefCell::new(DisplayCache::default());
    let image = preview(&ids, &cache, 90., op)
        .resolve(None, &doc, &display)
        .0
        .unwrap();
    for _ in 0..4 {
        let (same, update) =
            preview(&ids, &cache, 90., op).resolve(Some(point(0., 0., 3.)), &doc, &display);
        assert!(Rc::ptr_eq(&image, &same.unwrap()));
        assert_eq!(update, None);
    }
    let mut other_options = op.clone();
    other_options["preserve"] = Value::Bool(true);
    other_options["copy"] = Value::Bool(true);
    assert!(Rc::ptr_eq(
        &image,
        &preview(&ids, &cache, 90., &other_options)
            .resolve(None, &doc, &display)
            .0
            .unwrap()
    ));
    doc.set_objects_color(ids.iter().copied(), Some(ColorRgb::new(10, 20, 30)))
        .unwrap();
    assert!(Rc::ptr_eq(
        &image,
        &preview(&ids, &cache, 90., op)
            .resolve(None, &doc, &display)
            .0
            .unwrap()
    ));
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    let changed = preview(&ids, &cache, 90., op)
        .resolve(None, &doc, &display)
        .0
        .unwrap();
    assert!(!Rc::ptr_eq(&image, &changed));
    doc.set_tolerance(Tolerance::try_new(2e-5, 1e-12, 1e-9).unwrap());
    assert!(!Rc::ptr_eq(
        &changed,
        &preview(&ids, &cache, 90., op)
            .resolve(None, &doc, &display)
            .0
            .unwrap()
    ));
    let mut previous = 0.;
    for degrees in (0..=450).step_by(10) {
        let r = (degrees as Real).to_radians();
        let (image, update) = preview(&ids, &cache, previous, op).resolve(
            Some(point(5. * r.cos(), 5. * r.sin(), 0.)),
            &doc,
            &display,
        );
        assert!(image.is_some());
        previous = update.unwrap().unwrap();
        assert!((previous - degrees as Real).abs() < 1e-10);
    }
    assert_eq!(previous, 450.);
}

fn frame(
    context: &egui::Context,
    view: &mut Viewport,
    doc: &Document,
    input: ViewportInput<'_>,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut output = ViewportOutput::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                events,
                ..Default::default()
            },
            |ui| output = view.show(ui, doc, input, &[], 0, true),
        )
        .drop_without_applying_deltas();
    output
}
#[test]
fn twist_viewport_mouse_click_filters_and_leaving_the_view_keep_one_preview_angle() {
    use viboceros_drafting::{PointFilter, PointFilterSession};
    let (fixture, observed) = captures();
    let op = &fixture["operations"][0];
    let (doc, ids) = setup(op, &observed["results"][0]["value"]["before"]);
    let cache = RefCell::new(TwistPreviewCache::default());
    let preview = preview(&ids, &cache, 360., op);
    let context = egui::Context::default();
    let mut view = Viewport::new(ViewKind::Top);
    let input = ViewportInput {
        twist_preview: Some(preview),
        drafting: DraftingInput {
            active: true,
            ..Default::default()
        },
        angle_plane: Some(WorldPlane::Top.frame()),
        ..Default::default()
    };
    frame(&context, &mut view, &doc, input, vec![]);
    let pointer = view
        .project(point(0., 5., 0.), view.last_rect.unwrap())
        .unwrap();
    let output = frame(
        &context,
        &mut view,
        &doc,
        input,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ],
    );
    assert!(
        output
            .picked_point
            .unwrap()
            .distance_to(point(0., 5., 0.))
            .unwrap()
            < 1e-10
    );
    assert_eq!(output.twist_preview, Some(Some(450.)));
    let retained = TwistPreview {
        last_angle: Some(450.),
        ..preview
    };
    let image = retained.resolve(None, &doc, &view.display_cache).0.unwrap();
    for (events, filter) in [
        (vec![egui::Event::PointerGone], None),
        (
            vec![egui::Event::PointerMoved(pointer)],
            Some(PointFilterSession::new(
                PointFilter::parse(".wx").unwrap(),
                WorldPlane::Top.frame(),
            )),
        ),
    ] {
        let output = frame(
            &context,
            &mut view,
            &doc,
            ViewportInput {
                twist_preview: Some(retained),
                point_filter: filter,
                ..input
            },
            events,
        );
        assert_eq!(output.twist_preview, None);
        assert!(Rc::ptr_eq(
            &image,
            &retained.resolve(None, &doc, &view.display_cache).0.unwrap()
        ));
    }
    assert!(
        view.refresh_clipping_with_preview(
            &doc,
            view.last_rect.unwrap(),
            Some(ObjectPreview::Deformed(&image.objects))
        )
        .is_ok()
    );
    let before = image.objects[&ids[0]].geometry.wires().to_vec();
    view.orbit_yaw += 0.2;
    view.object_scene_with_object_preview(
        view.last_rect.unwrap(),
        &doc,
        None,
        &[],
        Some(ObjectPreview::Deformed(&image.objects)),
    );
    assert_eq!(image.objects[&ids[0]].geometry.wires(), before);
}

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn twist_gpu_overlay_is_visible_over_opaque_faces_and_obeys_world_clip_planes() {
    use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};
    use eframe::wgpu;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut renderer = OffscreenRenderer::new(format);
        let mut view = Viewport::new(ViewKind::Top);
        view.display_mode = DisplayMode::Shaded;
        view.perspective_camera_distance = 10.;
        view.frustum_near = 1.;
        view.frustum_far = 20.;
        let mut doc = Document::default();
        let mesh = TriangleMesh::try_new(
            vec![
                point(-3., -3., 0.),
                point(3., -3., 0.),
                point(3., 3., 0.),
                point(-3., 3., 0.),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            doc.tolerance(),
        )
        .unwrap();
        let face = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
        doc.set_objects_color([face], Some(ColorRgb::new(20, 60, 200)))
            .unwrap();
        let id = doc
            .add_geometry(Geometry::Line(
                LineSegment::try_new(point(-1., 0., -1.), point(1., 0., -1.), doc.tolerance())
                    .unwrap(),
            ))
            .unwrap();
        let point_id = doc
            .add_geometry(Geometry::Point(point(0., 1., -1.)))
            .unwrap();
        doc.set_objects_color([id, point_id], Some(ColorRgb::new(200, 80, 60)))
            .unwrap();
        let original = view.object_scene(rect, &doc);
        let hidden = renderer.render(&original);
        let mut objects = BTreeMap::new();
        for id in [id, point_id] {
            objects.insert(
                id,
                PreviewObject {
                    geometry: view
                        .display_cache
                        .borrow_mut()
                        .get(doc.object(id).unwrap(), doc.tolerance()),
                    transform: None,
                },
            );
        }
        let shown = renderer.render(&view.object_scene_with_object_preview(
            rect,
            &doc,
            None,
            &[],
            Some(ObjectPreview::Deformed(&objects)),
        ));
        let line = view.project(point(0., 0., -1.), rect).unwrap();
        let dot = view.project(point(0., 1., -1.), rect).unwrap();
        for pos in [line, dot] {
            let ix = pos.y.round() as usize * SIZE as usize + pos.x.round() as usize;
            assert!(hidden[ix][2] > hidden[ix][0], "source behind opaque face");
            assert!(
                shown[ix][0] > shown[ix][2] + 50,
                "overlay {format:?}: {:?}",
                shown[ix]
            );
        }
        let translate = AffineTransform3::from_translation(
            viboceros_geometry::Vector3::try_new(0., 0., -30.).unwrap(),
        );
        for o in objects.values_mut() {
            o.transform = Some(translate);
        }
        let clipped = renderer.render(&view.object_scene_with_object_preview(
            rect,
            &doc,
            None,
            &[],
            Some(ObjectPreview::Deformed(&objects)),
        ));
        assert_eq!(hidden, clipped, "overlay escaped world clipping {format:?}");
    }
}

#[test]
fn supplemental_native_brep_edges_and_single_face_preservation_match_cached_wires() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/twist_preview_edges.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/twist_preview_edges.json"
    ))
    .unwrap();
    let op = &fixture["operations"][0];
    let v = &observed["results"][0]["value"];
    let (doc, ids) = setup(op, &v["before"]);
    let cache = RefCell::new(TwistPreviewCache::default());
    let display = RefCell::new(DisplayCache::default());
    let input = preview(&ids, &cache, 90., op);
    let initial_image = input.resolve(None, &doc, &display).0.unwrap();
    let prepared = cache.borrow();
    let Cage::Brep(cage) = prepared.sources[0].cage.as_ref().unwrap() else {
        panic!()
    };
    let morph = TwistPointMorph::try_new(
        input.start,
        input.end,
        90_f64.to_radians(),
        false,
        doc.tolerance(),
    )
    .unwrap();
    let wires = cage.morphed_wires(&morph, false).unwrap();
    let expected = v["sdk_preview"][0]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| curve(&g["definition"]))
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 12);
    let mut matched = std::collections::BTreeSet::new();
    for c in wires.iter().take(12) {
        let d = c.domain();
        let found = expected
            .iter()
            .enumerate()
            .find(|(index, expected)| {
                !matched.contains(index)
                    && (0..2).any(|reverse| {
                        (0..=8).all(|i| {
                            let t = i as Real / 8.;
                            let q = if reverse == 0 { t } else { 1. - t };
                            let e = expected.domain();
                            c.evaluate(d.start() + (d.end() - d.start()) * t)
                                .unwrap()
                                .distance_to(
                                    expected
                                        .evaluate(e.start() + (e.end() - e.start()) * q)
                                        .unwrap(),
                                )
                                .unwrap()
                                < 1e-11
                        })
                    })
            })
            .unwrap()
            .0;
        matched.insert(found);
    }
    assert_eq!(matched.len(), 12);
    let (fixture, observed) = captures();
    // A preserved surface in the same batch must not change a multi-face box's policy.
    let mut mixed = doc.clone();
    let surface_id = mixed
        .add_geometry(Geometry::NurbsSurface(surface(
            &observed["results"][5]["value"]["before"][0]["geometry"]["surfaces"][0],
        )))
        .unwrap();
    let mixed_ids = [ids[0], surface_id];
    let mixed_cache = RefCell::new(TwistPreviewCache::default());
    let mixed_display = RefCell::new(DisplayCache::default());
    let mut mixed_op = op.clone();
    mixed_op["preserve"] = Value::Bool(true);
    let mixed_image = preview(&mixed_ids, &mixed_cache, 90., &mixed_op)
        .resolve(None, &mixed, &mixed_display)
        .0
        .unwrap();
    assert_eq!(
        initial_image.objects[&ids[0]].geometry.wires(),
        mixed_image.objects[&ids[0]].geometry.wires()
    );
    for index in [1, 5] {
        let op = &fixture["operations"][index];
        let v = &observed["results"][index]["value"];
        let (mut doc, ids) = setup(op, &v["before"]);
        let Geometry::NurbsSurface(s) = doc.object(ids[0]).unwrap().geometry() else {
            panic!()
        };
        let brep = Brep::try_surface_face(s.clone(), doc.tolerance()).unwrap();
        doc.replace_object_geometries([(ids[0], Geometry::Brep(brep))])
            .unwrap();
        let cache = RefCell::new(TwistPreviewCache::default());
        let display = RefCell::new(DisplayCache::default());
        let input = preview(&ids, &cache, 90., op);
        let image = input.resolve(None, &doc, &display).0.unwrap();
        let original_cache = cache.borrow();
        let Cage::Brep(cage) = original_cache.sources[0].cage.as_ref().unwrap() else {
            panic!()
        };
        let wires = cage
            .morphed_wires(&morph, op["preserve"].as_bool().unwrap())
            .unwrap();
        let surface = surface(&v["sdk_preview"][0]["surfaces"][0]);
        let du = surface.domain_u();
        let dv = surface.domain_v();
        for c in wires {
            let d = c.domain();
            assert!(
                [0., 0.5, 1.]
                    .into_iter()
                    .any(
                        |fixed| (0..2).any(|axis| (0..2).any(|reverse| (0..=8).all(|i| {
                            let t = i as Real / 8.;
                            let q = if reverse == 0 { t } else { 1. - t };
                            let (u, v) = if axis == 0 { (q, fixed) } else { (fixed, q) };
                            c.evaluate(d.start() + (d.end() - d.start()) * t)
                                .unwrap()
                                .distance_to(
                                    surface
                                        .evaluate(
                                            du.start() + (du.end() - du.start()) * u,
                                            dv.start() + (dv.end() - dv.start()) * v,
                                        )
                                        .unwrap(),
                                )
                                .unwrap()
                                < 1e-11
                        })))
                    )
            );
        }
        drop(original_cache);
        let mut flipped = op.clone();
        flipped["preserve"] = Value::Bool(!op["preserve"].as_bool().unwrap());
        let changed = preview(&ids, &cache, 90., &flipped)
            .resolve(None, &doc, &display)
            .0
            .unwrap();
        assert!(!Rc::ptr_eq(&image, &changed));
    }
}
