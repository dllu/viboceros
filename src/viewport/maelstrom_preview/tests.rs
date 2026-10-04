use super::*;
use crate::viewport::object_preview::ObjectPreview;
use crate::viewport::preview_test_support::*;
use serde_json::Value;
use std::sync::Arc;
use viboceros_geometry::PointMorph;

pub(crate) fn captures() -> (Value, Value) {
    (
        serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/maelstrom_preview.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/maelstrom_preview.json"
        ))
        .unwrap(),
    )
}
fn radius(v: &Value) -> MaelstromRadius {
    if v.is_array() {
        MaelstromRadius::Point(p(v))
    } else {
        MaelstromRadius::Number(v.as_f64().unwrap())
    }
}
fn preview<'a>(
    ids: &'a [ObjectId],
    cache: &'a RefCell<MaelstromPreviewCache>,
    op: &Value,
    v: &Value,
) -> MaelstromPreview<'a> {
    let cplane = if op["plane"] == "Tilt" {
        Frame3::try_from_normal(
            point(1., 2., 3.),
            Vector3::try_new(1., 2., 3.).unwrap(),
            Tolerance::default(),
        )
        .unwrap()
    } else {
        WorldPlane::Top.frame()
    };
    MaelstromPreview {
        circle_getter: None,
        sources: ids,
        cache,
        center: cplane.origin(),
        initial: (op["phase"] != "First").then(|| radius(&op["radius0"])),
        target: (!matches!(op["phase"].as_str().unwrap(), "First" | "Second"))
            .then(|| radius(&op["radius1"])),
        options: MaelstromOptions {
            copy: op["copy"].as_bool().unwrap(),
            rigid: op["rigid"].as_bool().unwrap(),
        },
        cplane,
        last: Some(MaelstromCursor {
            point: p(&v["calibration"]["valid_point"]),
            angle: Some(v["calibration"]["degrees"].as_f64().unwrap()),
        }),
    }
}
fn morph(preview: MaelstromPreview<'_>) -> MaelstromPointMorph {
    point_morph(
        preview.center,
        preview.initial.unwrap(),
        preview.target.unwrap(),
        preview.last.unwrap().angle.unwrap(),
        viboceros_command::CommandContext {
            construction_plane: preview.cplane,
        },
    )
    .unwrap()
}
#[test]
fn native_maelstrom_cubic_cages_surface_controls_and_rigid_placements_match_preview() {
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
        let cache = RefCell::new(MaelstromPreviewCache::default());
        let display = RefCell::new(DisplayCache::default());
        let preview = preview(&ids, &cache, op, v);
        if preview.initial.is_none() || preview.target.is_none() || op["cursor"] == "Degenerate" {
            let cursor = (op["cursor"] == "Degenerate").then_some(preview.center);
            assert!(
                preview.resolve(cursor, &doc, &display).0.is_none(),
                "{label}"
            );
            continue;
        }
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
                        let morph = morph(preview);
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
                        let morph = morph(preview);
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
                            for actual in a {
                                assert!(
                                    sdk[i]["points"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .any(|v| actual.distance_to(p(v)).unwrap() < 1e-6),
                                    "{label}: native mesh vertex"
                                );
                            }
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
            for id in &ids {
                assert_eq!(
                    image.objects_for_mode(mode)[id].geometry.wires(),
                    image.objects[id].geometry.wires(),
                    "{label}: Maelstrom keeps preview isocurves in every mode"
                );
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
fn maelstrom_preview_reuses_cages_tracks_turns_and_hides_at_axis() {
    use viboceros_command::CommandRegistry;
    let (fixture, observed) = captures();
    let op = &fixture["operations"][24];
    let v = &observed["results"][24]["value"];
    let (mut doc, ids) = setup(op, &v["before"]);
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let cache = RefCell::new(MaelstromPreviewCache::default());
    let display = RefCell::new(DisplayCache::default());
    let mut preview = preview(&ids, &cache, op, v);
    preview.last = None;
    assert!(preview.resolve(None, &doc, &display).0.is_none());
    let frame = preview.frame().unwrap();
    for i in 0..=80 {
        let angle = 450. * i as Real / 80.;
        let a = angle.to_radians();
        let cursor = frame.point_at([5. * a.cos(), 5. * a.sin(), 0.]).unwrap();
        let (image, update) = preview.resolve(Some(cursor), &doc, &display);
        assert!(image.is_some());
        preview.last = update;
        assert!((preview.last.unwrap().angle.unwrap() - angle).abs() < 1e-10);
    }
    let image = preview.resolve(None, &doc, &display).0.unwrap();
    let cage_controls = || {
        let c = cache.borrow();
        let super::super::morph_preview::Cage::Curve(curve) = c.sources[0].cage.as_ref().unwrap()
        else {
            panic!()
        };
        curve.control_points().as_ptr()
    };
    let prepared = cage_controls();
    assert!(Rc::ptr_eq(
        &image,
        &preview.resolve(None, &doc, &display).0.unwrap()
    ));
    let copy = MaelstromPreview {
        options: MaelstromOptions {
            copy: true,
            ..preview.options
        },
        ..preview
    };
    assert!(Rc::ptr_eq(
        &image,
        &copy.resolve(None, &doc, &display).0.unwrap()
    ));
    let (changed, update) =
        preview.resolve(Some(frame.point_at([5., 0., 0.]).unwrap()), &doc, &display);
    assert!(!Rc::ptr_eq(&image, &changed.unwrap()));
    assert_eq!(prepared, cage_controls());
    preview.last = update;
    let invalid = preview.resolve(Some(preview.center), &doc, &display);
    assert!(invalid.0.is_none());
    assert_eq!(invalid.1.unwrap().angle, Some(360.));
    preview.last = invalid.1;
    assert!(preview.resolve(None, &doc, &display).0.is_none());
    // Pointer departure and incomplete coordinate filters retain the last state.
    preview.last = Some(MaelstromCursor {
        point: p(&v["calibration"]["valid_point"]),
        angle: Some(450.),
    });
    assert!(preview.resolve(None, &doc, &display).0.is_some());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
    CommandRegistry::with_builtins()
        .execute(&mut doc, "Move 0,0,0 1,0,0")
        .unwrap();
    let changed = preview.resolve(None, &doc, &display).0.unwrap();
    assert!(!Rc::ptr_eq(&image, &changed));
}

#[test]
fn maelstrom_mouse_click_plane_filter_and_radius_guides_match_native_frames() {
    use viboceros_drafting::{PointFilter, PointFilterSession};
    let (fixture, observed) = captures();
    for index in [1, 15, 16, 20, 21, 22, 23, 24, 30] {
        let op = &fixture["operations"][index];
        let v = &observed["results"][index]["value"];
        let (mut doc, ids) = setup(op, &v["before"]);
        doc.clear_history().unwrap();
        let cache = RefCell::new(MaelstromPreviewCache::default());
        let mut preview = preview(&ids, &cache, op, v);
        let expected_degrees = preview.last.unwrap().angle;
        // Simulate the preceding turn without relying on frame timing.
        preview.last = if op["phase"] == "Wrap" {
            Some(MaelstromCursor {
                point: preview.center,
                angle: Some(440.),
            })
        } else {
            None
        };
        let context = egui::Context::default();
        let mut view = crate::viewport::clip_tests::captured_view(&v["pending"]["camera"]);
        let input = ViewportInput {
            maelstrom_preview: Some(preview),
            angle_plane: preview.initial.and_then(|_| preview.frame()),
            drafting: DraftingInput {
                active: true,
                anchor: Some(preview.center),
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
            op["id"].as_str().unwrap(),
        );
        let update = output.maelstrom_preview.unwrap();
        assert_eq!(output.picked_point, Some(update.point));
        if preview.target.is_some() {
            assert!((update.angle.unwrap() - expected_degrees.unwrap()).abs() < 1e-4);
        } else {
            assert_eq!(update.angle, None);
        }
        let retained = MaelstromPreview {
            last: Some(update),
            ..preview
        };
        assert_eq!(
            retained.circles(None).len(),
            if preview.initial.is_none() { 1 } else { 2 }
        );
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
                    maelstrom_preview: Some(retained),
                    point_filter: filter,
                    ..input
                },
                events,
            );
            assert_eq!(output.maelstrom_preview, None);
        }
        assert!(!doc.can_undo());
    }
}

#[test]
fn pending_construction_circles_match_native_definitions_without_model_edits() {
    use viboceros_command::circle_input::{CircleInput, CircleSizeMode};
    let mut checked = 0;
    for (fixture, observed) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_circle.json"),
            include_str!("../../../tools/rhino_oracle/observations/maelstrom_circle.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_circle_point.json"),
            include_str!("../../../tools/rhino_oracle/observations/maelstrom_circle_point.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_circle_angle.json"),
            include_str!("../../../tools/rhino_oracle/observations/maelstrom_circle_angle.json"),
        ),
    ] {
        let fixture: Value = serde_json::from_str(fixture).unwrap();
        let observed: Value = serde_json::from_str(observed).unwrap();
        let mut diameter = false;
        for (op, row) in fixture["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            let v = &row["value"];
            let plane = Frame3::try_from_normal(
                p(&op["origin"]),
                viboceros_geometry::Vector3::try_from(p(&op["normal"]).to_array()).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            let mut getter = CircleInput::new(plane).with_size_mode(if diameter {
                CircleSizeMode::Diameter
            } else {
                CircleSizeMode::Radius
            });
            diameter = v["diameter"].as_bool().unwrap();
            if v["circle"].is_null() {
                continue;
            }
            let inputs = v["resolved_inputs"].as_array().unwrap();
            let inputs = &inputs[..inputs.len() - 4];
            let Some(cursor) = inputs.last().filter(|value| value.is_array()) else {
                continue;
            };
            for value in &inputs[..inputs.len() - 1] {
                if let Some(name) = value.as_str() {
                    if !name.starts_with("ProjectOsnap=") {
                        assert!(getter.option(name));
                    }
                } else if value.is_array() {
                    assert!(getter.point(p(value)).unwrap().is_none());
                } else {
                    assert!(getter.number(value.as_f64().unwrap()).unwrap().is_none());
                }
            }
            let cache = RefCell::new(MaelstromPreviewCache::default());
            let preview = MaelstromPreview {
                circle_getter: Some(getter),
                sources: &[],
                center: getter.anchor().unwrap(),
                initial: None,
                target: None,
                options: MaelstromOptions::default(),
                cplane: plane,
                last: None,
                cache: &cache,
            };
            let circles = preview.circles(Some(p(cursor)));
            assert_eq!(circles.len(), 1, "{}", op["id"]);
            let (frame, radius) = circles[0];
            let native = &v["circle"];
            assert!(frame.origin().distance_to(p(&native["origin"])).unwrap() < 1e-11);
            assert!(
                Point3::try_from(frame.x_axis().as_vector().to_array())
                    .unwrap()
                    .distance_to(p(&native["x"]))
                    .unwrap()
                    < 1e-11
            );
            assert!((radius - native["radius"].as_f64().unwrap()).abs() < 1e-11);
            assert_eq!(getter, preview.circle_getter.unwrap());
            checked += 1;
        }
    }
    assert_eq!(checked, 33);
}
