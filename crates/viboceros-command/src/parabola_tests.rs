use super::parabola::*;
use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn parabola_three_point_all_orders_share_geometry_and_last_pick_direction_anchor() {
    let s = p(-1., 0., 0.25);
    let end = p(3., 0., 2.25);
    for mode in [Mode::Focus, Mode::Vertex, Mode::ThroughPoint] {
        let special = match mode {
            Mode::Focus => p(0., 0., 1.),
            Mode::Vertex => p(0., 0., 0.),
            Mode::ThroughPoint => p(1., 0., 0.25),
        };
        let mut reference = None;
        for order in [
            PickOrder::Default,
            PickOrder::EndsFirst,
            PickOrder::EndsLast,
        ] {
            let points = match order {
                PickOrder::Default => [s, special, end],
                PickOrder::EndsFirst => [s, end, special],
                PickOrder::EndsLast => [special, s, end],
            };
            let mut input = Input::new(
                Kind::ThreePoint,
                Options {
                    mode,
                    pick_order: order,
                    ..Options::default()
                },
                CommandContext::default().construction_plane,
                Tolerance::DEFAULT,
            );
            let mut definition = None;
            for point in points {
                definition = input.point(point).unwrap();
            }
            if mode == Mode::ThroughPoint {
                assert!(definition.is_none());
                assert_eq!(input.slot(), Slot::OpeningDirection);
                assert_eq!(input.anchor(), Some(points[2]));
                let cursor = points[2]
                    .translated(Vector3::try_new(0., 0., 1.).unwrap())
                    .unwrap();
                let original = input;
                let preview = input.preview(cursor).unwrap();
                assert_eq!(input, original);
                definition = input.point(cursor).unwrap();
                assert_eq!(definition.as_ref(), Some(&preview));
            }
            let d = definition.unwrap();
            assert_eq!(d.curve.control_points()[0].point(), s);
            assert_eq!(d.curve.control_points()[2].point(), end);
            if let Some(reference) = &reference {
                assert_eq!(&d, reference);
            } else {
                reference = Some(d);
            }
        }
    }
}
#[test]
fn parabola_failed_getters_and_options_preserve_input_and_allow_retry() {
    let mut i = Input::new(
        Kind::Axis,
        Options::default(),
        CommandContext::default().construction_plane,
        Tolerance::DEFAULT,
    );
    i.point(p(0., 0., 1.)).unwrap();
    let before = i;
    assert!(i.point(p(0., 0., 1.)).is_err());
    assert_eq!(before, i);
    assert!(i.option("Half=Maybe").is_err());
    assert_eq!(before, i);
    i.point(p(0., 0., 2.)).unwrap();
    let before = i;
    assert!(i.point(p(0., 0., 3.)).is_err());
    assert_eq!(before, i);
    assert!(i.option("Half").unwrap());
    assert!(i.option("MarkFocus=Yes").unwrap());
    let d = i.point(p(2., 0., 1.)).unwrap().unwrap();
    assert_eq!(d.curve.control_points()[0].point(), p(0., 0., 0.));
}
#[test]
fn parabola_preferences_survive_undo_are_separate_and_only_success_is_remembered() {
    let commands = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    commands
        .execute(
            &mut doc,
            "Parabola Vertex 0,0,0 0,0,1 2,0,0 Half=Yes MarkFocus=Yes",
        )
        .unwrap();
    let settings = commands.parabola_options(Kind::Axis);
    doc.undo().unwrap();
    assert_eq!(settings, commands.parabola_options(Kind::Axis));
    assert!(
        commands
            .execute(&mut doc, "Parabola Focus 0,0,0 0,0,0 2,0,0 Half=No")
            .is_err()
    );
    assert_eq!(settings, commands.parabola_options(Kind::Axis));
    assert_eq!(
        commands.parabola_options(Kind::ThreePoint),
        Options::default()
    );
    assert_eq!(
        CommandRegistry::with_builtins().parabola_options(Kind::Axis),
        Options::default()
    );
    commands
        .execute(&mut doc, "Parabola 0,0,0 0,0,1 2,0,0")
        .unwrap();
    assert_eq!(doc.objects().count(), 2);
}
