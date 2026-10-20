use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn scene() -> (Document, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let source = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(-4., 0., 0.), p(4., 0., 0.), doc.tolerance()).unwrap(),
        ))
        .unwrap();
    let surface = doc
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                p(-10., -10., 0.),
                p(10., -10., 0.),
                p(10., 10., 0.),
                p(-10., 10., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    (doc, source, surface)
}
fn samples(doc: &Document) -> Vec<Point3> {
    doc.objects()
        .filter(|o| o.geometry().curve_ref().is_some())
        .skip(1)
        .map(|o| {
            let c = o.geometry().curve_ref().unwrap();
            c.evaluate(c.parameter_at(0.5).unwrap()).unwrap()
        })
        .collect()
}

#[test]
fn distances_flip_and_number_create_one_atomic_history_entry() {
    let (mut doc, source, surface) = scene();
    let registry = CommandRegistry::with_builtins();
    let original = (
        doc.object(source).unwrap().geometry_snapshot().clone(),
        doc.object(surface).unwrap().geometry_snapshot().clone(),
    );
    doc.select_object(source, SelectionMode::Replace).unwrap();
    registry
        .execute(
            &mut doc,
            &format!("OffsetCrvOnSrf 1 Surface={surface} Number=3 SidePoint=0,-2,0 Flip=Yes"),
        )
        .unwrap();
    let mut ys = samples(&doc).iter().map(|p| p.y()).collect::<Vec<_>>();
    ys.sort_by(Real::total_cmp);
    assert_eq!(ys.len(), 3);
    for (i, y) in ys.iter().enumerate() {
        assert!((y - (i + 1) as Real).abs() < 1e-8);
    }
    assert!(
        original
            .0
            .shares_storage_with(doc.object(source).unwrap().geometry_snapshot())
    );
    assert!(
        original
            .1
            .shares_storage_with(doc.object(surface).unwrap().geometry_snapshot())
    );
    assert_eq!(doc.undo_label(), Some("OffsetCrvOnSrf"));
    doc.undo().unwrap();
    assert_eq!(doc.objects().count(), 2);
    doc.redo().unwrap();
    assert_eq!(doc.objects().count(), 5);
}

#[test]
fn directed_temporary_sources_and_surface_edges_preserve_their_parents() {
    let (mut doc, source, surface) = scene();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(
            &mut doc,
            &format!("OffsetCrvOnSrf 1 Source={source} Surface={surface} SubCrv=6,2"),
        )
        .unwrap();
    let c = doc
        .objects()
        .last()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!(
        c.start_point()
            .unwrap()
            .distance_to(p(10., -1., 0.))
            .unwrap()
            < 1e-8
    );
    assert!(
        c.end_point()
            .unwrap()
            .distance_to(p(-10., -1., 0.))
            .unwrap()
            < 1e-8
    );
    let edge = curve_reference::resolve(
        doc.object(surface).unwrap().geometry(),
        Some(0),
        doc.tolerance(),
    )
    .unwrap()
    .curve()
    .to_nurbs()
    .unwrap();
    // A negative face-left distance points into this edge's rectangle.
    let normal = UnitVector3::try_new(0., 0., 1., doc.tolerance()).unwrap();
    let t = CurveRef::NurbsCurve(&edge)
        .evaluate_with_tangent(*edge.domain().start())
        .unwrap();
    let left = normal.as_vector().cross(t.tangent().as_vector()).unwrap();
    let middle = edge.evaluate(edge.parameter_at(0.5).unwrap()).unwrap();
    let center = p(0., 0., 0.);
    let distance = if middle.vector_to(center).unwrap().dot(left).unwrap() > 0. {
        1.
    } else {
        -1.
    };
    registry
        .execute(
            &mut doc,
            &format!("OffsetCrvOnSrf {distance} Edge={surface},0 Surface={surface}"),
        )
        .unwrap();
    assert_eq!(doc.selected_object_count(), 0);
    assert!(matches!(
        doc.object(surface).unwrap().geometry(),
        Geometry::NurbsSurface(_)
    ));
}

#[test]
fn previews_and_failed_commands_preserve_selection_redo_and_geometry() {
    let (mut doc, source, surface) = scene();
    let registry = CommandRegistry::with_builtins();
    doc.select_object(source, SelectionMode::Replace).unwrap();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for n in 1..=3 {
        let plan = Plan::prepare(
            &doc,
            Source::capture(&doc, source, None, None).unwrap(),
            Target::capture(&doc, surface, None).unwrap(),
            None,
            Options {
                number: n,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(plan.current(&doc));
        assert_eq!(plan.curves.len(), n);
    }
    assert_eq!(format!("{doc:?}"), before);
    for arguments in [
        "0 Number=1",
        "1 Number=0",
        "1 Number=1025",
        "1 Flip=Maybe",
        "1 SidePoint=0,0,0",
        "1 SubCrv=0.2,0.2",
    ] {
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("OffsetCrvOnSrf {arguments} Surface={surface} Source={source}")
                )
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    doc.redo().unwrap();
}

#[test]
fn reversed_faces_change_the_default_side_but_an_explicit_pick_controls_world_side() {
    let (mut doc, source, surface) = scene();
    let s = match doc.object(surface).unwrap().geometry() {
        Geometry::NurbsSurface(s) => s.clone(),
        _ => unreachable!(),
    };
    let brep = Brep::try_surface_face(s, doc.tolerance())
        .unwrap()
        .reversed();
    doc.replace_object_geometries([(surface, Geometry::Brep(brep))])
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(
            &mut doc,
            &format!("OffsetCrvOnSrf 1 Source={source} Surface={surface}"),
        )
        .unwrap();
    assert!((samples(&doc)[0].y() + 1.).abs() < 1e-8);
    registry
        .execute(
            &mut doc,
            &format!("OffsetCrvOnSrf 1 Source={source} Surface={surface} SidePoint=0,3,0"),
        )
        .unwrap();
    assert!((samples(&doc).last().unwrap().y() - 1.).abs() < 1e-8);
}

#[test]
fn a_trimmed_planar_face_keeps_its_hole_and_can_return_two_offset_pieces() {
    let (mut doc, source, surface) = scene();
    let polygon = |points: Vec<Point3>| {
        Polyline3::try_new(points, doc.tolerance())
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let outer = polygon(vec![
        p(-5., -5., 0.),
        p(5., -5., 0.),
        p(5., 5., 0.),
        p(-5., 5., 0.),
        p(-5., -5., 0.),
    ]);
    let hole = polygon(vec![
        p(-1., 0.5, 0.),
        p(1., 0.5, 0.),
        p(1., 1.5, 0.),
        p(-1., 1.5, 0.),
        p(-1., 0.5, 0.),
    ]);
    let face = Brep::try_planar_face_with_holes(&outer, &[hole], doc.tolerance()).unwrap();
    doc.replace_object_geometries([(surface, Geometry::Brep(face))])
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut doc,
            &format!("OffsetCrvOnSrf 1 Source={source} Surface={surface} SidePoint=0,2,0"),
        )
        .unwrap();
    assert_eq!(samples(&doc).len(), 2);
    for q in samples(&doc) {
        assert!((q.y() - 1.).abs() < 1e-8);
        assert!(q.x().abs() > 1.);
    }
}
