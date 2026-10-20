use viboceros_command::{
    CommandRegistry,
    array_curve_surface::{Layout, Placement, References},
};
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{LineSegment, NurbsSurface, Point3, PointCloud3, Tolerance};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_from([x, y, z]).unwrap()
}
fn setup() -> (Document, References, viboceros_document::ObjectId) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap());
    let source = doc
        .add_geometry(Geometry::PointCloud(
            PointCloud3::try_new(vec![
                p(0., 0., 0.),
                p(1., 0., 0.),
                p(0., 1., 0.),
                p(0., 0., 1.),
            ])
            .unwrap(),
        ))
        .unwrap();
    let curve = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 5.), p(0., 10., 5.), doc.tolerance()).unwrap(),
        ))
        .unwrap();
    let surface = doc
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                p(-20., -20., 5.),
                p(20., -20., 5.),
                p(20., 20., 5.),
                p(-20., 20., 5.),
            ])
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    (
        doc,
        References {
            curve,
            edge: None,
            surface,
            face: None,
            pick: None,
        },
        source,
    )
}
#[test]
fn surface_curve_division_maps_the_source_axes_to_the_tangent_and_surface_normal() {
    let (mut doc, refs, source) = setup();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let placement = Placement::try_new(
        &doc,
        p(0., 0., 0.),
        viboceros_command::CommandContext::default().construction_plane,
        refs,
        None,
        false,
    )
    .unwrap();
    let maps = placement.transforms(&Layout::Divide(3), 1).unwrap();
    assert_eq!(maps.len(), 3);
    for (map, y) in maps.iter().zip([0., 5., 10.]) {
        for (from, to) in [
            (p(0., 0., 0.), p(0., y, 5.)),
            (p(1., 0., 0.), p(0., y + 1., 5.)),
            (p(0., 1., 0.), p(-1., y, 5.)),
            (p(0., 0., 1.), p(0., y, 6.)),
        ] {
            assert!(map.transform_point(from).unwrap().distance_to(to).unwrap() < 1e-12);
        }
    }
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.undo_label().is_none());
    CommandRegistry::with_builtins()
        .execute(
            &mut doc,
            &format!("ArrayCrvOnSrf 0,0,0 {} Divide=3", refs.arguments()),
        )
        .unwrap();
    assert_eq!(doc.objects().count(), 6);
    assert!(doc.is_selected(source));
    assert_eq!(doc.undo_label(), Some("ArrayCrvOnSrf"));
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    doc.redo().unwrap();
    assert_eq!(doc.objects().count(), 6);
}
#[test]
fn surface_curve_array_invalid_layouts_and_singular_frames_preserve_redo() {
    let (mut doc, refs, _) = setup();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for layout in [
        "Divide=0",
        "Divide=1000001",
        "Multiple=0",
        "Multiple=NaN",
        "Distances=3,20",
        "Divide=2 Multiple=3",
    ] {
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("ArrayCrvOnSrf 0,0,0 {} {layout}", refs.arguments())
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.redo_label(), Some("Move"));
        assert!(doc.undo_label().is_none());
    }
    doc.replace_object_geometries([(
        refs.curve,
        Geometry::Line(
            LineSegment::try_new(p(0., 0., 5.), p(0., 0., 15.), doc.tolerance()).unwrap(),
        ),
    )])
    .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("ArrayCrvOnSrf 0,0,0 {} Divide=3", refs.arguments())
            )
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn zero_spacing_and_exhausted_multiple_preserve_redo_for_explicit_command_picks() {
    let (mut doc, refs, source) = setup();
    let commands = CommandRegistry::with_builtins();
    commands.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for args in [
        "Multiple=3,0",
        "Multiple=3,20",
        "Multiple=3,0 SubCrv=0.2,0.8",
    ] {
        commands
            .execute(
                &mut doc,
                &format!(
                    "ArrayCrvOnSrf 0,0,0 {} {args} PickedSources={source}",
                    refs.arguments()
                ),
            )
            .unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.undo_label().is_none());
        assert_eq!(doc.redo_label(), Some("Move"));
    }
}
