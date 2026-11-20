use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::curve_fillet::enter;
use super::*;
use std::time::{Duration, Instant};
use viboceros_geometry::NurbsCurve;

fn setup(profile: &str, pole: Point3) -> (VibocerosApp, [ObjectId; 3]) {
    let mut app = test_app();
    for input in ["Line 0,0,0 0,0,5", profile] {
        app.commands.execute(&mut app.document, input).unwrap();
    }
    app.document.add_geometry(Geometry::Point(pole)).unwrap();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    (app, ids.try_into().unwrap())
}

fn wait_for_wires(
    app: &mut VibocerosApp,
    context: &egui::Context,
    ready: impl Fn(&[NurbsCurve]) -> bool,
) -> Vec<NurbsCurve> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        viewport_frame(app, context, vec![]);
        let wires = app.sweep_session.as_ref().unwrap().preview_wires();
        if ready(wires) {
            return wires.to_vec();
        }
        assert!(
            Instant::now() < deadline,
            "Sweep1 preview did not update: {:?}",
            app.command_log
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn hover_pole(
    app: &mut VibocerosApp,
    context: &egui::Context,
    cursor: Point3,
    pole: Point3,
) -> (egui::Pos2, Vec<NurbsCurve>) {
    viewport_frame(app, context, vec![]);
    let pixel = screen_point(app, cursor);
    viewport_frame(app, context, vec![egui::Event::PointerMoved(pixel)]);
    let wires = wait_for_wires(app, context, |wires| {
        wires.iter().any(|wire| {
            let domain = wire.domain();
            [*domain.start(), *domain.end()]
                .into_iter()
                .any(|parameter| {
                    wire.evaluate(parameter)
                        .is_ok_and(|point| point.distance_to(pole).unwrap() < 1e-5)
                })
        })
    });
    assert!(!viewport_frame(app, context, vec![]).is_empty());
    (pixel, wires)
}

#[test]
fn sweep_point_preview_tracks_the_snapped_pole_and_committed_geometry() {
    for (command, profile, getter, height, rail_first) in [
        ("Sweep1", "Line 0,0,2.5 2,0,2.5", "StartPoint", 0., true),
        (
            "Sweep1 Parameters=2.5 FrameStyle=Roadlike Axis=0,1,0",
            "Circle 0,0,2.5 2",
            "EndPoint",
            5.,
            false,
        ),
    ] {
        let pole = point(1.5, 0., height);
        let (mut app, [rail, profile, _]) = setup(profile, pole);
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let context = egui::Context::default();
        enter(&mut app, command);
        let [first, last] = if rail_first {
            [rail, profile]
        } else {
            [profile, rail]
        };
        for id in [first, last] {
            enter(&mut app, &id.to_string());
        }
        let initial = wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
        enter(&mut app, "Rebuild=7");
        wait_for_wires(&mut app, &context, |wires| {
            !wires.is_empty() && wires != initial
        });
        enter(&mut app, "Undo");
        wait_for_wires(&mut app, &context, |wires| wires == initial);

        enter(&mut app, getter);
        let free = point(-1., 0., height);
        let (_, provisional) = hover_pole(&mut app, &context, free, free);
        viewport_frame(&mut app, &context, vec![egui::Event::PointerGone]);
        assert!(
            app.sweep_session
                .as_ref()
                .unwrap()
                .preview_wires()
                .is_empty()
        );
        enter(&mut app, "Point");
        let (pixel, hovered) = hover_pole(&mut app, &context, point(1.6, 0., height), pole);
        assert_ne!(hovered, provisional);
        assert!(app.snaps.model_override.is_some());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(app.document.undo_label(), None);
        viewport_click(&mut app, &context, pixel);
        assert!(app.snaps.model_override.is_none());
        wait_for_wires(&mut app, &context, |wires| wires == hovered);
        enter(&mut app, "");

        let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
            panic!("expected Sweep1 B-rep: {:?}", app.command_log);
        };
        assert!(brep.vertices().iter().any(|vertex| vertex.point() == pole));
        assert_eq!(
            brep.wireframe_curves(1, app.document.tolerance()).unwrap(),
            hovered
        );
        assert_eq!(app.document.undo_label(), Some("Sweep1"));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn renaming_a_named_rail_invalidates_the_sweep_preview() {
    let (mut app, [rail, profile, _]) = setup("Line 0,0,2.5 2,0,2.5", point(1., 0., 5.));
    app.document
        .set_object_names([(rail, Some("SweepRail".into()))])
        .unwrap();
    enter(&mut app, "Sweep1 RailName=SweepRail");
    enter(&mut app, &profile.to_string());
    let context = egui::Context::default();
    wait_for_wires(&mut app, &context, |wires| !wires.is_empty());
    app.document
        .set_object_names([(rail, Some("OtherRail".into()))])
        .unwrap();
    let label = app.document.undo_label().unwrap().to_owned();
    viewport_frame(&mut app, &context, vec![]);
    assert!(app.sweep_session.is_none());
    assert_eq!(app.document.undo_label(), Some(label.as_str()));
    assert_eq!(app.document.objects().count(), 3);
}
