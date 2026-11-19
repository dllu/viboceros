use super::*;

#[test]
fn oblique_boundary_surface_uses_world_stock_bounds_for_a_tilted_source() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let rotate = AffineTransform3::try_rotation(
        point(0., 0., 0.),
        UnitVector3::try_new(1., 0., 0., tolerance).unwrap(),
        25_f64.to_radians(),
    )
    .unwrap();
    let [x, y, z] = rotate
        .transform_vector(Vector3::try_new(1., 0., 6.).unwrap())
        .unwrap()
        .to_array();
    let direction = format!("Direction={x},{y},{z}");
    let options =
        TaperedExtrusionOptions::parse(&[&direction, "DraftAngle=8", "DeleteInput=Yes"]).unwrap();
    let profile = Brep::try_planar_face(&circle(tolerance), tolerance)
        .unwrap()
        .transformed(rotate, tolerance)
        .unwrap();
    let commands = CommandRegistry::with_builtins();
    for (rectangle, far_point) in [
        ([-2., -2.7, 3.13, 3.01], None),
        ([-2., -2.7, 3.29, 2.7], None),
        (
            [-2., -7., 3.13, 7.],
            Some(point(
                3.433403353001881,
                2.7189233611099497,
                15.484585336600638,
            )),
        ),
    ] {
        let mut doc = Document::new(tolerance);
        let source = doc.add_geometry(Geometry::Brep(profile.clone())).unwrap();
        let chart = cutter(rectangle, true).transformed(rotate).unwrap();
        let object = doc.add_geometry(Geometry::NurbsSurface(chart)).unwrap();
        let reference = BoundaryRef { object, face: None };
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        assert_eq!(
            commands
                .execute_surface_tapered_boundary_pick(
                    &mut doc,
                    &SurfaceFaces::References(vec![(source, 0)]),
                    options,
                    reference,
                    CommandContext::default().construction_plane,
                )
                .unwrap(),
            TaperedBoundaryOutcome::Fallback,
            "{rectangle:?}"
        );
        assert!(doc.is_selected(source));
        assert!(doc.is_selected(object));
        let product = doc
            .objects()
            .find(|obj| ![source, object].contains(&obj.id()));
        let Some(expected) = far_point else {
            assert!(product.is_none(), "{rectangle:?}");
            assert!(!doc.can_undo());
            continue;
        };
        let Geometry::Brep(brep) = product.unwrap().geometry() else {
            panic!("expected the temporary sweep");
        };
        assert!(!brep.is_solid());
        assert_eq!((brep.faces().len(), brep.edges().len()), (5, 12));
        let bounds = Shape::from_brep(brep, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        assert!(bounds.max().is_near(expected, tolerance), "{bounds:?}");
    }
}
