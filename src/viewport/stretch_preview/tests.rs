use super::*;
use crate::deformation_test_support as support;
use crate::viewport::preview_test_support::{near, point};

#[test]
fn stretch_cached_points_and_rigid_groups_match_native_in_all_display_modes() {
    let mut count = 0;
    for (op, value) in support::rows() {
        if op["op"] != "stretch_command" {
            continue;
        }
        let (doc, sources) = support::setup(&op, &value);
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let (start, end) = support::axis(&op);
        let target = support::targets(&op)[0]
            .morph(start, end)
            .unwrap()
            .morph_point(end)
            .unwrap();
        let cache = RefCell::new(StretchPreviewCache::default());
        let display = RefCell::new(DisplayCache::default());
        let preview = StretchPreview {
            sources: &sources.ids,
            grips: &[],
            start,
            end,
            options: support::options(&op),
            last_point: Some(target),
            cache: &cache,
        };
        let image = preview.resolve(None, &doc, &display).0.unwrap();
        assert!(Rc::ptr_eq(
            &image,
            &preview.resolve(None, &doc, &display).0.unwrap()
        ));
        let copy_offset = if op["copy"] == true && op["factor"] != 1. {
            sources.ids.len()
        } else {
            0
        };
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            for (i, id) in sources.ids.iter().enumerate() {
                let object = &image.objects_for_mode(mode)[id];
                let Geometry::Point(p) = &*object.geometry.geometry else {
                    panic!();
                };
                let actual = object
                    .transform
                    .map_or(*p, |t| t.transform_point(*p).unwrap());
                near(
                    actual,
                    support::point(&value["after"][i + copy_offset]["point"]),
                    1e-10,
                    op["id"].as_str().unwrap(),
                );
            }
        }
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        count += 1;
    }
    assert_eq!(count, 16);
}

#[test]
fn stretch_preview_invalidates_geometry_and_grip_picks_and_constrains_mouse_axis() {
    let (op, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "grips_circle_00")
        .unwrap();
    let (mut doc, sources) = support::setup(&op, &value);
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let cache = RefCell::new(StretchPreviewCache::default());
    let display = RefCell::new(DisplayCache::default());
    let (start, end) = support::axis(&op);
    let preview = StretchPreview {
        sources: &[],
        grips: &sources.grips,
        start,
        end,
        options: Options::default(),
        last_point: Some(point(3., 0., 0.)),
        cache: &cache,
    };
    let constraint = preview.mouse_line().unwrap();
    assert_eq!(constraint.anchor, start);
    assert_eq!(constraint.direction, Some(start.direction_to(end).unwrap()));
    let image = preview.resolve(None, &doc, &display).0.unwrap();
    for id in &sources.grips {
        near(
            image.grips[id],
            support::point(&value["after"][0]["grips"][id.index]["point"]),
            1e-12,
            "selected grip preview",
        );
    }
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    let changed = StretchPreview {
        grips: &sources.grips[..1],
        ..preview
    };
    let next = changed.resolve(None, &doc, &display).0.unwrap();
    assert!(!Rc::ptr_eq(&image, &next));
    assert_ne!(image.grips, next.grips);
    doc.delete_objects([sources.source.unwrap()]).unwrap();
    assert!(preview.resolve(None, &doc, &display).0.is_none());
}

#[test]
fn stretch_viewport_mouse_click_and_retained_hover_use_the_same_axis_target() {
    use crate::viewport::preview_test_support::frame;
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(point(2., 1., 5.)))
        .unwrap();
    doc.clear_history().unwrap();
    let source = doc.objects().cloned().collect::<Vec<_>>();
    let cache = RefCell::new(StretchPreviewCache::default());
    let preview = StretchPreview {
        sources: &[id],
        grips: &[],
        start: point(0., 0., 0.),
        end: point(0., 0., 10.),
        options: Options::default(),
        last_point: None,
        cache: &cache,
    };
    let target = point(0., 0., 20.);
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        let context = egui::Context::default();
        let mut view = Viewport::new(ViewKind::Front);
        view.pixels_per_unit = 10.;
        view.display_mode = mode;
        let input = ViewportInput {
            stretch_preview: Some(preview),
            drafting: DraftingInput {
                active: true,
                anchor: Some(preview.end),
                ..Default::default()
            },
            ..Default::default()
        };
        frame(&context, &mut view, &doc, input, vec![]);
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
        near(output.picked_point.unwrap(), target, 1e-10, "axis click");
        assert_eq!(output.stretch_preview.unwrap(), output.picked_point);
        let retained = StretchPreview {
            last_point: output.picked_point,
            ..preview
        };
        let image = retained.resolve(None, &doc, &view.display_cache).0.unwrap();
        let Geometry::Point(p) = &*image.objects[&id].geometry.geometry else {
            panic!();
        };
        near(*p, point(2., 1., 10.), 1e-10, "native cubic midpoint");
        let output = frame(
            &context,
            &mut view,
            &doc,
            ViewportInput {
                stretch_preview: Some(retained),
                ..input
            },
            vec![egui::Event::PointerGone],
        );
        assert_eq!(output.stretch_preview, None);
        assert!(Rc::ptr_eq(
            &image,
            &retained.resolve(None, &doc, &view.display_cache).0.unwrap()
        ));
    }
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), source);
    assert!(!doc.can_undo());
}
