use super::*;
use crate::{CommandContext, CommandRegistry};
use viboceros_document::{Document, Geometry};
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn input() -> Input {
    Input::new(
        Options::default(),
        CommandContext::default().construction_plane,
        Tolerance::DEFAULT,
    )
}
fn rail(i: &mut Input) {
    for q in [p(0., 0., 0.), p(10., 0., 0.), p(0., 0., -1.)] {
        assert!(!i.point(q).unwrap());
    }
}
#[test]
fn catenary_preview_commit_apex_and_history_share_one_definition() {
    let mut i = input();
    rail(&mut i);
    i.option("MarkApex=Yes").unwrap();
    i.option("PointCount=8").unwrap();
    let before = i;
    let preview = i.preview(p(3., 0., -2.)).unwrap();
    assert_eq!(i, before);
    i.point(p(3., 0., -2.)).unwrap();
    let expected = i.solution().unwrap();
    assert_eq!(&preview.curve, expected.curve());
    assert_eq!(preview.apex, Some(expected.apex()));
    let commands = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    commands.execute(&mut doc, &i.command().unwrap()).unwrap();
    assert_eq!(doc.objects().len(), 2);
    let objects = doc.objects().collect::<Vec<_>>();
    assert_eq!(objects[0].geometry(), &Geometry::Point(expected.apex()));
    let Geometry::NurbsCurve(curve) = objects[1].geometry() else {
        panic!()
    };
    assert_eq!(curve, &expected.curve().to_nurbs().unwrap());
    assert!(objects.iter().all(|o| !doc.is_selected(o.id())));
    let remembered = commands.catenary_options();
    assert_eq!(remembered.length, Some(expected.length()));
    assert_eq!(remembered.parameter, Some(expected.parameter()));
    assert_eq!(doc.undo().unwrap().as_deref(), Some("Catenary"));
    assert_eq!(doc.objects().len(), 0);
    assert_eq!(commands.catenary_options(), remembered);
    assert_eq!(doc.redo().unwrap().as_deref(), Some("Catenary"));
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(commands.catenary_options(), remembered);
}
#[test]
fn catenary_invalid_picks_counts_and_numbers_leave_the_getter_unchanged() {
    let mut i = input();
    assert!(i.scalar_default().is_none());
    i.point(p(0., 0., 0.)).unwrap();
    let before = i;
    assert!(i.point(p(0., 0., 0.)).is_err());
    assert_eq!(i, before);
    i.point(p(10., 0., 0.)).unwrap();
    let before = i;
    assert!(i.point(p(5., 0., 0.)).is_err());
    assert_eq!(i, before);
    i.point(p(0., 0., -1.)).unwrap();
    let before = i;
    assert!(i.point(p(10., 0., 0.)).is_err());
    assert_eq!(i, before);
    for word in [
        "Mode=Unknown",
        "PointCount=3",
        "PointCount=2.5",
        "PointCount=999999",
        "Output=Unknown",
        "MarkApex=Maybe",
    ] {
        assert!(i.option(word).is_err());
        assert_eq!(i, before);
    }
    i.option("Mode=Length").unwrap();
    let before = i;
    for n in [0., 10., Real::NAN, Real::INFINITY] {
        assert!(i.number(n).is_err());
        assert_eq!(i, before);
    }
    assert!(i.enter().is_err());
    assert_eq!(i, before);
    i.number(14.).unwrap();
    assert_eq!(i.slot(), Slot::Complete);
}
#[test]
fn catenary_numeric_points_measure_from_start_and_select_the_sag_side() {
    let mut i = input();
    i.point(p(5., 2., 0.)).unwrap();
    i.point(p(15., 2., 0.)).unwrap();
    i.point(p(5., 2., -1.)).unwrap();
    i.option("Mode=Parameter").unwrap();
    let before = i;
    let preview = i.preview(p(5., 5., 4.)).unwrap();
    assert_eq!(i, before);
    i.point(p(5., 5., 4.)).unwrap();
    let solution = i.solution().unwrap();
    assert_eq!(solution.parameter(), 5.);
    assert!((solution.apex().z() - 2.7154031740762186).abs() < 1e-12);
    assert_eq!(preview.curve, solution.into_curve());
}
#[test]
fn catenary_enter_at_oblique_end_has_a_dedicated_apex_interpretation() {
    let mut i = input();
    for q in [p(0., 0., 0.), p(10., 0., -2.), p(0., 0., -1.)] {
        i.point(q).unwrap();
    }
    assert!(i.point(p(10., 0., -2.)).is_err());
    assert_eq!(i.slot(), Slot::Constraint);
    i.enter().unwrap();
    assert!(
        i.solution()
            .unwrap()
            .apex()
            .distance_to(p(10., 0., -2.))
            .unwrap()
            < 1e-12
    );
    assert!(i.command().unwrap().contains("Mode=Apex"));
    assert_eq!(i.options().mode, Mode::ThroughPoint);
    let commands = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    commands.execute(&mut doc, &i.command().unwrap()).unwrap();
    commands.remember_catenary_options(i.options());
    assert_eq!(commands.catenary_options().mode, Mode::ThroughPoint);
}
#[test]
fn catenary_length_and_parameter_memory_crosses_modes_but_not_registries() {
    let commands = CommandRegistry::with_builtins();
    let other = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    commands
        .execute(
            &mut doc,
            "Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 Mode=ThroughPoint PointCount=8",
        )
        .unwrap();
    let mut i = Input::new(
        commands.catenary_options(),
        CommandContext::default().construction_plane,
        doc.tolerance(),
    );
    rail(&mut i);
    i.option("Mode=Length").unwrap();
    i.enter().unwrap();
    assert!(
        (i.options().parameter.unwrap() - commands.catenary_options().parameter.unwrap()).abs()
            < 1e-12
    );
    assert_eq!(other.catenary_options(), Options::default());
    i.option("Mode=Parameter").unwrap_err();
}
#[test]
fn catenary_default_axis_keeps_the_initial_plane_and_polyline_count_has_ui_limits() {
    let mut i = input();
    i.point(p(1., 2., 3.)).unwrap();
    i.set_initial_plane(
        Frame3::try_from_directions(
            p(4., 5., 6.),
            viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
            viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    i.point(p(11., 2., 3.)).unwrap();
    assert_eq!(i.axis_guide(), Some([p(1., 2., 3.), p(1., 1., 3.)]));
    i.enter().unwrap();
    assert_eq!(i.slot(), Slot::Constraint);
    i.option("Output").unwrap();
    i.option("PointCount=4").unwrap();
    let before = i;
    assert!(i.option("PointCount=2").is_err());
    assert_eq!(i, before);
    i.option("Mode=Parameter").unwrap();
    i.number(4.).unwrap();
    let CatenaryCurve::Polyline(poly) = i.solution().unwrap().into_curve() else {
        panic!()
    };
    assert_eq!(poly.vertices().len(), 4);
}
