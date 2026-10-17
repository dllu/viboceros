use super::*;
use crate::{CommandContext, CommandRegistry};
use viboceros_document::{Document, Geometry};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn input() -> Input {
    Input::new(
        Options::default(),
        CommandContext::default().construction_plane,
        Tolerance::DEFAULT,
    )
}
#[test]
fn ellipse_projected_axes_numeric_locks_preview_and_command_share_geometry() {
    let mut i = input();
    i.point(p(0., 0., 0.)).unwrap();
    i.number(4.).unwrap();
    i.number(6.).unwrap();
    let before = i;
    assert!(i.point(p(0., 0., 0.)).is_err());
    assert_eq!(i, before);
    i.point(p(2., 0., 0.)).unwrap();
    assert_eq!(i.reference(), Some(p(6., 0., 0.)));
    let ordinary = i.preview(p(2., 3., 0.)).unwrap();
    assert_eq!(ordinary.ellipse.radius_y(), 3.);
    i.number(5.).unwrap();
    i.number(3.).unwrap();
    let before = i;
    let preview = i.preview(p(2., -3., 0.)).unwrap();
    assert_eq!(i, before);
    assert_eq!(preview.ellipse.radius_x(), 6.);
    assert_eq!(preview.ellipse.radius_y(), 3.);
    assert!(i.point(p(3., 0., 0.)).is_err());
    assert_eq!(i, before);
    assert_eq!(i.point(p(2., -3., 0.)).unwrap(), Some(preview));
    let commands = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    commands.execute(&mut doc, &i.command().unwrap()).unwrap();
    assert_eq!(
        doc.objects().next().unwrap().geometry(),
        &Geometry::Ellipse(preview.ellipse)
    );
    assert_eq!(doc.undo().unwrap().as_deref(), Some("Ellipse"));
    assert_eq!(doc.objects().len(), 0);
    assert_eq!(doc.redo().unwrap().as_deref(), Some("Ellipse"));
    assert_eq!(doc.objects().len(), 1);
}
#[test]
fn ellipse_options_and_invalid_constructions_are_atomic_and_registry_local() {
    let mut i = input();
    let before = i;
    assert!(i.option("Deformable=Maybe").is_err());
    assert_eq!(i, before);
    i.option("FromFoci").unwrap();
    let before = i;
    assert!(i.option("MarkFoci=Maybe").is_err());
    assert_eq!(i, before);
    i.option("MarkFoci=Yes").unwrap();
    i.point(p(0., 0., 0.)).unwrap();
    i.point(p(8., 0., 0.)).unwrap();
    let before = i;
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(i.number(value).is_err());
        assert_eq!(i, before);
    }
    assert!(i.point(p(2., 0., 0.)).is_err());
    assert_eq!(i, before);
    assert!(i.preview(p(4., 3., 0.)).unwrap().foci.is_some());
    i.point(p(4., 3., 0.)).unwrap();
    let registry = CommandRegistry::with_builtins();
    let other = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, &i.command().unwrap()).unwrap();
    let saved = registry.ellipse_options();
    assert!(saved.mark_foci);
    assert!(!other.ellipse_options().mark_foci);
    assert_eq!(doc.objects().len(), 3);
    assert!(doc.objects().all(|o| !doc.is_selected(o.id())));
    doc.undo().unwrap();
    doc.redo().unwrap();
    assert_eq!(registry.ellipse_options(), saved);
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for script in [
        "Ellipse FromFoci 0,0 8,0 2,0 MarkFoci=No",
        "Ellipse 0,0 6,0 2,0",
        "Ellipse Corner 0,0 0,3",
        "Ellipse 0,0 6,0 0,3 99",
    ] {
        assert!(registry.execute(&mut doc, script).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(registry.ellipse_options(), saved);
    }
}
#[test]
fn ellipse_corner_switch_retains_center_and_diameter_sizes_are_full_lengths() {
    let mut i = input();
    i.point(p(1., 2., 3.)).unwrap();
    i.option("Corner").unwrap();
    let e = i.point(p(7., 10., 7.)).unwrap().unwrap().ellipse;
    assert_eq!(e.center(), p(1., 2., 3.));
    assert_eq!((e.radius_x(), e.radius_y()), (6., 8.));
    let mut i = input();
    i.option("Corner").unwrap();
    i.point(p(1., 2., 3.)).unwrap();
    let e = i.point(p(7., 10., 7.)).unwrap().unwrap().ellipse;
    assert_eq!(e.center(), p(4., 6., 3.));
    assert_eq!((e.radius_x(), e.radius_y()), (3., 4.));
    let mut i = input();
    i.option("Diameter").unwrap();
    i.option("Vertical").unwrap();
    i.point(p(-6., 0., 0.)).unwrap();
    i.number(12.).unwrap();
    i.point(p(2., 0., 0.)).unwrap();
    i.number(6.).unwrap();
    let e = i.point(p(0., 2., -4.)).unwrap().unwrap().ellipse;
    assert_eq!((e.radius_x(), e.radius_y()), (6., 3.));
    assert_eq!(e.quadrants().unwrap()[1], p(0., 0., -3.));
}

