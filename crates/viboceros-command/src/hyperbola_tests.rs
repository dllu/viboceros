use super::hyperbola::*;
use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn hyperbola_branch_preview_and_asymptotes_share_exact_definition() {
    let mut input = Input::new(
        Options::default(),
        CommandContext::default().construction_plane,
        Tolerance::DEFAULT,
    );
    input.point(p(0., 0., 0.)).unwrap();
    input.point(p(5., 0., 0.)).unwrap();
    input.option("BothBranches=Yes").unwrap();
    input.option("ShowAsymptotes=Yes").unwrap();
    let original = input;
    let preview = input.preview(p(3.75, 3., 0.)).unwrap();
    assert_eq!(original, input);
    let definition = input.point(p(3.75, 3., 0.)).unwrap().unwrap();
    for (a, b) in definition.coefficients().into_iter().zip([3., 4.]) {
        assert!((a - b).abs() < 1e-12);
    }
    assert_eq!(definition.center(), p(0., 0., 0.));
    assert_eq!(
        preview.curves,
        definition.curves(true, Tolerance::DEFAULT).unwrap()
    );
    assert_eq!(preview.foci, [p(-5., 0., 0.), p(5., 0., 0.)]);
    for line in preview.asymptotes.unwrap() {
        for point in line {
            assert!((point.x().powi(2) / 9. - point.y().powi(2) / 16.).abs() < 1e-12);
        }
        assert_eq!(
            line[0]
                .translated(line[0].vector_to(line[1]).unwrap().scaled(0.5).unwrap())
                .unwrap(),
            definition.center()
        );
    }
    assert!(preview.curves[0].control_points()[0].point().x() < 0.);
    assert!(preview.curves[1].control_points()[0].point().x() > 0.);
}
#[test]
fn hyperbola_invalid_getters_coefficients_and_options_are_atomic_and_can_retry() {
    let mut input = Input::new(
        Options::default(),
        CommandContext::default().construction_plane,
        Tolerance::DEFAULT,
    );
    assert!(input.option("FromCoefficient").unwrap());
    input.point(p(0., 0., 0.)).unwrap();
    let before = input;
    assert!(input.point(p(0., 0., 0.)).is_err());
    assert_eq!(input, before);
    input.point(p(1., 0., 0.)).unwrap();
    let before = input;
    for text in ["A=0", "B=-1", "A=nan", "BothBranches=Maybe"] {
        assert!(input.option(text).is_err());
        assert_eq!(input, before);
    }
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(input.coefficient(Coefficient::A, value).is_err());
        assert_eq!(input, before);
    }
    input.option("A=3").unwrap();
    input.option("B=4").unwrap();
    let before = input;
    assert!(input.point(p(2., 2., 0.)).is_err());
    assert_eq!(input, before);
    input.option("ShowAsymptotes").unwrap();
    let preview = input.preview(p(5., 2., 0.)).unwrap();
    assert!(preview.asymptotes.is_some());
    assert!(input.point(p(5., 2., 0.)).unwrap().is_some());
}
#[test]
fn hyperbola_remembers_drawing_coefficients_but_starts_at_center_and_keeps_registries_isolated() {
    let commands = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    commands.execute(&mut doc,"Hyperbola FromCoefficient 0,0,0 1,0,0 5,2,0 A=3 B=4 BothBranches=Yes MarkFoci=Yes ShowAsymptotes=Yes").unwrap();
    let saved = commands.hyperbola_options();
    assert_eq!(saved.a, 3.);
    assert_eq!(saved.b, 4.);
    assert!(saved.both_branches && saved.mark_foci && saved.show_asymptotes);
    doc.undo().unwrap();
    assert_eq!(commands.hyperbola_options(), saved);
    assert!(
        commands
            .execute(&mut doc, "Hyperbola FromCoefficient 0,0,0 1,0,0 2,2,0 A=8")
            .is_err()
    );
    assert_eq!(commands.hyperbola_options(), saved);
    assert_eq!(
        CommandRegistry::with_builtins().hyperbola_options(),
        Options::default()
    );
    commands
        .execute(&mut doc, "Hyperbola 0,0,0 5,0,0 3.75,3,0")
        .unwrap();
    assert_eq!(doc.objects().count(), 4);
    assert_eq!(doc.undo().unwrap().as_deref(), Some("Hyperbola"));
    assert!(doc.undo().unwrap().is_none());
}
#[test]
fn hyperbola_projective_plane_chart_keeps_weights_and_declines_length_at_its_pole() {
    let frame = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut input = Input::new(Options::default(), frame, Tolerance::DEFAULT);
    input.point(p(0., 0., 0.)).unwrap();
    input.point(p(5., 0., 0.)).unwrap();
    let preview = input.preview(p(3.75, 3., 0.)).unwrap();
    let c = &preview.curves[0];
    assert_eq!(c.control_points()[1].weight(), -1.);
    assert_eq!(c.quadratic_pole_fractions(), Some([0.5, 0.5]));
    assert!(c.evaluate(0.5).is_err());
    assert!(c.length(Tolerance::DEFAULT).is_err());
}
