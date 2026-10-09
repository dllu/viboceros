use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn contour_preserves_sources_and_undoes_all_planes_and_groups_together() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line -2,0,0 2,0,0").unwrap();
    registry.execute(&mut doc, "Line -2,1,0 2,1,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let source = doc.objects().next().unwrap().clone();
    registry
        .execute(
            &mut doc,
            "Contour 0,0,0 1,0,0 1 GroupObjectsByContourPlane=Yes",
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 12);
    assert_eq!(doc.groups().len(), 5);
    assert_eq!(doc.object(source.id()), Some(&source));
    let points = doc
        .objects()
        .filter_map(|o| match o.geometry() {
            Geometry::Point(p) => Some(p.x()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(points, vec![2., 2., 1., 1., 0., 0., -1., -1., -2., -2.]);
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(doc.groups().len(), 0);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 12);
    assert_eq!(doc.groups().len(), 5);
}
#[test]
fn invalid_and_excessive_contours_leave_model_and_history_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line -2,0,0 2,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let label = doc.undo_label().map(str::to_owned);
    for command in [
        "Contour 0,0,0 1,0,0 0",
        "Contour 0,0,0 1,0,0 -1",
        "Contour 0,0,0 1,0,0 NaN",
        "Contour 0,0,0 0,0,0 1",
        "Contour 0,0,0 1,0,0 0.000001",
        "Contour 0,0,0 1,0,0 1 Range=Yes Range=No",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.undo_label(), label.as_deref());
    }
}
#[test]
fn far_base_grid_retains_indices_above_f64_integer_precision() {
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Line(
        viboceros_geometry::LineSegment::try_new(p(-2., 0., 0.), p(2., 0., 0.), doc.tolerance())
            .unwrap(),
    ))
    .unwrap();
    let inputs = doc.objects().collect::<Vec<_>>();
    let planes = plane_origins(
        &inputs,
        p(1e16, 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        1.,
        1.,
        false,
        doc.tolerance(),
    )
    .unwrap();
    assert_eq!(
        planes,
        vec![
            p(-2., 0., 0.),
            p(-1., 0., 0.),
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(2., 0., 0.)
        ]
    );
}
#[test]
fn range_uses_completed_spacing_intervals_and_can_create_no_planes() {
    for (end, spacing, count) in [(0.75, 1., 0), (1.1, 0.5, 2), (1.9, 1., 1), (2.25, 1., 2)] {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Line -2,0,0 2,0,0").unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        registry
            .execute(
                &mut doc,
                &format!("Contour 0,0,0 {end},0,0 {spacing} Range=Yes"),
            )
            .unwrap();
        assert_eq!(doc.objects().len(), count + 1);
    }
}
#[test]
fn unrepresentable_spacing_and_ineligible_sources_fail_without_outputs() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "Line 10000000000000000,0,0 10000000000000004,0,0")
        .unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut doc, "Contour 0,0,0 1,0,0 1"),
        Err(CommandError::ContourUnrepresentablePlane)
    ));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 0,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    assert!(matches!(
        registry.execute(&mut doc, "Contour 0,0,0 1,0,0 1"),
        Err(CommandError::NoObjectsSelected)
    ));
    assert_eq!(doc.objects().len(), 1);
}
#[test]
fn contour_preserves_native_edge_parameters_and_signed_axis_line_domains() {
    let surface = NurbsSurface::try_bilinear([
        p(-2., -2., 0.),
        p(2., -2., 0.),
        p(2., 2., 0.),
        p(-2., 2., 0.),
    ])
    .unwrap();
    let source = Geometry::NurbsSurface(surface.clone());
    let frame = Frame3::try_from_normal(
        p(0., 6., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cut = geometry::cut(&source, frame, Tolerance::DEFAULT).unwrap();
    let curve = cut[0].curve_ref().unwrap();
    assert_eq!(curve.domain(), 4. ..=8.);
    let frame = Frame3::try_from_normal(
        p(2., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cut = geometry::cut(&source, frame, Tolerance::DEFAULT).unwrap();
    assert_eq!(cut[0].curve_ref().unwrap().domain(), -1. ..=0.);
    assert_eq!(source, Geometry::NurbsSurface(surface));
}
#[test]
fn contour_exact_isocurve_lineage_preserves_shifted_uv_domains_and_flattens_linear_weights() {
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            (p(-2., -2., 0.), 1.),
            (p(2., -2., 0.), 1.),
            (p(-2., 2., 0.), 2.),
            (p(2., 2., 0.), 2.),
        ]
        .into_iter()
        .map(|(p, w)| viboceros_geometry::WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![2., 2., 8., 8.],
        vec![3., 3., 9., 9.],
    )
    .unwrap();
    let source = Geometry::NurbsSurface(surface.clone());
    for (x, z) in [(2., 0.), (1., 100.)] {
        let frame = Frame3::try_from_normal(
            p(x, 0., z),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cut = geometry::cut(&source, frame, Tolerance::DEFAULT).unwrap();
        let curve = cut[0].curve_ref().unwrap();
        assert_eq!(curve.domain(), -9. ..=-3.);
        assert!(curve.evaluate(-6.).unwrap().y().abs() < 1e-12);
    }
    assert_eq!(source, Geometry::NurbsSurface(surface));
}
#[test]
fn one_output_per_plane_is_not_grouped_even_when_grouping_is_enabled() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Line -2,0,0 2,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry
        .execute(
            &mut doc,
            "Contour 0,0,0 1,0,0 1 GroupObjectsByContourPlane=Yes",
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 6);
    assert_eq!(doc.groups().len(), 0);
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 1);
}