#[test]
fn ellipse_focal_replay_preserves_the_captured_plane_without_an_ambient_cplane() {
    let plane = Frame3::try_from_directions(
        p(3., 4., 5.),
        viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut i = Input::new(
        Options {
            mark_foci: true,
            ..Options::default()
        },
        plane,
        Tolerance::DEFAULT,
    );
    i.option("FromFoci").unwrap();
    i.point(p(0., 0., 0.)).unwrap();
    i.point(p(0., 8., 0.)).unwrap();
    let expected = i.point(p(0., 12., 0.)).unwrap().unwrap();
    let mut doc = Document::default();
    CommandRegistry::with_builtins()
        .execute(&mut doc, &i.command().unwrap())
        .unwrap();
    let actual = doc
        .objects()
        .find_map(|o| {
            if let Geometry::Ellipse(e) = o.geometry() {
                Some(*e)
            } else {
                None
            }
        })
        .unwrap();
    for (a, b) in actual
        .quadrants()
        .unwrap()
        .into_iter()
        .zip(expected.ellipse.quadrants().unwrap())
    {
        assert!(a.distance_to(b).unwrap() < 1e-12);
    }
}

#[test]
fn ellipse_around_projects_axes_and_retains_native_source_selection_replay_policy() {
    use viboceros_document::SelectionMode;
    use viboceros_geometry::LineSegment;
    for preselected in [false, true] {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let source = doc
            .add_geometry(Geometry::Line(
                LineSegment::try_new(p(0., 0., 0.), p(0., 0., 10.), doc.tolerance()).unwrap(),
            ))
            .unwrap();
        let saved = doc.object(source).unwrap().clone();
        if preselected {
            doc.select_object(source, SelectionMode::Replace).unwrap();
        }
        doc.clear_history().unwrap();
        registry
            .execute(
                &mut doc,
                &format!("Ellipse AroundCurve Curve={source} 2,3,5 3,0,9 0,2,8"),
            )
            .unwrap();
        let Geometry::Ellipse(e) = doc.objects().find(|o| o.id() != source).unwrap().geometry()
        else {
            panic!()
        };
        assert_eq!(e.center(), p(0., 0., 5.));
        assert_eq!((e.radius_x(), e.radius_y()), (3., 2.));
        assert_eq!(doc.is_selected(source), preselected);
        assert_eq!(doc.undo().unwrap().as_deref(), Some("Ellipse"));
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(doc.is_selected(source), preselected);
        assert_eq!(doc.redo().unwrap().as_deref(), Some("Ellipse"));
        assert_eq!(doc.is_selected(source), preselected);
        assert_eq!(doc.object(source).unwrap(), &saved);
        assert!(
            saved
                .geometry_snapshot()
                .shares_storage_with(doc.object(source).unwrap().geometry_snapshot())
        );
        let mut input = input();
        input.option("AroundCurve").unwrap();
        assert!(input.requests_curve());
        assert!(input.point(p(0., 0., 5.)).is_err());
        input.around_curve(
            crate::curve_reference::perpendicular_frame(
                saved.geometry().curve_ref().unwrap(),
                p(1., 2., 5.),
            )
            .unwrap(),
        );
        let before = input;
        assert!(input.point(p(0., 0., 8.)).is_err());
        assert_eq!(input, before);
        input.number(6.).unwrap();
        input.point(p(2., 0., 9.)).unwrap();
        input.number(3.).unwrap();
        let before = input;
        let preview = input.preview(p(2., 4., 8.)).unwrap();
        assert_eq!(input, before);
        assert_eq!(input.point(p(2., 4., 8.)).unwrap(), Some(preview));
        assert_eq!(
            (preview.ellipse.radius_x(), preview.ellipse.radius_y()),
            (6., 3.)
        );
    }
}

#[test]
fn ellipse_switch_from_around_curve_discards_the_pending_guide() {
    let mut doc = Document::default();
    let guide = doc
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                p(0., 0., 0.),
                p(0., 0., 10.),
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    doc.clear_history().unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut doc,
            &format!("Ellipse AroundCurve Curve={guide} Default 1,2,3 4,2,3 1,4,3"),
        )
        .unwrap();
    let Geometry::Ellipse(ellipse) = doc.objects().find(|o| o.id() != guide).unwrap().geometry()
    else {
        panic!()
    };
    assert_eq!(ellipse.center(), p(1., 2., 3.));
    assert_eq!((ellipse.radius_x(), ellipse.radius_y()), (3., 2.));
    assert!(!doc.is_selected(guide));
    assert_eq!(doc.undo().unwrap().as_deref(), Some("Ellipse"));
    assert_eq!(doc.objects().len(), 1);
}

#[test]
fn ellipse_deform_count_pending_validation_full_scripts_and_dense_periodic_seam() {
    let mut i = input();
    i.option("Deformable").unwrap();
    i.option("PointCount").unwrap();
    let before = i;
    for number in [0., 3., 3.5, 1001., f64::NAN, f64::INFINITY] {
        assert!(i.number(number).is_err());
        assert_eq!(i, before);
    }
    assert!(i.point(p(0., 0., 0.)).is_err());
    assert_eq!(i, before);
    i.option("Enter").unwrap();
    assert_eq!(i.options().point_count, 12);
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(
            &mut doc,
            "Ellipse Deformable PointCount 8 0 0 0 6 0 0 0 3 0",
        )
        .unwrap();
    let Geometry::NurbsCurve(curve) = doc.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(curve.degree(), 3);
    assert_eq!(curve.control_points().len(), 11);
    assert!(curve.is_periodic());
    assert!(curve.is_closed().unwrap());
    assert!(curve.control_points().iter().all(|p| p.weight() == 1.));
    assert_eq!(doc.undo().unwrap().as_deref(), Some("Ellipse"));
    assert_eq!(doc.objects().len(), 0);
    let e = Ellipse3::try_from_three_points(
        p(0., 0., 0.),
        p(6., 0., 0.),
        p(0., 3., 0.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let dense = e.try_deformable(1000).unwrap();
    assert_eq!(dense.control_points().len(), 1003);
    for station in [0, 1, 123, 499, 999] {
        let fraction = station as f64 / 1000.;
        let expected = e.point_at_angle(std::f64::consts::TAU * fraction).unwrap();
        assert!(
            dense
                .evaluate(6_f64.hypot(3.) * fraction)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-12
        );
    }
}
