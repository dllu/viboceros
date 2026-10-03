use super::*;
use crate::viewport::object_preview::ObjectPreview;
use crate::viewport::preview_test_support::*;
use serde_json::Value;
use std::sync::Arc;
use viboceros_command::CommandRegistry;
use viboceros_document::ColorRgb;
use viboceros_geometry::PointMorph;

fn captures() -> (Value, Value) {
    let inputs = [
        include_str!("../../../tools/rhino_oracle/fixtures/bend_preview.json"),
        include_str!("../../../tools/rhino_oracle/fixtures/bend_preview_followup.json"),
    ];
    let outputs = [
        include_str!("../../../tools/rhino_oracle/observations/bend_preview.json"),
        include_str!("../../../tools/rhino_oracle/observations/bend_preview_followup.json"),
    ];
    let operations = inputs
        .into_iter()
        .flat_map(|s| {
            serde_json::from_str::<Value>(s).unwrap()["operations"]
                .as_array()
                .unwrap()
                .clone()
        })
        .collect::<Vec<_>>();
    let results = outputs
        .into_iter()
        .flat_map(|s| {
            serde_json::from_str::<Value>(s).unwrap()["results"]
                .as_array()
                .unwrap()
                .clone()
        })
        .collect::<Vec<_>>();
    (
        serde_json::json!({"operations":operations}),
        serde_json::json!({"results":results}),
    )
}
fn preview<'a>(
    ids: &'a [ObjectId],
    cache: &'a RefCell<BendPreviewCache>,
    op: &Value,
    through: Point3,
) -> BendPreview<'a> {
    BendPreview {
        sources: ids,
        cache,
        start: point(0., 0., op["spine_z"].as_f64().unwrap_or(0.)),
        end: point(0., 0., op["spine_z"].as_f64().unwrap_or(0.) + 10.),
        last_point: Some(through),
        options: BendOptions {
            copy: op["copy"].as_bool().unwrap(),
            rigid: op["rigid"].as_bool().unwrap(),
            limit_to_spine: op["limited"].as_bool().unwrap(),
            symmetric: op["symmetric"].as_bool().unwrap(),
            preserve_structure: op["preserve"].as_bool().unwrap(),
            non_attenuated: op["uniform"].as_bool().unwrap(),
            angle: Some(op["angle"].as_f64().unwrap()),
        },
    }
}

