use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn section_plane_is_perpendicular_to_the_cplane_and_contains_the_picked_line() {
    let (frame, length) = section_frame(
        p(-1., 0., 0.),
        p(1., 0., 0.),
        CommandContext::default().construction_plane,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(length, 2.);
    assert_eq!(frame.coordinates_of(p(0., 0., 5.)).unwrap()[2], 0.);
    assert!(frame.coordinates_of(p(0., 2., 0.)).unwrap()[2].abs() > 1.);
}
#[test]
fn crossing_curve_section_preserves_input_and_replays_the_output_as_one_step() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line 0,-2,0 0,2,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let source = doc.objects().next().unwrap().clone();
    registry.execute(&mut doc, "Section -1,0,0 1,0,0").unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(doc.object(source.id()), Some(&source));
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 1);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 2);
}