#[test]
fn native_bend_cubic_cages_surface_controls_and_rigid_placements_match_preview() {
    let (fixture, observed) = captures();
    assert_eq!(
        fixture["operations"].as_array().unwrap().len(),
        observed["results"].as_array().unwrap().len()
    );
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let label = op["id"].as_str().unwrap();
        assert_eq!(op["id"], row["id"]);
        let v = &row["value"];
        let (mut doc, ids) = setup(op, &v["before"]);
        let before = doc.objects().cloned().collect::<Vec<_>>();
        doc.clear_history().unwrap();
        let cache = RefCell::new(BendPreviewCache::default());
        let display = RefCell::new(DisplayCache::default());
        let preview = preview(&ids, &cache, op, p(&v["calibration"]["valid_point"]));
        let image = preview.resolve(None, &doc, &display).0.unwrap();
        let sdk = v["sdk_preview"].as_array().unwrap();
        if op["rigid"] == true {
            for (id, native) in ids.iter().zip(v["after"].as_array().unwrap()) {
                let object = &image.objects[id];
                assert!(Rc::ptr_eq(
                    &object.geometry,
                    &display
                        .borrow_mut()
                        .get(doc.object(*id).unwrap(), doc.tolerance())
                ));
                if let Geometry::Point(source) = doc.object(*id).unwrap().geometry() {
                    near(
                        object.transform.unwrap().transform_point(*source).unwrap(),
                        p(&native["geometry"]["points"][0]),
                        1e-7,
                        label,
                    );
                } else if let Geometry::Brep(source) = doc.object(*id).unwrap().geometry() {
                    let vertices = native["geometry"]["vertices"].as_array().unwrap();
                    assert_eq!(source.vertices().len(), vertices.len(), "{label}");
                    // Box builders use different topology orderings; require a
                    // bijection of the independently captured vertex positions.
                    let mut unmatched: Vec<_> = vertices.iter().map(p).collect();
                    for vertex in source.vertices() {
                        let actual = object
                            .transform
                            .unwrap()
                            .transform_point(vertex.point())
                            .unwrap();
                        let index = unmatched
                            .iter()
                            .position(|expected| actual.distance_to(*expected).unwrap() < 1e-7)
                            .unwrap_or_else(|| {
                                panic!("{label}: unmatched rigid vertex {actual:?}")
                            });
                        unmatched.swap_remove(index);
                    }
                }
            }
        } else if !sdk.is_empty() {
            for (i, id) in ids.iter().enumerate() {
                match &*image.objects[id].geometry.geometry {
                    Geometry::NurbsCurve(actual) => {
                        let native = &v["sdk_cubic_preview"][i];
                        let expected = curve(&native["definition"]);
                        assert_eq!(actual.degree(), expected.degree(), "{label}");
                        assert_eq!(
                            actual.control_points().len(),
                            expected.control_points().len(),
                            "{label}"
                        );
                        for (a, b) in actual
                            .control_points()
                            .iter()
                            .zip(expected.control_points())
                        {
                            near(a.point(), b.point(), 1e-11, label);
                            assert!((a.weight() - b.weight()).abs() < 1e-14, "{label}");
                        }
                        let d = actual.domain();
                        for (j, pnt) in native["samples"].as_array().unwrap().iter().enumerate() {
                            near(
                                actual
                                    .evaluate(d.start() + (d.end() - d.start()) * j as Real / 64.)
                                    .unwrap(),
                                p(pnt),
                                1e-11,
                                label,
                            );
                        }
                    }
                    Geometry::NurbsSurface(actual) => {
                        let expected = surface(&sdk[i]["surfaces"][0]);
                        assert_eq!(
                            [actual.degree_u(), actual.degree_v()],
                            [expected.degree_u(), expected.degree_v()],
                            "{label}"
                        );
                        assert_eq!(
                            actual.control_points().len(),
                            expected.control_points().len(),
                            "{label}"
                        );
                        for (a, b) in actual
                            .control_points()
                            .iter()
                            .zip(expected.control_points())
                        {
                            near(a.point(), b.point(), 1e-11, label);
                            assert!((a.weight() - b.weight()).abs() < 1e-14, "{label}");
                        }
                    }
                    Geometry::Brep(_) => {
                        let prepared = cache.borrow();
                        let crate::viewport::morph_preview::Cage::Brep(cage) =
                            prepared.sources[i].cage.as_ref().unwrap()
                        else {
                            panic!()
                        };
                        let morph = BendPointMorph::try_for_command(
                            preview.start,
                            preview.end,
                            preview.last_point.unwrap(),
                            None,
                            false,
                            op["symmetric"].as_bool().unwrap(),
                            doc.tolerance(),
                        )
                        .unwrap();
                        let patches = sdk[i]["surfaces"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(surface)
                            .collect::<Vec<_>>();
                        for c in cage.morphed_wires(&morph, false).unwrap() {
                            let d = c.domain();
                            let samples = (0..9)
                                .map(|j| {
                                    c.evaluate(d.start() + (d.end() - d.start()) * j as Real / 8.)
                                        .unwrap()
                                })
                                .collect::<Vec<_>>();
                            assert!(
                                patches.iter().any(|patch| (0..2).any(|axis| (0..9).any(
                                    |station| (0..2).any(|reverse| samples.iter().enumerate().all(
                                        |(j, a)| {
                                            let j = if reverse == 0 { j } else { 8 - j };
                                            let (u, v) = if axis == 0 {
                                                (j, station)
                                            } else {
                                                (station, j)
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
                                        }
                                    ))
                                ))),
                                "{label}: B-rep wire differs from native quick-preview patches"
                            );
                        }
                    }
                    Geometry::Mesh(_) => {
                        let morph = BendPointMorph::try_for_command(
                            preview.start,
                            preview.end,
                            preview.last_point.unwrap(),
                            None,
                            false,
                            false,
                            doc.tolerance(),
                        )
                        .unwrap();
                        let source = display
                            .borrow_mut()
                            .get(doc.object(*id).unwrap(), doc.tolerance());
                        for (a, b) in image.objects[id]
                            .geometry
                            .wires()
                            .iter()
                            .zip(source.wires())
                        {
                            near(a[0], morph.morph_point(b[0]).unwrap(), 1e-11, label);
                            near(a[1], morph.morph_point(b[1]).unwrap(), 1e-11, label);
                        }
                    }
                    Geometry::Point(actual) => {
                        near(*actual, p(&sdk[i]["points"][0]), 1e-11, label);
                        if op["finish"] == "Click" {
                            near(
                                *actual,
                                p(&v["after"][i]["geometry"]["points"][0]),
                                1e-11,
                                label,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            if mode != DisplayMode::Wireframe
                && matches!(op["shape"].as_str(), Some("Surface" | "Box"))
            {
                for id in &ids {
                    assert!(
                        image.objects_for_mode(mode)[id].geometry.wires().len()
                            < image.objects[id].geometry.wires().len(),
                        "{label}: shaded preview retained interior isocurves"
                    );
                }
            }
            let mut view = Viewport::new(ViewKind::Front);
            view.display_mode = mode;
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
            let original = view.object_scene(rect, &doc);
            let scene = view.object_scene_with_object_preview(
                rect,
                &doc,
                None,
                &[],
                Some(ObjectPreview::Deformed(image.objects_for_mode(mode))),
            );
            assert_eq!(
                scene.triangles.len(),
                original.triangles.len(),
                "{label}: preview added faces"
            );
            for (a, b) in scene.triangles.iter().zip(&original.triangles) {
                assert_eq!(a.color, b.color, "{label}: source face color");
            }
            assert_eq!(scene.overlay_line_start, original.lines.len());
            assert_eq!(scene.overlay_point_start, original.points.len());
            assert!(Arc::ptr_eq(
                &scene,
                &view.object_scene_with_object_preview(
                    rect,
                    &doc,
                    None,
                    &[],
                    Some(ObjectPreview::Deformed(image.objects_for_mode(mode)))
                )
            ));
        }
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
}

#[test]
fn bend_preview_cache_tracks_options_and_source_changes_without_model_edits() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(point(2., 1., 5.)))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    let cache = RefCell::new(BendPreviewCache::default());
    let display = RefCell::new(DisplayCache::default());
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let preview = BendPreview {
        sources: &[id],
        start: point(0., 0., 0.),
        end: point(0., 0., 10.),
        options: BendOptions::default(),
        last_point: None,
        cache: &cache,
    };
    assert!(preview.resolve(None, &doc, &display).0.is_none());
    let target = point(10., 0., 10.);
    let (image, update) = preview.resolve(Some(target), &doc, &display);
    let image = image.unwrap();
    assert_eq!(update, Some(Some(target)));
    let preview = BendPreview {
        last_point: Some(target),
        ..preview
    };
    for _ in 0..4 {
        assert!(Rc::ptr_eq(
            &image,
            &preview.resolve(None, &doc, &display).0.unwrap()
        ));
    }
    let copy = BendPreview {
        options: BendOptions {
            copy: true,
            ..preview.options
        },
        ..preview
    };
    assert!(Rc::ptr_eq(
        &image,
        &copy.resolve(None, &doc, &display).0.unwrap()
    ));
    doc.set_objects_color([id], Some(ColorRgb::new(10, 20, 30)))
        .unwrap();
    assert!(Rc::ptr_eq(
        &image,
        &preview.resolve(None, &doc, &display).0.unwrap()
    ));
    assert_eq!(doc.object(id).unwrap().geometry(), before[0].geometry());
    let angled = BendPreview {
        options: BendOptions {
            angle: Some(45.),
            ..preview.options
        },
        ..preview
    };
    assert!(!Rc::ptr_eq(
        &image,
        &angled.resolve(None, &doc, &display).0.unwrap()
    ));
    CommandRegistry::with_builtins()
        .execute(&mut doc, "Move 0,0,0 1,0,0")
        .unwrap();
    assert!(!Rc::ptr_eq(
        &image,
        &preview.resolve(None, &doc, &display).0.unwrap()
    ));
    doc.set_tolerance(Tolerance::try_new(2e-5, 1e-12, 1e-9).unwrap());
    let changed = preview.resolve(None, &doc, &display).0.unwrap();
    assert!(!Rc::ptr_eq(&image, &changed));
}

#[test]
fn bend_mouse_plane_click_filter_and_invalid_cursor_match_native_lifecycle() {
    use viboceros_drafting::{PointFilter, PointFilterSession};
    let (fixture, observed) = captures();
    for index in [0, 26, 31, 34] {
        let op = &fixture["operations"][index];
        let v = &observed["results"][index]["value"];
        let (mut doc, ids) = setup(op, &v["before"]);
        doc.clear_history().unwrap();
        let cache = RefCell::new(BendPreviewCache::default());
        let mut preview = preview(&ids, &cache, op, p(&v["calibration"]["valid_point"]));
        preview.last_point = None;
        let context = egui::Context::default();
        let mut view = crate::viewport::clip_tests::captured_view(&v["pending"]["camera"]);
        let input = ViewportInput {
            bend_preview: Some(preview),
            drafting: DraftingInput {
                active: true,
                anchor: Some(preview.end),
                ..Default::default()
            },
            ..Default::default()
        };
        frame(&context, &mut view, &doc, input, vec![]);
        let target = p(&v["calibration"]["valid_point"]);
        let pointer = view.project(target, view.last_rect.unwrap()).unwrap();
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
        near(
            output.picked_point.unwrap(),
            target,
            1e-5,
            "native mouse plane",
        );
        near(
            output.bend_preview.unwrap().unwrap(),
            target,
            1e-5,
            "preview and click use one point",
        );
        assert_eq!(output.picked_point, output.bend_preview.unwrap());
        let retained = BendPreview {
            last_point: Some(target),
            ..preview
        };
        let image = retained.resolve(None, &doc, &view.display_cache).0.unwrap();
        assert!(!image.guide.is_empty());
        for (events, filter) in [
            (vec![egui::Event::PointerGone], None),
            (
                vec![egui::Event::PointerMoved(pointer)],
                Some(PointFilterSession::new(
                    PointFilter::parse(".wx").unwrap(),
                    view.construction_plane(),
                )),
            ),
        ] {
            let output = frame(
                &context,
                &mut view,
                &doc,
                ViewportInput {
                    bend_preview: Some(retained),
                    point_filter: filter,
                    ..input
                },
                events,
            );
            assert_eq!(output.bend_preview, None);
            assert!(Rc::ptr_eq(
                &image,
                &retained.resolve(None, &doc, &view.display_cache).0.unwrap()
            ));
        }
        let invalid = retained.resolve(Some(retained.end), &doc, &view.display_cache);
        assert!(invalid.0.is_none());
        assert_eq!(invalid.1, Some(None));
        assert!(
            BendPreview {
                last_point: None,
                ..retained
            }
            .resolve(None, &doc, &view.display_cache)
            .0
            .is_none()
        );
        assert!(!doc.can_undo());
    }
}

#[test]
fn limited_preview_guides_have_a_finite_budget_for_tiny_through_circles() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(point(2., 1., 5.)))
        .unwrap();
    let cache = RefCell::new(BendPreviewCache::default());
    let display = RefCell::new(DisplayCache::default());
    let preview = BendPreview {
        sources: &[id],
        start: point(0., 0., 0.),
        end: point(0., 0., 10.),
        last_point: Some(point(1e-7, 0., 0.)),
        options: BendOptions {
            limit_to_spine: true,
            ..Default::default()
        },
        cache: &cache,
    };
    let image = preview.resolve(None, &doc, &display).0.unwrap();
    assert!(image.guide.len() <= 258);
    assert_eq!(doc.objects().count(), 1);
}
